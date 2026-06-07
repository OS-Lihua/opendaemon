use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::{accept_async, tungstenite::Message};

use crate::{
    agent::profile::{CreateAgentProfile, ExecutionPolicy, ProviderConfig},
    config::{ControlPlaneConfig, RuntimeDetectionConfig, SchedulerConfig, StoreConfig},
    runtime::store::RuntimeStore,
    security::directory::{DirectoryCapability, DirectoryLockPolicy, WorkspaceMode},
    store::directory_grants::CreateDirectoryGrant,
    tests::{TempDir, temp_registry_with_provider, valid_manifest_json},
};

fn control_plane_test_app(
    temp_dir: &TempDir,
) -> (
    crate::api::AppState,
    crate::security::directory::DirectoryGrant,
    crate::store::tasks::TaskStore,
) {
    let mut manifest = valid_manifest_json();
    manifest["id"] = json!("codex");
    manifest["display_name"] = json!("Codex");
    manifest["models"]["default"] = json!("gpt-5-codex");
    manifest["models"]["supported"] = json!(["gpt-5-codex"]);
    let (_registry_temp, providers_dir) = temp_registry_with_provider("codex", manifest);
    let store_config = StoreConfig::new(temp_dir.path().join("dispatch.sqlite3"));
    let product_store = crate::store::products::ProductStore::open(store_config.clone()).unwrap();
    product_store
        .create_product(crate::product::CreateProduct {
            id: "product".to_owned(),
            display_name: "Product".to_owned(),
            description: None,
        })
        .unwrap();
    let directory_store =
        crate::store::directory_grants::DirectoryGrantStore::open(store_config.clone()).unwrap();
    let agent_store =
        crate::store::agent_profiles::AgentProfileStore::open(store_config.clone()).unwrap();
    let task_store = crate::store::tasks::TaskStore::open(store_config).unwrap();
    std::fs::create_dir_all(temp_dir.path().join("project/.git")).unwrap();
    let grant = directory_store
        .create(CreateDirectoryGrant {
            product_id: "product".to_owned(),
            agent_id: "agent".to_owned(),
            path: temp_dir.path().join("project"),
            capabilities: vec![DirectoryCapability::Read],
            workspace_modes: Some(vec![WorkspaceMode::Worktree]),
            default_workspace_mode: Some(WorkspaceMode::Worktree),
            lock_policy: Some(DirectoryLockPolicy::Shared),
            direct_mode_requires_explicit_task_opt_in: Some(true),
            allow_remote_execution: Some(false),
        })
        .unwrap();
    agent_store
        .create(CreateAgentProfile {
            id: "agent".to_owned(),
            name: "Agent".to_owned(),
            owner_product_id: "product".to_owned(),
            provider_id: "codex".to_owned(),
            model: "gpt-5-codex".to_owned(),
            instructions: None,
            execution_policy: ExecutionPolicy::default(),
            provider_config: ProviderConfig::default(),
        })
        .unwrap();
    let app_state = crate::api::AppState::with_task_store(
        providers_dir,
        RuntimeStore::default(),
        RuntimeDetectionConfig::default(),
        crate::config::AuthConfig::default(),
        product_store,
        directory_store,
        agent_store,
        task_store.clone(),
        SchedulerConfig::default(),
    );

    (app_state, grant, task_store)
}

