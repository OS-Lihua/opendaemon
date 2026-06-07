use std::{fs, path::Path};
use std::time::Duration;

use axum::{
    Json,
    body::to_bytes,
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
};
use serde_json::{Value, json};
use tokio::time::timeout;

use crate::{
    agent::profile::{CreateAgentProfile, ExecutionPolicy, ProviderConfig},
    api::{
        AppState, task_cancel, task_create, task_events, task_get, task_list, task_post_event,
        tasks::{CreateTaskRequest, TaskEventRequest, TaskEventsQuery, TaskListQuery},
    },
    config::{RuntimeDetectionConfig, SchedulerConfig, StoreConfig},
    runtime::store::RuntimeStore,
    scheduler::{
        locks::{LockDecision, LockRequest},
        service::{SchedulerService, TaskValidationError},
        workspace::{FailingWorkspacePreparer, FakeWorkspacePreparer, WorkspacePreparer},
    },
    security::directory::{
        DirectoryCapability, DirectoryLockPolicy, WorkspaceMode as DirectoryWorkspaceMode,
    },
    store::{
        agent_profiles::AgentProfileStore,
        directory_grants::{CreateDirectoryGrant, DirectoryGrantStore},
        tasks::{TaskFilters, TaskStore},
    },
    task::{
        event::{PermissionDecision, PermissionRequestEvent, TaskEventType},
        model::{CreateTask, TaskStatus},
        permission::PermissionRequestStatus,
        service::{TaskStreamFrame, is_terminal_event_type},
        state::{TaskTransition, validate_transition},
    },
    tests::TempDir,
};

#[test]
fn task_model_validates_defaults_and_state_transitions() {
    assert_eq!(
        serde_json::to_value(TaskStatus::WaitingDirectoryLock).unwrap(),
        json!("waiting_directory_lock")
    );

    let valid = create_task_input("product", "agent", "dir_1");
    assert_eq!(
        valid.required_capabilities(),
        vec![DirectoryCapability::Read]
    );
    assert!(valid.validate().is_ok());

    assert!(
        CreateTask {
            prompt: " ".to_owned(),
            ..valid.clone()
        }
        .validate()
        .is_err()
    );
    assert!(
        CreateTask {
            required_capabilities: Some(vec![]),
            ..valid.clone()
        }
        .validate()
        .is_err()
    );
    assert!(
        CreateTask {
            metadata: Some(json!("not-object")),
            ..valid.clone()
        }
        .validate()
        .is_err()
    );
    assert!(
        CreateTask {
            timeout_seconds: Some(0),
            ..valid.clone()
        }
        .validate()
        .is_err()
    );

    assert_eq!(
        validate_transition(TaskStatus::Queued, TaskStatus::WaitingDirectoryLock).unwrap(),
        TaskTransition::Changed
    );
    assert_eq!(
        validate_transition(TaskStatus::Cancelled, TaskStatus::Cancelled).unwrap(),
        TaskTransition::Idempotent
    );
    assert!(validate_transition(TaskStatus::Completed, TaskStatus::Running).is_err());
    assert!(TaskStatus::Completed.is_terminal());
    assert!(!TaskStatus::Running.is_terminal());
}