#[tokio::test]
async fn control_plane_config_reads_env_and_defaults_disabled() {
    let _guard = crate::tests::process_env_test_guard().await;
    let endpoint_key = "OPENDAEMON_CONTROL_PLANE_URL";
    let secret_key = "OPENDAEMON_CONTROL_PLANE_ENROLLMENT_SECRET";
    let previous_endpoint = std::env::var_os(endpoint_key);
    let previous_secret = std::env::var_os(secret_key);
    unsafe {
        std::env::set_var(endpoint_key, "ws://127.0.0.1:4100/ws");
        std::env::set_var(secret_key, "enroll-secret");
    }

    let config = ControlPlaneConfig::from_env();
    assert_eq!(config.endpoint.as_deref(), Some("ws://127.0.0.1:4100/ws"));
    assert_eq!(config.enrollment_secret.as_deref(), Some("enroll-secret"));
    assert!(config.enabled());

    match previous_endpoint {
        Some(value) => unsafe { std::env::set_var(endpoint_key, value) },
        None => unsafe { std::env::remove_var(endpoint_key) },
    }
    match previous_secret {
        Some(value) => unsafe { std::env::set_var(secret_key, value) },
        None => unsafe { std::env::remove_var(secret_key) },
    }

    let disabled = ControlPlaneConfig::default();
    assert!(!disabled.enabled());
}

#[tokio::test]
async fn daemon_state_persists_registration_and_liveness_timestamps() {
    let temp_dir = TempDir::new();
    let store_config = StoreConfig::new(temp_dir.path().join("control-plane.sqlite3"));
    let store = crate::store::daemon_state::DaemonStateStore::open(store_config).unwrap();

    let registered = store
        .save_registration(crate::control_plane::model::DaemonRegistrationRecord {
            daemon_id: "daemon_123".to_owned(),
            control_plane_url: "ws://127.0.0.1:4100/ws".to_owned(),
            daemon_token: "daemon-token".to_owned(),
            status: crate::control_plane::model::DaemonConnectionStatus::Online,
            registered_at: "2026-06-07T00:00:00Z".to_owned(),
            last_heartbeat_at: Some("2026-06-07T00:00:30Z".to_owned()),
            last_error_code: None,
            session_id: Some("session_1".to_owned()),
        })
        .unwrap();

    assert_eq!(registered.daemon_id, "daemon_123");
    assert_eq!(
        registered.last_heartbeat_at.as_deref(),
        Some("2026-06-07T00:00:30Z")
    );

    let updated = store
        .mark_heartbeat(
            "daemon_123",
            "2026-06-07T00:01:00Z",
            crate::control_plane::model::DaemonConnectionStatus::Online,
        )
        .unwrap();
    assert_eq!(
        updated.last_heartbeat_at.as_deref(),
        Some("2026-06-07T00:01:00Z")
    );
}

#[test]
fn liveness_tracker_marks_connection_offline_after_staleness_threshold() {
    let tracker = crate::control_plane::liveness::LivenessTracker::new(Duration::from_secs(30));
    let online = tracker.evaluate(
        Some("2026-06-07T00:00:10Z"),
        "2026-06-07T00:00:35Z",
        crate::control_plane::model::DaemonConnectionStatus::Online,
    );
    assert_eq!(
        online,
        crate::control_plane::model::DaemonConnectionStatus::Online
    );

    let offline = tracker.evaluate(
        Some("2026-06-07T00:00:10Z"),
        "2026-06-07T00:01:00Z",
        crate::control_plane::model::DaemonConnectionStatus::Online,
    );
    assert_eq!(
        offline,
        crate::control_plane::model::DaemonConnectionStatus::Offline
    );
}

#[tokio::test]
async fn control_plane_dispatch_creates_durable_local_task() {
    let temp_dir = TempDir::new();
    let (app_state, grant, _task_store) = control_plane_test_app(&temp_dir);

    let dispatcher = crate::control_plane::dispatch::ControlPlaneDispatchService::new(app_state);
    let task = dispatcher
        .ingest(crate::control_plane::protocol::RemoteDispatchTask {
            remote_task_id: "remote_task_1".to_owned(),
            owner_product_id: "product".to_owned(),
            agent_id: "agent".to_owned(),
            directory_id: grant.id.clone(),
            prompt: "Do remote work".to_owned(),
            required_capabilities: vec!["read".to_owned()],
            workspace_mode: "worktree".to_owned(),
            timeout_seconds: Some(45),
            task_token: "task-token-1".to_owned(),
            metadata: json!({"source":"control_plane"}),
        })
        .await
        .unwrap();

    assert_eq!(task.status, crate::task::model::TaskStatus::Queued);
    assert_eq!(
        task.metadata.as_ref().unwrap()["control_plane"]["remote_task_id"],
        "remote_task_1"
    );
}

#[test]
fn daemon_registration_service_builds_resume_request_from_persisted_identity() {
    let temp_dir = TempDir::new();
    let store_config = StoreConfig::new(temp_dir.path().join("daemon-state.sqlite3"));
    let store = crate::store::daemon_state::DaemonStateStore::open(store_config).unwrap();
    let config = ControlPlaneConfig {
        endpoint: Some("ws://127.0.0.1:4100/ws".to_owned()),
        enrollment_secret: Some("enroll-secret".to_owned()),
        ..ControlPlaneConfig::default()
    };
    let service =
        crate::control_plane::registration::DaemonRegistrationService::new(config, store.clone());

    let initial = service.build_registration_request(Vec::new()).unwrap();
    assert_eq!(initial.daemon_id, None);
    assert_eq!(initial.session_id, None);
    assert_eq!(initial.enrollment_secret, "enroll-secret");

    service
        .accept(crate::control_plane::model::DaemonRegistrationAccepted {
            daemon_id: "daemon_123".to_owned(),
            daemon_token: "daemon-token".to_owned(),
            session_id: Some("session_1".to_owned()),
            registered_at: "2026-06-07T00:00:00Z".to_owned(),
        })
        .unwrap();

    let resumed = service.build_registration_request(Vec::new()).unwrap();
    assert_eq!(resumed.daemon_id.as_deref(), Some("daemon_123"));
    assert_eq!(resumed.session_id.as_deref(), Some("session_1"));
    assert_eq!(
        resumed.capabilities,
        vec![
            "task_dispatch".to_owned(),
            "task_cancel".to_owned(),
            "runtime_status".to_owned()
        ]
    );
}

#[tokio::test]
async fn control_plane_websocket_handler_dispatches_and_cancels_remote_task() {
    let temp_dir = TempDir::new();
    let (app_state, grant, task_store) = control_plane_test_app(&temp_dir);
    let daemon_state_store = crate::store::daemon_state::DaemonStateStore::open(StoreConfig::new(
        temp_dir.path().join("daemon-state.sqlite3"),
    ))
    .unwrap();
    let handler = crate::control_plane::client::ControlPlaneMessageHandler::new(
        crate::control_plane::dispatch::ControlPlaneDispatchService::new(app_state),
        daemon_state_store,
    );

    let dispatched = handler
        .handle_text(
            &json!({
                "type": "task_dispatch",
                "task": {
                    "remote_task_id": "remote_task_1",
                    "owner_product_id": "product",
                    "agent_id": "agent",
                    "directory_id": grant.id,
                    "prompt": "Do remote work",
                    "required_capabilities": ["read"],
                    "workspace_mode": "worktree",
                    "timeout_seconds": 45,
                    "task_token": "task-token-1",
                    "metadata": {"source": "control_plane"}
                }
            })
            .to_string(),
        )
        .await
        .unwrap();
    let local_task_id = match dispatched {
        crate::control_plane::client::HandledControlPlaneMessage::TaskDispatched(task) => {
            task.id.clone()
        }
        other => panic!("unexpected message result: {other:?}"),
    };

    let duplicate = handler
        .handle_text(
            &json!({
                "type": "task_dispatch",
                "task": {
                    "remote_task_id": "remote_task_1",
                    "owner_product_id": "product",
                    "agent_id": "agent",
                    "directory_id": grant.id,
                    "prompt": "Do remote work",
                    "required_capabilities": ["read"],
                    "workspace_mode": "worktree",
                    "timeout_seconds": 45,
                    "task_token": "task-token-1",
                    "metadata": {"source": "control_plane"}
                }
            })
            .to_string(),
        )
        .await
        .unwrap();
    match duplicate {
        crate::control_plane::client::HandledControlPlaneMessage::TaskDispatched(task) => {
            assert_eq!(task.id, local_task_id);
        }
        other => panic!("unexpected message result: {other:?}"),
    }

    let cancelled = handler
        .handle_text(
            &json!({
                "type": "task_cancel",
                "remote_task_id": "remote_task_1"
            })
            .to_string(),
        )
        .await
        .unwrap();
    match cancelled {
        crate::control_plane::client::HandledControlPlaneMessage::TaskCancelled(task) => {
            assert_eq!(task.id, local_task_id);
            assert_eq!(task.status, crate::task::model::TaskStatus::Cancelled);
        }
        other => panic!("unexpected message result: {other:?}"),
    }

    let persisted = task_store.get(&local_task_id).unwrap();
    assert_eq!(persisted.status, crate::task::model::TaskStatus::Cancelled);
}