#[test]
fn task_store_persists_filters_transitions_events_results_and_locks() {
    let temp_dir = TempDir::new();
    let store = temp_task_store(temp_dir.path());

    let first = store
        .create(create_task_input("product-a", "agent-a", "dir-a"))
        .unwrap();
    let second = store
        .create(CreateTask {
            owner_product_id: "product-b".to_owned(),
            agent_id: "agent-b".to_owned(),
            directory_id: "dir-b".to_owned(),
            workspace_mode: Some(DirectoryWorkspaceMode::Direct),
            required_capabilities: Some(vec![
                DirectoryCapability::Read,
                DirectoryCapability::Write,
            ]),
            ..create_task_input("product-b", "agent-b", "dir-b")
        })
        .unwrap();

    assert_eq!(first.id, "task_1");
    assert_eq!(first.status, TaskStatus::Queued);
    assert_eq!(first.required_capabilities, vec![DirectoryCapability::Read]);
    time::OffsetDateTime::parse(
        &first.created_at,
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();
    let events = store.list_events(&first.id).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].sequence, 1);
    assert_eq!(events[0].event_type, TaskEventType::Queued);

    let reopened = temp_task_store(temp_dir.path());
    assert_eq!(
        reopened
            .list(TaskFilters::default())
            .unwrap()
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>(),
        ["task_1", "task_2"]
    );
    assert_eq!(
        reopened
            .list(TaskFilters {
                owner_product_id: Some("product-a".to_owned()),
                ..TaskFilters::default()
            })
            .unwrap(),
        vec![first.clone()]
    );
    assert_eq!(
        reopened
            .list(TaskFilters {
                status: Some(TaskStatus::Queued),
                ..TaskFilters::default()
            })
            .unwrap()
            .len(),
        2
    );

    let waiting = reopened
        .transition(
            &second.id,
            TaskStatus::WaitingDirectoryLock,
            Some(json!({"reason": "conflict"})),
        )
        .unwrap();
    assert_eq!(waiting.status, TaskStatus::WaitingDirectoryLock);
    assert_ne!(waiting.updated_at, second.updated_at);
    assert_eq!(reopened.list_events(&second.id).unwrap().len(), 2);

    let cancelled = reopened.cancel(&second.id).unwrap();
    assert_eq!(cancelled.status, TaskStatus::Cancelled);
    assert!(cancelled.cancelled_at.is_some());
    assert_eq!(reopened.cancel(&second.id).unwrap(), cancelled);

    reopened
        .transition(&first.id, TaskStatus::WaitingDirectoryLock, None)
        .unwrap();
    reopened
        .transition(&first.id, TaskStatus::Preparing, None)
        .unwrap();
    reopened
        .transition(&first.id, TaskStatus::Running, None)
        .unwrap();
    let completed = reopened
        .transition(&first.id, TaskStatus::Completed, None)
        .unwrap();
    assert!(completed.completed_at.is_some());
    reopened
        .save_result(&first.id, "Task completed.", vec!["src/lib.rs".to_owned()])
        .unwrap();
    let fetched = reopened.get(&first.id).unwrap();
    assert_eq!(
        fetched.result.as_ref().unwrap().final_message,
        "Task completed."
    );
    assert_eq!(fetched.result.unwrap().changed_files, ["src/lib.rs"]);
}

#[tokio::test]
async fn task_event_service_replays_from_cursor_tails_live_and_emits_heartbeat() {
    let temp_dir = TempDir::new();
    let state = fixture_state_with_scheduler_config(
        temp_dir.path(),
        false,
        SchedulerConfig {
            task_event_heartbeat_interval: Duration::from_millis(25),
            ..SchedulerConfig::default()
        },
    );
    let grant = create_fixture_grant(temp_dir.path(), "product", "agent", false, false);
    let task = state
        .task_store()
        .create(create_task_input("product", "agent", &grant.id))
        .unwrap();
    state
        .task_store()
        .transition(&task.id, TaskStatus::WaitingDirectoryLock, None)
        .unwrap();
    state
        .task_store()
        .transition(&task.id, TaskStatus::Preparing, None)
        .unwrap();
    let service = state.task_event_service();
    let mut stream = service.stream(&task.id, 1).unwrap();

    match timeout(Duration::from_secs(1), stream.recv()).await.unwrap() {
        Some(TaskStreamFrame::Event(event)) => {
            assert_eq!(event.sequence, 2);
            assert_eq!(event.event_type, TaskEventType::WaitingDirectoryLock);
        }
        other => panic!("unexpected frame: {other:?}"),
    }

    match timeout(Duration::from_secs(1), stream.recv()).await.unwrap() {
        Some(TaskStreamFrame::Event(event)) => {
            assert_eq!(event.sequence, 3);
            assert_eq!(event.event_type, TaskEventType::Preparing);
        }
        other => panic!("unexpected frame: {other:?}"),
    }

    match timeout(Duration::from_millis(200), stream.recv()).await.unwrap() {
        Some(TaskStreamFrame::Heartbeat) => {}
        other => panic!("unexpected frame: {other:?}"),
    }

    state
        .task_store()
        .append_event(
            &task.id,
            TaskEventType::ProcessStdout,
            json!({"text":"hello","stream":"stdout"}),
        )
        .unwrap();

    match timeout(Duration::from_secs(1), stream.recv()).await.unwrap() {
        Some(TaskStreamFrame::Event(event)) => {
            assert_eq!(event.event_type, TaskEventType::ProcessStdout);
            assert_eq!(event.payload["text"], "hello");
        }
        other => panic!("unexpected frame: {other:?}"),
    }
}