#[tokio::test]
async fn terminal_callback_is_idempotent_for_repeated_delivery_attempts() {
    let temp_dir = TempDir::new();
    let (app_state, grant, _task_store) = control_plane_test_app(&temp_dir);
    let dispatch =
        crate::control_plane::dispatch::ControlPlaneDispatchService::new(app_state.clone());
    let task = dispatch
        .ingest(crate::control_plane::protocol::RemoteDispatchTask {
            remote_task_id: "remote_task_1".to_owned(),
            owner_product_id: "product".to_owned(),
            agent_id: "agent".to_owned(),
            directory_id: grant.id,
            prompt: "Do remote work".to_owned(),
            required_capabilities: vec!["read".to_owned()],
            workspace_mode: "worktree".to_owned(),
            timeout_seconds: Some(45),
            task_token: "task-token-1".to_owned(),
            metadata: json!({"source":"control_plane"}),
        })
        .await
        .unwrap();
    let task_store = app_state.task_store().clone();
    let callback_service = crate::control_plane::client::ControlPlaneCallbackService::default();

    task_store
        .transition(
            &task.id,
            crate::task::model::TaskStatus::WaitingDirectoryLock,
            None,
        )
        .unwrap();
    task_store
        .transition(&task.id, crate::task::model::TaskStatus::Preparing, None)
        .unwrap();
    task_store
        .transition(&task.id, crate::task::model::TaskStatus::Running, None)
        .unwrap();
    task_store
        .transition(&task.id, crate::task::model::TaskStatus::Completed, None)
        .unwrap();
    task_store
        .save_result(&task.id, "done", Vec::new())
        .unwrap();
    let event = task_store
        .list_events_after(&task.id, 0)
        .unwrap()
        .into_iter()
        .find(|event| event.event_type == crate::task::event::TaskEventType::Completed)
        .unwrap();

    let first = callback_service
        .callback_for_event(&task_store, &event)
        .unwrap()
        .unwrap();
    assert_eq!(first["type"], "task_callback");
    assert_eq!(first["event"], "task.completed");
    assert_eq!(first["remote_task_id"], "remote_task_1");
    assert_eq!(first["task_token"], "task-token-1");

    let duplicate = callback_service
        .callback_for_event(&task_store, &event)
        .unwrap();
    assert!(duplicate.is_none());
}