#[tokio::test]
async fn terminal_task_event_stream_replays_then_closes() {
    let temp_dir = TempDir::new();
    let state = fixture_state(temp_dir.path(), false);
    let grant = create_fixture_grant(temp_dir.path(), "product", "agent", false, false);
    let task = state
        .task_store()
        .create(create_task_input("product", "agent", &grant.id))
        .unwrap();
    state
        .task_store()
        .transition(&task.id, TaskStatus::WaitingDirectoryLock, None)
        .unwrap();
    state
        .task_store()
        .transition(&task.id, TaskStatus::Preparing, None)
        .unwrap();
    state
        .task_store()
        .transition(&task.id, TaskStatus::Running, None)
        .unwrap();
    state.task_store().transition(&task.id, TaskStatus::Completed, None).unwrap();
    let service = state.task_event_service();
    let mut stream = service.stream(&task.id, 0).unwrap();

    let mut seen = Vec::new();
    while let Some(frame) = timeout(Duration::from_secs(1), stream.recv()).await.unwrap() {
        match frame {
            TaskStreamFrame::Event(event) => seen.push(event.event_type),
            TaskStreamFrame::Heartbeat => panic!("terminal stream should not heartbeat"),
        }
    }

    assert_eq!(
        seen,
        vec![
            TaskEventType::Queued,
            TaskEventType::WaitingDirectoryLock,
            TaskEventType::Preparing,
            TaskEventType::Running,
            TaskEventType::Completed
        ]
    );
    assert!(seen.last().copied().is_some_and(is_terminal_event_type));
}

#[test]
fn task_store_persists_permission_requests_and_idempotent_resolution() {
    let temp_dir = TempDir::new();
    let state = fixture_state(temp_dir.path(), false);
    let grant = create_fixture_grant(temp_dir.path(), "product", "agent", false, false);
    let task = state
        .task_store()
        .create(create_task_input("product", "agent", &grant.id))
        .unwrap();
    let requested = state
        .task_store()
        .record_permission_request(
            &task.id,
            PermissionRequestEvent {
                request_id: "perm_1".to_owned(),
                provider_id: "acp-example".to_owned(),
                permission_kind: "shell_command".to_owned(),
                summary: "run git push".to_owned(),
                details: Some(json!({"command":["git","push"]})),
                options: vec![PermissionDecision::Approve, PermissionDecision::Deny],
                expires_at: None,
            },
        )
        .unwrap();
    assert_eq!(
        requested.event_type,
        TaskEventType::ProviderPermissionRequested
    );

    let reopened = temp_task_store(temp_dir.path());
    let pending = reopened
        .get_permission_request(&task.id, "perm_1")
        .unwrap();
    assert_eq!(pending.status, PermissionRequestStatus::Pending);

    let resolution = reopened
        .resolve_permission_request(
            &task.id,
            "perm_1",
            PermissionDecision::Approve,
            Some("approved".to_owned()),
        )
        .unwrap();
    assert_eq!(resolution.status, PermissionRequestStatus::Approved);
    assert!(!resolution.duplicated);
    assert_eq!(
        resolution.event.event_type,
        TaskEventType::ProviderPermissionDecided
    );

    let duplicate = reopened
        .resolve_permission_request(
            &task.id,
            "perm_1",
            PermissionDecision::Approve,
            Some("approved".to_owned()),
        )
        .unwrap();
    assert!(duplicate.duplicated);

    let conflict = reopened.resolve_permission_request(
        &task.id,
        "perm_1",
        PermissionDecision::Deny,
        None,
    );
    assert!(matches!(
        conflict.unwrap_err(),
        crate::store::tasks::TaskStoreError::PermissionRequestAlreadyResolved
    ));
}

#[test]
fn scheduler_validates_task_policy_and_directory_locks() {
    let temp_dir = TempDir::new();
    let state = fixture_state(temp_dir.path(), true);
    let service = SchedulerService::new(
        state.task_store().clone(),
        state.agent_profile_store().clone(),
        state.directory_grant_store().clone(),
        SchedulerConfig::default(),
    );
    let grant = create_fixture_grant(temp_dir.path(), "product", "agent", true, true);

    let task = service
        .enqueue_task(CreateTask {
            directory_id: grant.id.clone(),
            required_capabilities: Some(vec![
                DirectoryCapability::Read,
                DirectoryCapability::Write,
            ]),
            workspace_mode: Some(DirectoryWorkspaceMode::Direct),
            direct_mode_task_opt_in: true,
            ..create_task_input("product", "agent", &grant.id)
        })
        .unwrap();
    assert_eq!(task.status, TaskStatus::Queued);

    let lock = service
        .try_acquire_directory_lock(&LockRequest::from_task(&task))
        .unwrap();
    assert_eq!(lock, LockDecision::Acquired);

    let conflicting = service
        .enqueue_task(CreateTask {
            directory_id: grant.id.clone(),
            required_capabilities: Some(vec![DirectoryCapability::Write]),
            workspace_mode: Some(DirectoryWorkspaceMode::Direct),
            direct_mode_task_opt_in: true,
            ..create_task_input("product", "agent", &grant.id)
        })
        .unwrap();
    let lock = service
        .try_acquire_directory_lock(&LockRequest::from_task(&conflicting))
        .unwrap();
    assert_eq!(lock, LockDecision::Waiting);
    assert_eq!(
        state.task_store().get(&conflicting.id).unwrap().status,
        TaskStatus::WaitingDirectoryLock
    );

    service.cancel_task(&conflicting.id).unwrap();
    service.mark_preparing(&task.id).unwrap();
    service.mark_running(&task.id).unwrap();
    service.complete_task(&task.id, "done").unwrap();
    assert!(
        state
            .task_store()
            .active_locks(&grant.id)
            .unwrap()
            .is_empty()
    );

    let wrong_owner = service.enqueue_task(CreateTask {
        owner_product_id: "wrong".to_owned(),
        directory_id: grant.id.clone(),
        workspace_mode: Some(DirectoryWorkspaceMode::Worktree),
        ..create_task_input("wrong", "agent", &grant.id)
    });
    assert!(matches!(
        wrong_owner.unwrap_err(),
        TaskValidationError::AgentAuthorizationFailed
    ));

    let provider_override = service.enqueue_task(CreateTask {
        directory_id: grant.id.clone(),
        provider_id: Some("other-provider".to_owned()),
        workspace_mode: Some(DirectoryWorkspaceMode::Worktree),
        ..create_task_input("product", "agent", &grant.id)
    });
    assert!(matches!(
        provider_override.unwrap_err(),
        TaskValidationError::ProviderOverrideNotAllowed
    ));

    let model_override = service.enqueue_task(CreateTask {
        directory_id: grant.id.clone(),
        model: Some("other-model".to_owned()),
        workspace_mode: Some(DirectoryWorkspaceMode::Worktree),
        ..create_task_input("product", "agent", &grant.id)
    });
    assert!(matches!(
        model_override.unwrap_err(),
        TaskValidationError::ModelOverrideNotAllowed
    ));

    let permission_override = service.enqueue_task(CreateTask {
        directory_id: grant.id.clone(),
        permission_mode: Some("trusted".to_owned()),
        workspace_mode: Some(DirectoryWorkspaceMode::Worktree),
        ..create_task_input("product", "agent", &grant.id)
    });
    assert!(matches!(
        permission_override.unwrap_err(),
        TaskValidationError::PermissionModeOverrideNotAllowed
    ));

    let missing_agent = service.enqueue_task(CreateTask {
        agent_id: "missing".to_owned(),
        directory_id: grant.id.clone(),
        workspace_mode: Some(DirectoryWorkspaceMode::Worktree),
        ..create_task_input("product", "missing", &grant.id)
    });
    assert!(matches!(
        missing_agent.unwrap_err(),
        TaskValidationError::AgentNotFound
    ));

    let missing_capability = service.enqueue_task(CreateTask {
        directory_id: create_fixture_grant(temp_dir.path(), "product", "agent", false, false).id,
        required_capabilities: Some(vec![DirectoryCapability::Write]),
        workspace_mode: Some(DirectoryWorkspaceMode::Worktree),
        ..create_task_input("product", "agent", &grant.id)
    });
    assert!(matches!(
        missing_capability.unwrap_err(),
        TaskValidationError::CapabilityNotAllowed
    ));
}