#[tokio::test]
async fn fake_control_plane_websocket_dispatches_and_receives_completion_callback() {
    let temp_dir = TempDir::new();
    let (app_state, grant, _task_store) = control_plane_test_app(&temp_dir);
    let daemon_state_store = crate::store::daemon_state::DaemonStateStore::open(StoreConfig::new(
        temp_dir.path().join("daemon-state.sqlite3"),
    ))
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (message_tx, mut message_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(16);
    let grant_id = grant.id.clone();

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();

        let register = socket.next().await.unwrap().unwrap();
        let register = match register {
            Message::Text(text) => serde_json::from_str::<serde_json::Value>(&text).unwrap(),
            other => panic!("unexpected register frame: {other:?}"),
        };
        message_tx.send(register).await.unwrap();

        socket
            .send(Message::Text(
                json!({
                    "type": "registration_accepted",
                    "registration": {
                        "daemon_id": "daemon_123",
                        "daemon_token": "daemon-token",
                        "session_id": "session_1",
                        "registered_at": "2026-06-07T00:00:00Z"
                    }
                })
                .to_string(),
            ))
            .await
            .unwrap();

        socket
            .send(Message::Text(
                json!({
                    "type": "task_dispatch",
                    "task": {
                        "remote_task_id": "remote_task_1",
                        "owner_product_id": "product",
                        "agent_id": "agent",
                        "directory_id": grant_id,
                        "prompt": "Do remote work",
                        "required_capabilities": ["read"],
                        "workspace_mode": "worktree",
                        "timeout_seconds": 45,
                        "task_token": "task-token-1",
                        "metadata": {"source": "control_plane"}
                    }
                })
                .to_string(),
            ))
            .await
            .unwrap();

        while let Some(frame) = socket.next().await {
            let frame = frame.unwrap();
            if let Message::Text(text) = frame {
                let payload = serde_json::from_str::<serde_json::Value>(&text).unwrap();
                message_tx.send(payload.clone()).await.unwrap();
                if payload["type"] == "task_callback" && payload["event"] == "task.completed" {
                    break;
                }
            }
        }
    });

    let config = ControlPlaneConfig {
        endpoint: Some(format!("ws://{addr}")),
        enrollment_secret: Some("enroll-secret".to_owned()),
        heartbeat_interval: Duration::from_secs(3600),
        ..ControlPlaneConfig::default()
    };
    let registration = crate::control_plane::registration::DaemonRegistrationService::new(
        config.clone(),
        daemon_state_store.clone(),
    );
    let handler = crate::control_plane::client::ControlPlaneMessageHandler::new(
        crate::control_plane::dispatch::ControlPlaneDispatchService::new(app_state.clone()),
        daemon_state_store,
    );
    let client = crate::control_plane::client::ControlPlaneClient::new(
        config,
        registration,
        handler,
        app_state.runtime_store().clone(),
        app_state.task_store().clone(),
        app_state.task_event_bus().clone(),
    );
    let client_run = tokio::spawn(async move { client.run().await.unwrap() });

    let register = tokio::time::timeout(Duration::from_secs(5), message_rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(register["type"], "register");
    assert_eq!(
        register["registration"]["enrollment_secret"],
        "enroll-secret"
    );

    let local_task_id = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let tasks = app_state.task_store().list(Default::default()).unwrap();
            if let Some(task) = tasks.into_iter().find(|task| {
                task.metadata
                    .as_ref()
                    .and_then(|metadata| metadata["control_plane"]["remote_task_id"].as_str())
                    == Some("remote_task_1")
            }) {
                break task.id;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();

    app_state
        .task_store()
        .transition(
            &local_task_id,
            crate::task::model::TaskStatus::WaitingDirectoryLock,
            None,
        )
        .unwrap();
    app_state
        .task_store()
        .transition(
            &local_task_id,
            crate::task::model::TaskStatus::Preparing,
            None,
        )
        .unwrap();
    app_state
        .task_store()
        .transition(
            &local_task_id,
            crate::task::model::TaskStatus::Running,
            None,
        )
        .unwrap();
    app_state
        .task_store()
        .transition(
            &local_task_id,
            crate::task::model::TaskStatus::Completed,
            None,
        )
        .unwrap();
    app_state
        .task_store()
        .save_result(&local_task_id, "done", Vec::new())
        .unwrap();

    let callback = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(payload) = message_rx.recv().await
                && payload["type"] == "task_callback"
            {
                break payload;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(callback["event"], "task.completed");
    assert_eq!(callback["remote_task_id"], "remote_task_1");
    assert_eq!(callback["task_token"], "task-token-1");

    client_run.abort();
    let _ = client_run.await;
    server.await.unwrap();
}