#[test]
fn scheduler_allows_shared_read_locks_and_enforces_global_capacity() {
    let temp_dir = TempDir::new();
    let state = fixture_state(temp_dir.path(), false);
    let service = SchedulerService::new(
        state.task_store().clone(),
        state.agent_profile_store().clone(),
        state.directory_grant_store().clone(),
        SchedulerConfig {
            max_concurrent_tasks: 1,
            ..SchedulerConfig::default()
        },
    );
    let grant = create_fixture_grant(temp_dir.path(), "product", "agent", false, false);
    let first = service
        .enqueue_task(CreateTask {
            directory_id: grant.id.clone(),
            workspace_mode: Some(DirectoryWorkspaceMode::Worktree),
            ..create_task_input("product", "agent", &grant.id)
        })
        .unwrap();
    let second = service
        .enqueue_task(CreateTask {
            directory_id: grant.id.clone(),
            workspace_mode: Some(DirectoryWorkspaceMode::Worktree),
            ..create_task_input("product", "agent", &grant.id)
        })
        .unwrap();

    assert_eq!(
        service
            .try_acquire_directory_lock(&LockRequest::from_task(&first))
            .unwrap(),
        LockDecision::Acquired
    );
    assert_eq!(
        service
            .try_acquire_directory_lock(&LockRequest::from_task(&second))
            .unwrap(),
        LockDecision::Acquired
    );
    assert_eq!(state.task_store().active_locks(&grant.id).unwrap().len(), 2);

    service.mark_preparing(&first.id).unwrap();
    let other_grant = create_fixture_grant(temp_dir.path(), "product", "agent", false, false);
    let third = service
        .enqueue_task(CreateTask {
            directory_id: other_grant.id.clone(),
            workspace_mode: Some(DirectoryWorkspaceMode::Worktree),
            ..create_task_input("product", "agent", &other_grant.id)
        })
        .unwrap();
    assert_eq!(
        service
            .try_acquire_directory_lock(&LockRequest::from_task(&third))
            .unwrap(),
        LockDecision::Waiting
    );
}

#[test]
fn workspace_preparation_failure_marks_task_failed() {
    let temp_dir = TempDir::new();
    let state = fixture_state(temp_dir.path(), false);
    let service = SchedulerService::new(
        state.task_store().clone(),
        state.agent_profile_store().clone(),
        state.directory_grant_store().clone(),
        SchedulerConfig::default(),
    );
    let grant = create_fixture_grant(temp_dir.path(), "product", "agent", false, false);
    let task = service
        .enqueue_task(CreateTask {
            directory_id: grant.id.clone(),
            workspace_mode: Some(DirectoryWorkspaceMode::Worktree),
            ..create_task_input("product", "agent", &grant.id)
        })
        .unwrap();
    service
        .try_acquire_directory_lock(&LockRequest::from_task(&task))
        .unwrap();

    assert!(
        service
            .prepare_workspace(&task.id, &FailingWorkspacePreparer)
            .is_err()
    );
    assert_eq!(
        state.task_store().get(&task.id).unwrap().status,
        TaskStatus::Failed
    );
    assert!(
        state
            .task_store()
            .get(&task.id)
            .unwrap()
            .failed_at
            .is_some()
    );
}

#[test]
fn workspace_preparer_handles_direct_and_fake_worktree_without_provider_execution() {
    let temp_dir = TempDir::new();
    let direct_grant = create_fixture_grant(temp_dir.path(), "product", "agent", true, true);
    let preparer = FakeWorkspacePreparer::new(temp_dir.path().join("workspaces"));

    let direct = preparer
        .prepare("task_1", &direct_grant, DirectoryWorkspaceMode::Direct)
        .unwrap();
    assert_eq!(
        direct.working_directory,
        std::path::PathBuf::from(&direct_grant.path)
    );
    assert!(direct.worktree_path.is_none());

    let worktree = preparer
        .prepare("task_2", &direct_grant, DirectoryWorkspaceMode::Worktree)
        .unwrap();
    assert_eq!(worktree.workspace_mode, DirectoryWorkspaceMode::Worktree);
    assert!(worktree.working_directory.ends_with("task_2"));
    assert_eq!(worktree.branch_name.as_deref(), Some("opendaemon/task_2"));
}

#[tokio::test]
async fn task_api_creates_lists_gets_and_cancels_tasks() {
    let temp_dir = TempDir::new();
    let state = fixture_state(temp_dir.path(), false);
    let grant = create_fixture_grant(temp_dir.path(), "product", "agent", true, false);

    let initial = task_list(
        State(state.clone()),
        Query(TaskListQuery {
            owner_product_id: None,
            agent_id: None,
            directory_id: None,
            status: None,
        }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(initial.tasks, vec![]);

    let created = task_create(
        State(state.clone()),
        Json(
            serde_json::from_value::<CreateTaskRequest>(json!({
                "owner_product_id": "product",
                "agent_id": "agent",
                "directory_id": grant.id,
                "prompt": "Inspect this project.",
                "required_capabilities": ["read"],
                "workspace_mode": "worktree",
                "metadata": {"issue_id": "BUG-123"}
            }))
            .unwrap(),
        ),
    )
    .await
    .unwrap();
    assert_eq!(created.0, StatusCode::CREATED);
    let task = created.1.0.task;
    assert_eq!(task.id, "task_1");
    assert_eq!(task.status, TaskStatus::Queued);
    let body = serde_json::to_string(&task).unwrap();
    assert!(!body.contains("runtime"));
    assert!(!body.contains("provider_result"));
    assert!(!body.contains("secret"));
    assert!(!body.contains("token"));

    let fetched = task_get(State(state.clone()), AxumPath(task.id.clone()))
        .await
        .unwrap()
        .0
        .task;
    assert_eq!(fetched, task);

    let filtered = task_list(
        State(state.clone()),
        Query(TaskListQuery {
            owner_product_id: Some("product".to_owned()),
            agent_id: Some("agent".to_owned()),
            directory_id: Some(task.directory_id.clone()),
            status: Some(TaskStatus::Queued),
        }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(filtered.tasks, vec![task.clone()]);

    let cancelled = task_cancel(State(state.clone()), AxumPath(task.id.clone()))
        .await
        .unwrap()
        .0
        .task;
    assert_eq!(cancelled.status, TaskStatus::Cancelled);

    let error = task_get(State(state), AxumPath("missing".to_owned()))
        .await
        .unwrap_err();
    assert_error(error, StatusCode::NOT_FOUND, "task_not_found").await;
}

#[tokio::test]
async fn task_api_returns_stable_errors_for_invalid_policy() {
    let temp_dir = TempDir::new();
    let state = fixture_state(temp_dir.path(), false);
    let grant = create_fixture_grant(temp_dir.path(), "product", "agent", false, false);

    let error = task_create(
        State(state.clone()),
        Json(
            serde_json::from_value::<CreateTaskRequest>(json!({
                "owner_product_id": "product",
                "agent_id": "agent",
                "directory_id": grant.id,
                "prompt": "Write changes.",
                "required_capabilities": ["write"],
                "workspace_mode": "direct",
                "direct_mode_task_opt_in": true
            }))
            .unwrap(),
        ),
    )
    .await
    .unwrap_err();
    assert_error(error, StatusCode::FORBIDDEN, "direct_mode_not_allowed").await;

    let error = task_create(
        State(state),
        Json(
            serde_json::from_value::<CreateTaskRequest>(json!({
                "owner_product_id": "product",
                "agent_id": "agent",
                "directory_id": "missing",
                "prompt": "Read project."
            }))
            .unwrap(),
        ),
    )
    .await
    .unwrap_err();
    assert_error(error, StatusCode::NOT_FOUND, "directory_not_found").await;
}

#[tokio::test]
async fn task_events_api_validates_cursor_and_prefers_query_cursor() {
    let temp_dir = TempDir::new();
    let state = fixture_state(temp_dir.path(), false);
    let grant = create_fixture_grant(temp_dir.path(), "product", "agent", false, false);
    let task = state
        .task_store()
        .create(create_task_input("product", "agent", &grant.id))
        .unwrap();
    state
        .task_store()
        .transition(&task.id, TaskStatus::WaitingDirectoryLock, None)
        .unwrap();
    state
        .task_store()
        .transition(&task.id, TaskStatus::Preparing, None)
        .unwrap();
    state
        .task_store()
        .transition(&task.id, TaskStatus::Running, None)
        .unwrap();
    state
        .task_store()
        .transition(&task.id, TaskStatus::Completed, None)
        .unwrap();

    let error = task_events(
        State(state.clone()),
        AxumPath(task.id.clone()),
        Query(TaskEventsQuery {
            cursor: Some("bad".to_owned()),
        }),
        HeaderMap::new(),
    )
    .await
    .unwrap_err();
    assert_error(error, StatusCode::BAD_REQUEST, "invalid_event_cursor").await;

    let mut headers = HeaderMap::new();
    headers.insert("last-event-id", HeaderValue::from_static("3"));
    let response = task_events(
        State(state),
        AxumPath(task.id),
        Query(TaskEventsQuery {
            cursor: Some("1".to_owned()),
        }),
        headers,
    )
    .await
    .unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();

    assert!(text.contains("id: 2"));
    assert!(text.contains("id: 3"));
    assert!(text.contains("id: 4"));
    assert!(text.contains("id: 5"));
    assert!(!text.contains("id: 1"));
}

#[tokio::test]
async fn task_event_post_resolves_permission_request_and_reports_stable_errors() {
    let temp_dir = TempDir::new();
    let state = fixture_state(temp_dir.path(), false);
    let grant = create_fixture_grant(temp_dir.path(), "product", "agent", false, false);
    let task = state
        .task_store()
        .create(create_task_input("product", "agent", &grant.id))
        .unwrap();
    state
        .task_store()
        .record_permission_request(
            &task.id,
            PermissionRequestEvent {
                request_id: "perm_1".to_owned(),
                provider_id: "acp-example".to_owned(),
                permission_kind: "shell_command".to_owned(),
                summary: "run git push".to_owned(),
                details: None,
                options: vec![PermissionDecision::Approve, PermissionDecision::Deny],
                expires_at: None,
            },
        )
        .unwrap();
    let waiter = state.task_event_bus().register_waiter(&task.id, "perm_1");

    let response = task_post_event(
        State(state.clone()),
        AxumPath(task.id.clone()),
        Json(TaskEventRequest {
            event_type: "provider.permission_response".to_owned(),
            request_id: "perm_1".to_owned(),
            decision: "approve".to_owned(),
            reason: Some("approved".to_owned()),
        }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(response.status, "resolved");
    assert_eq!(response.decision, PermissionDecision::Approve);
    let decision = timeout(Duration::from_secs(1), waiter).await.unwrap().unwrap();
    assert_eq!(decision.decision, PermissionDecision::Approve);

    let duplicate = task_post_event(
        State(state.clone()),
        AxumPath(task.id.clone()),
        Json(TaskEventRequest {
            event_type: "provider.permission_response".to_owned(),
            request_id: "perm_1".to_owned(),
            decision: "approve".to_owned(),
            reason: None,
        }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(duplicate.status, "resolved");

    let conflict = task_post_event(
        State(state.clone()),
        AxumPath(task.id.clone()),
        Json(TaskEventRequest {
            event_type: "provider.permission_response".to_owned(),
            request_id: "perm_1".to_owned(),
            decision: "deny".to_owned(),
            reason: None,
        }),
    )
    .await
    .unwrap_err();
    assert_error(
        conflict,
        StatusCode::CONFLICT,
        "permission_request_already_resolved",
    )
    .await;

    let missing = task_post_event(
        State(state.clone()),
        AxumPath(task.id.clone()),
        Json(TaskEventRequest {
            event_type: "provider.permission_response".to_owned(),
            request_id: "missing".to_owned(),
            decision: "approve".to_owned(),
            reason: None,
        }),
    )
    .await
    .unwrap_err();
    assert_error(missing, StatusCode::NOT_FOUND, "permission_request_not_found").await;

    let invalid = task_post_event(
        State(state.clone()),
        AxumPath(task.id.clone()),
        Json(TaskEventRequest {
            event_type: "process.stdout".to_owned(),
            request_id: "perm_2".to_owned(),
            decision: "approve".to_owned(),
            reason: None,
        }),
    )
    .await
    .unwrap_err();
    assert_error(invalid, StatusCode::BAD_REQUEST, "invalid_event_request").await;

    let invalid_decision = task_post_event(
        State(state.clone()),
        AxumPath(task.id.clone()),
        Json(
            serde_json::from_value(json!({
                "event_type": "provider.permission_response",
                "request_id": "perm_2",
                "decision": "maybe",
                "reason": null
            }))
            .unwrap(),
        ),
    )
    .await
    .unwrap_err();
    assert_error(
        invalid_decision,
        StatusCode::BAD_REQUEST,
        "invalid_permission_decision",
    )
    .await;
}

async fn assert_error(error: crate::api::tasks::ApiError, status: StatusCode, code: &str) {
    let response = error.into_response();
    assert_eq!(response.status(), status);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], code);
}

fn fixture_state(root: &Path, allow_direct: bool) -> AppState {
    fixture_state_with_scheduler_config(root, allow_direct, SchedulerConfig::default())
}

fn fixture_state_with_scheduler_config(
    root: &Path,
    allow_direct: bool,
    scheduler_config: SchedulerConfig,
) -> AppState {
    let profile_store = temp_profile_store(root);
    profile_store
        .create(CreateAgentProfile {
            id: "agent".to_owned(),
            name: "Test Agent".to_owned(),
            owner_product_id: "product".to_owned(),
            provider_id: "codex".to_owned(),
            model: "gpt-5-codex".to_owned(),
            instructions: None,
            execution_policy: ExecutionPolicy {
                default_workspace_mode: if allow_direct {
                    crate::agent::profile::WorkspaceMode::Direct
                } else {
                    crate::agent::profile::WorkspaceMode::Worktree
                },
                allow_direct_directory: allow_direct,
            },
            provider_config: ProviderConfig::default(),
        })
        .unwrap();

    AppState::with_task_store(
        crate::registry::default_providers_dir(),
        RuntimeStore::default(),
        RuntimeDetectionConfig::default(),
        temp_directory_store(root),
        profile_store,
        temp_task_store(root),
        scheduler_config,
    )
}

fn create_fixture_grant(
    root: &Path,
    product_id: &str,
    agent_id: &str,
    allow_direct: bool,
    write: bool,
) -> crate::security::directory::DirectoryGrant {
    let project = root.join(format!("project-{}", unique_name()));
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(project.join(".git")).unwrap();
    temp_directory_store(root)
        .create(CreateDirectoryGrant {
            product_id: product_id.to_owned(),
            agent_id: agent_id.to_owned(),
            path: project,
            capabilities: if write {
                vec![DirectoryCapability::Read, DirectoryCapability::Write]
            } else {
                vec![DirectoryCapability::Read]
            },
            workspace_modes: Some(if allow_direct {
                vec![
                    DirectoryWorkspaceMode::Worktree,
                    DirectoryWorkspaceMode::Direct,
                ]
            } else {
                vec![DirectoryWorkspaceMode::Worktree]
            }),
            default_workspace_mode: Some(if allow_direct {
                DirectoryWorkspaceMode::Direct
            } else {
                DirectoryWorkspaceMode::Worktree
            }),
            lock_policy: Some(if write {
                DirectoryLockPolicy::Exclusive
            } else {
                DirectoryLockPolicy::Shared
            }),
            direct_mode_requires_explicit_task_opt_in: Some(true),
        })
        .unwrap()
}

fn create_task_input(product_id: &str, agent_id: &str, directory_id: &str) -> CreateTask {
    CreateTask {
        owner_product_id: product_id.to_owned(),
        agent_id: agent_id.to_owned(),
        directory_id: directory_id.to_owned(),
        prompt: "Do the work.".to_owned(),
        required_capabilities: None,
        workspace_mode: None,
        direct_mode_task_opt_in: false,
        metadata: Some(json!({"test": true})),
        provider_id: None,
        model: None,
        permission_mode: None,
        timeout_seconds: None,
    }
}

fn temp_profile_store(root: &Path) -> AgentProfileStore {
    AgentProfileStore::open(StoreConfig::new(root.join("opendaemon.sqlite3"))).unwrap()
}

fn temp_directory_store(root: &Path) -> DirectoryGrantStore {
    DirectoryGrantStore::open(StoreConfig::new(root.join("opendaemon.sqlite3"))).unwrap()
}

fn temp_task_store(root: &Path) -> TaskStore {
    TaskStore::open(StoreConfig::new(root.join("opendaemon.sqlite3"))).unwrap()
}

fn unique_name() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    NEXT_ID.fetch_add(1, Ordering::Relaxed).to_string()
}
