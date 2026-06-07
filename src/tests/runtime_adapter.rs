use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use serde_json::{Value, json};

use crate::{
    agent::profile::{CreateAgentProfile, ExecutionPolicy, ProviderConfig},
    config::{SchedulerConfig, StoreConfig},
    registry::IntegrationType,
    runtime::{
        adapter::{AdapterSelector, RuntimeAdapter, RuntimeExecutionStatus},
        model::RuntimeView,
        store::RuntimeStore,
        template::{CommandTemplate, TemplateValues},
    },
    scheduler::{
        execution::{ExecutionError, SchedulerExecutionService},
        workspace::FakeWorkspacePreparer,
    },
    security::directory::{DirectoryCapability, DirectoryLockPolicy, WorkspaceMode},
    store::{
        agent_profiles::AgentProfileStore,
        directory_grants::{CreateDirectoryGrant, DirectoryGrantStore},
        tasks::TaskStore,
    },
    task::{
        event::{PermissionDecision, TaskEventType},
        model::CreateTask,
        model::TaskStatus,
        permission::PermissionResponseRequest,
        service::{TaskEventBus, TaskEventService},
    },
    tests::{TempDir, temp_registry_with_provider, valid_acp_manifest_json, valid_manifest_json},
};

#[test]
fn command_template_renders_known_values_and_rejects_unknown_variables() {
    let values = TemplateValues {
        prompt: Some("hello world".to_owned()),
        model: Some("test-model".to_owned()),
        workspace: Some("/tmp/work space".to_owned()),
        task_id: Some("task_1".to_owned()),
        agent_id: Some("agent".to_owned()),
        directory_id: Some("dir_1".to_owned()),
    };

    let rendered = CommandTemplate::render_args(
        &[
            "--prompt={{prompt}}".to_owned(),
            "--model".to_owned(),
            "{{model}}".to_owned(),
            "{{workspace}}".to_owned(),
        ],
        &values,
    )
    .unwrap();

    assert_eq!(
        rendered,
        [
            "--prompt=hello world",
            "--model",
            "test-model",
            "/tmp/work space"
        ]
    );
    assert!(
        CommandTemplate::render_args(&["{{unknown}}".to_owned()], &values)
            .unwrap_err()
            .code()
            == "command_render_failed"
    );
}

#[tokio::test]
async fn adapter_selection_gates_non_cli_integrations_without_spawning() {
    let selector = AdapterSelector::default();
    let mut manifest = manifest_with_execution("acp", &[], "stdin", &[]);
    manifest["integration_type"] = json!("cli");
    let cli: crate::registry::ProviderManifest = serde_json::from_value(manifest).unwrap();
    assert!(selector.for_manifest(&cli).is_ok());

    let acp: crate::registry::ProviderManifest =
        serde_json::from_value(valid_acp_manifest_json()).unwrap();
    assert!(selector.for_manifest(&acp).is_ok());

    for (integration_type, code) in [
        (IntegrationType::Http, "remote_execution_not_allowed"),
        (IntegrationType::Native, "adapter_not_implemented"),
    ] {
        let mut manifest = cli.clone();
        manifest.integration_type = integration_type;
        let error = selector.for_manifest(&manifest).unwrap_err();
        assert_eq!(error.code(), code);
    }
}

#[tokio::test]
async fn acp_adapter_selection_and_execution_normalizes_events() {
    let temp_dir = TempDir::new();
    let command = write_fake_command(
        temp_dir.path(),
        "test-acp-provider",
        fake_acp_provider_body(),
    );
    let manifest: crate::registry::ProviderManifest =
        serde_json::from_value(valid_acp_manifest_json()).unwrap();
    let selector = AdapterSelector::default();

    let adapter = selector.for_manifest(&manifest).unwrap();
    let outcome = adapter
        .execute(
            execution_request(
                temp_dir.path(),
                command,
                valid_acp_manifest_json(),
                "ACP prompt",
            )
            .with_runtime("rt_test_provider_local_acp"),
        )
        .await;

    assert_eq!(outcome.status, RuntimeExecutionStatus::Completed);
    assert_eq!(outcome.session_id.as_deref(), Some("acp-session-1"));
    assert!(outcome.events.iter().any(|event| {
        event.kind == TaskEventType::ProcessStdout
            && event.payload["text"].as_str() == Some("ACP prompt")
    }));
}

#[tokio::test]
async fn execution_service_runs_acp_task_and_persists_session_metadata() {
    let temp_dir = TempDir::new();
    let command = write_fake_command(
        temp_dir.path(),
        "test-acp-provider",
        fake_acp_provider_body(),
    );
    let state = acp_fixture(temp_dir.path(), command.clone());
    let task = enqueue_fixture_task(&state, temp_dir.path(), Some(5)).await;
    state
        .runtime_store
        .save(RuntimeView::available_with_kind(
            "test-provider",
            crate::runtime::model::RuntimeKind::LocalAcp,
            command,
            None,
        ))
        .await;

    let completed = execution_service(&state)
        .execute_task(&task.id)
        .await
        .unwrap();

    assert_eq!(completed.status, TaskStatus::Completed);
    assert_eq!(
        completed
            .result
            .as_ref()
            .and_then(|result| result.session_id.as_deref()),
        Some("acp-session-1")
    );
    let events = state.task_store.list_events(&task.id).unwrap();
    assert!(events.iter().any(|event| {
        event.event_type == TaskEventType::ProcessStdout
            && event.payload["text"].as_str() == Some("Do phase 6")
    }));
}

#[tokio::test]
async fn execution_service_bridges_acp_permission_requests_through_task_events() {
    let temp_dir = TempDir::new();
    let command = write_fake_command(
        temp_dir.path(),
        "test-acp-provider",
        fake_acp_permission_provider_body(),
    );
    let state = acp_fixture_with_event_bus(temp_dir.path(), command.clone());
    let task = enqueue_fixture_task(&state, temp_dir.path(), Some(5)).await;
    state
        .runtime_store
        .save(RuntimeView::available_with_kind(
            "test-provider",
            crate::runtime::model::RuntimeKind::LocalAcp,
            command,
            None,
        ))
        .await;

    let execution = execution_service(&state);
    let task_id = task.id.clone();
    let join = tokio::spawn(async move { execution.execute_task(&task_id).await.unwrap() });

    let pending = loop {
        match state.task_store.get_permission_request(&task.id, "perm_1") {
            Ok(request) => break request,
            Err(crate::store::tasks::TaskStoreError::PermissionRequestNotFound) => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(error) => panic!("unexpected permission request error: {error:?}"),
        }
    };
    assert_eq!(pending.request.permission_kind, "shell_command");

    let resolution = state
        .task_event_service
        .resolve_permission_response(
            &task.id,
            PermissionResponseRequest {
                request_id: "perm_1".to_owned(),
                decision: PermissionDecision::Approve,
                reason: Some("approved".to_owned()),
            },
        )
        .unwrap();
    assert_eq!(resolution.decision, PermissionDecision::Approve);

    let completed = join.await.unwrap();
    assert_eq!(completed.status, TaskStatus::Completed);
    let events = state.task_store.list_events(&task.id).unwrap();
    assert!(events.iter().any(|event| {
        event.event_type == TaskEventType::ProviderPermissionRequested
            && event.payload["request_id"].as_str() == Some("perm_1")
    }));
    assert!(events.iter().any(|event| {
        event.event_type == TaskEventType::ProviderPermissionDecided
            && event.payload["request_id"].as_str() == Some("perm_1")
    }));
}

#[tokio::test]
async fn cli_adapter_executes_arg_stdin_and_temp_file_modes_without_shell() {
    for (input_mode, args, expected) in [
        (
            "arg",
            vec!["--arg-prompt", "{{prompt}}"],
            "ARG:Prompt with spaces",
        ),
        ("stdin", vec!["--stdin-prompt"], "STDIN:Prompt with spaces"),
        (
            "temp_file",
            vec!["--file-prompt", "{{prompt}}"],
            "FILE:Prompt with spaces",
        ),
    ] {
        let temp_dir = TempDir::new();
        let command = write_fake_command(temp_dir.path(), "fake-provider", fake_provider_body());
        let request = execution_request(
            temp_dir.path(),
            command,
            manifest_with_execution(input_mode, &args, input_mode, &[]),
            "Prompt with spaces",
        );
        let adapter = crate::runtime::cli::LocalCliAdapter::new(Duration::from_secs(5));

        let outcome = adapter.execute(request).await;

        assert_eq!(outcome.status, RuntimeExecutionStatus::Completed);
        assert_eq!(outcome.exit_code, Some(0));
        assert!(
            outcome
                .events
                .iter()
                .any(|event| event.kind == TaskEventType::ProcessStdout
                    && event.payload["text"].as_str().unwrap().contains(expected)),
            "{outcome:#?}"
        );
        assert!(
            outcome
                .events
                .iter()
                .any(|event| event.kind == TaskEventType::ProcessStderr),
            "{outcome:#?}"
        );
    }
}

#[tokio::test]
async fn cli_adapter_removes_provider_secret_env_and_appends_custom_args() {
    let temp_dir = TempDir::new();
    let command = write_fake_command(temp_dir.path(), "fake-provider", fake_provider_body());
    let request = execution_request(
        temp_dir.path(),
        command,
        manifest_with_execution(
            "arg",
            &["--arg-prompt", "{{prompt}}"],
            "arg",
            &["SECRET_PROVIDER_TOKEN"],
        ),
        "secret check",
    )
    .with_custom_args(vec!["--flag-from-profile".to_owned()]);
    let adapter = crate::runtime::cli::LocalCliAdapter::new(Duration::from_secs(5));

    let outcome = adapter.execute(request).await;

    assert_eq!(outcome.status, RuntimeExecutionStatus::Completed);
    let stdout = merged_event_text(&outcome.events, TaskEventType::ProcessStdout);
    assert!(stdout.contains("CUSTOM_ARG:yes"));
    assert!(stdout.contains("SECRET_VISIBLE:no"));
}

#[tokio::test]
async fn cli_adapter_copies_custom_env_keys_only_when_explicitly_enabled() {
    let _guard = crate::tests::runtime_detection_test_guard().await;
    unsafe {
        std::env::set_var("OPENDAEMON_ALLOWED_TEST_ENV", "visible");
    }
    let temp_dir = TempDir::new();
    let command = write_fake_command(temp_dir.path(), "fake-provider", fake_provider_body());
    let adapter = crate::runtime::cli::LocalCliAdapter::new(Duration::from_secs(5));
    let disabled = execution_request(
        temp_dir.path(),
        command.clone(),
        manifest_with_execution("arg", &["--arg-prompt", "{{prompt}}"], "arg", &[]),
        "env check",
    )
    .with_custom_env_keys(vec!["OPENDAEMON_ALLOWED_TEST_ENV".to_owned()]);

    let disabled_outcome = adapter.execute(disabled).await;

    assert!(
        merged_event_text(&disabled_outcome.events, TaskEventType::ProcessStdout)
            .contains("ALLOWED_ENV:hidden")
    );

    let enabled = execution_request(
        temp_dir.path(),
        command,
        manifest_with_execution("arg", &["--arg-prompt", "{{prompt}}"], "arg", &[]),
        "env check",
    )
    .with_custom_env_keys(vec!["OPENDAEMON_ALLOWED_TEST_ENV".to_owned()])
    .with_agent_custom_env_enabled();

    let enabled_outcome = adapter.execute(enabled).await;

    unsafe {
        std::env::remove_var("OPENDAEMON_ALLOWED_TEST_ENV");
    }
    assert!(
        merged_event_text(&enabled_outcome.events, TaskEventType::ProcessStdout)
            .contains("ALLOWED_ENV:visible")
    );
}

#[tokio::test]
async fn execution_service_runs_cli_task_and_persists_result_events_and_lock_release() {
    let temp_dir = TempDir::new();
    let command = write_fake_command(temp_dir.path(), "fake-provider", fake_provider_body());
    let state = fixture(
        temp_dir.path(),
        command.clone(),
        "stdin",
        &["--stdin-prompt"],
        &[],
    );
    let task = enqueue_fixture_task(&state, temp_dir.path(), Some(5)).await;
    state
        .runtime_store
        .save(RuntimeView::available(
            "test-provider",
            command,
            Some("1.0".to_owned()),
        ))
        .await;
    let execution = execution_service(&state);

    let completed = execution.execute_task(&task.id).await.unwrap();

    assert_eq!(completed.status, TaskStatus::Completed);
    assert_eq!(
        completed.result.as_ref().unwrap().status,
        TaskStatus::Completed
    );
    assert!(
        completed
            .result
            .as_ref()
            .unwrap()
            .final_message
            .contains("completed")
    );
    let events = state.task_store.list_events(&task.id).unwrap();
    assert!(
        events
            .iter()
            .any(|event| event.event_type == TaskEventType::Running)
    );
    assert!(events.iter().any(|event| {
        event.event_type == TaskEventType::ProcessStdout
            && event.payload["text"]
                .as_str()
                .unwrap()
                .contains("STDIN:Do phase 6")
    }));
    assert!(
        state
            .task_store
            .active_locks(&task.directory_id)
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn execution_service_maps_non_zero_timeout_and_unavailable_runtime() {
    let temp_dir = TempDir::new();
    let failing = write_fake_command(temp_dir.path(), "failing-provider", "echo boom >&2; exit 7");
    let state = fixture(temp_dir.path(), failing.clone(), "stdin", &[], &[]);
    let task = enqueue_fixture_task(&state, temp_dir.path(), Some(5)).await;
    state
        .runtime_store
        .save(RuntimeView::available("test-provider", failing, None))
        .await;

    let failed = execution_service(&state)
        .execute_task(&task.id)
        .await
        .unwrap();
    assert_eq!(failed.status, TaskStatus::Failed);
    assert!(
        failed
            .result
            .as_ref()
            .unwrap()
            .error
            .as_ref()
            .unwrap()
            .contains("exit status")
    );

    let slow = write_fake_command(temp_dir.path(), "slow-provider", "sleep 5; echo too-late");
    let state = fixture(temp_dir.path(), slow.clone(), "stdin", &[], &[]);
    let task = enqueue_fixture_task(&state, temp_dir.path(), Some(1)).await;
    state
        .runtime_store
        .save(RuntimeView::available("test-provider", slow, None))
        .await;

    let timed_out = execution_service(&state)
        .execute_task(&task.id)
        .await
        .unwrap();
    assert_eq!(timed_out.status, TaskStatus::TimedOut);
    assert_eq!(
        timed_out.result.as_ref().unwrap().error.as_deref(),
        Some("command_timeout")
    );

    let state = fixture(
        temp_dir.path(),
        temp_dir.path().join("missing-provider"),
        "stdin",
        &[],
        &[],
    );
    let task = enqueue_fixture_task(&state, temp_dir.path(), Some(5)).await;
    let error = execution_service(&state)
        .execute_task(&task.id)
        .await
        .unwrap_err();
    assert!(matches!(error, ExecutionError::RuntimeUnavailable));
}

#[tokio::test]
async fn running_cli_task_can_be_cancelled_and_reaped() {
    let temp_dir = TempDir::new();
    let slow = write_fake_command(
        temp_dir.path(),
        "cancellable-provider",
        "echo started; sleep 30; echo too-late",
    );
    let state = fixture(temp_dir.path(), slow.clone(), "stdin", &[], &[]);
    let task = enqueue_fixture_task(&state, temp_dir.path(), Some(30)).await;
    state
        .runtime_store
        .save(RuntimeView::available("test-provider", slow, None))
        .await;
    let execution = execution_service(&state);
    let task_id = task.id.clone();
    let running = {
        let execution = execution.clone();
        tokio::spawn(async move { execution.execute_task(&task_id).await.unwrap() })
    };

    tokio::time::sleep(Duration::from_millis(200)).await;
    execution.cancel_running_task(&task.id).await.unwrap();
    let cancelled = running.await.unwrap();

    assert_eq!(cancelled.status, TaskStatus::Cancelled);
    assert_eq!(
        cancelled.result.as_ref().unwrap().error.as_deref(),
        Some("command_cancelled")
    );
    assert!(
        state
            .task_store
            .list_events(&task.id)
            .unwrap()
            .iter()
            .any(|event| event.event_type == TaskEventType::Cancelled)
    );
}

struct Fixture {
    _registry_temp: TempDir,
    providers_dir: PathBuf,
    runtime_store: RuntimeStore,
    agent_store: AgentProfileStore,
    directory_store: DirectoryGrantStore,
    task_store: TaskStore,
    task_event_service: TaskEventService,
}

fn execution_request(
    root: &Path,
    executable: PathBuf,
    manifest: Value,
    prompt: &str,
) -> crate::runtime::adapter::RuntimeExecutionRequest {
    let manifest = serde_json::from_value(manifest).unwrap();
    crate::runtime::adapter::RuntimeExecutionRequest {
        task_id: "task_1".to_owned(),
        provider_id: "test-provider".to_owned(),
        runtime_id: "rt_test_provider_local_cli".to_owned(),
        executable,
        manifest,
        agent_profile: CreateAgentProfile {
            id: "agent".to_owned(),
            name: "Agent".to_owned(),
            owner_product_id: "product".to_owned(),
            provider_id: "test-provider".to_owned(),
            model: "test-model".to_owned(),
            instructions: None,
            execution_policy: ExecutionPolicy::default(),
            provider_config: ProviderConfig::default(),
        }
        .into_profile(
            "2026-01-01T00:00:00Z".to_owned(),
            "2026-01-01T00:00:00Z".to_owned(),
        ),
        directory_grant: directory_grant(root),
        task: CreateTask {
            owner_product_id: "product".to_owned(),
            agent_id: "agent".to_owned(),
            directory_id: "dir_1".to_owned(),
            prompt: prompt.to_owned(),
            required_capabilities: Some(vec![DirectoryCapability::Read]),
            workspace_mode: Some(WorkspaceMode::Direct),
            direct_mode_task_opt_in: true,
            metadata: None,
            provider_id: Some("test-provider".to_owned()),
            model: Some("test-model".to_owned()),
            permission_mode: Some("provider_default".to_owned()),
            timeout_seconds: Some(5),
        }
        .into_test_task("task_1"),
        workspace: crate::scheduler::workspace::PreparedWorkspace {
            workspace_mode: WorkspaceMode::Direct,
            working_directory: root.to_path_buf(),
            source_directory_id: "dir_1".to_owned(),
            worktree_path: None,
            branch_name: None,
        },
        timeout_seconds: 5,
        allow_agent_custom_env: false,
        task_event_service: None,
    }
}

trait TestTaskExt {
    fn into_test_task(self, id: &str) -> crate::task::model::Task;
}

impl TestTaskExt for CreateTask {
    fn into_test_task(self, id: &str) -> crate::task::model::Task {
        let required_capabilities = self.required_capabilities();
        crate::task::model::Task {
            id: id.to_owned(),
            owner_product_id: self.owner_product_id,
            agent_id: self.agent_id,
            directory_id: self.directory_id,
            status: TaskStatus::Queued,
            required_capabilities,
            workspace_mode: self.workspace_mode.unwrap(),
            direct_mode_task_opt_in: self.direct_mode_task_opt_in,
            prompt: self.prompt,
            metadata: self.metadata,
            provider_id: self.provider_id.unwrap(),
            model: self.model.unwrap(),
            permission_mode: self.permission_mode.unwrap(),
            timeout_seconds: self.timeout_seconds,
            result: None,
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
            started_at: None,
            completed_at: None,
            cancelled_at: None,
            failed_at: None,
        }
    }
}

impl crate::runtime::adapter::RuntimeExecutionRequest {
    fn with_runtime(mut self, runtime_id: &str) -> Self {
        self.runtime_id = runtime_id.to_owned();
        self
    }

    fn with_custom_args(mut self, custom_args: Vec<String>) -> Self {
        self.agent_profile.provider_config.custom_args = custom_args;
        self
    }

    fn with_custom_env_keys(mut self, custom_env_keys: Vec<String>) -> Self {
        self.agent_profile.provider_config.custom_env_keys = custom_env_keys;
        self
    }

    fn with_agent_custom_env_enabled(mut self) -> Self {
        self.allow_agent_custom_env = true;
        self
    }
}

fn fake_acp_provider_body() -> &'static str {
    r#"
printf '{"type":"session.started","session_id":"acp-session-1"}\n'
read input
printf '{"type":"message.delta","text":"%s"}\n' "$input"
printf '{"type":"session.completed"}\n'
"#
}

fn fake_acp_permission_provider_body() -> &'static str {
    r#"
printf '{"type":"session.started","session_id":"acp-session-1"}\n'
read input
printf '{"type":"permission.requested","request_id":"perm_1","permission_kind":"shell_command","summary":"run git push","details":{"command":["git","push"]}}\n'
read decision
printf '{"type":"message.delta","text":"%s"}\n' "$input"
printf '{"type":"session.completed"}\n'
"#
}

fn fixture(
    root: &Path,
    executable: PathBuf,
    input_mode: &str,
    args: &[&str],
    secret_keys: &[&str],
) -> Fixture {
    let (registry_temp, providers_dir) = temp_registry_with_provider(
        "test-provider",
        manifest_with_execution(input_mode, args, input_mode, secret_keys),
    );
    let store_config = StoreConfig::new(root.join(format!("{}.sqlite3", unique_name())));
    let agent_store = AgentProfileStore::open(store_config.clone()).unwrap();
    let directory_store = DirectoryGrantStore::open(store_config.clone()).unwrap();
    let event_bus = std::sync::Arc::new(TaskEventBus::default());
    let task_store = TaskStore::open(store_config)
        .unwrap()
        .with_event_bus(event_bus.clone());
    let task_event_service =
        TaskEventService::new(task_store.clone(), event_bus, Duration::from_secs(1));
    agent_store
        .create(CreateAgentProfile {
            id: "agent".to_owned(),
            name: "Agent".to_owned(),
            owner_product_id: "product".to_owned(),
            provider_id: "test-provider".to_owned(),
            model: "test-model".to_owned(),
            instructions: None,
            execution_policy: ExecutionPolicy {
                default_workspace_mode: crate::agent::profile::WorkspaceMode::Direct,
                allow_direct_directory: true,
            },
            provider_config: ProviderConfig::default(),
        })
        .unwrap();
    fs::create_dir_all(root.join("project/.git")).unwrap();
    directory_store
        .create(CreateDirectoryGrant {
            product_id: "product".to_owned(),
            agent_id: "agent".to_owned(),
            path: root.join("project"),
            capabilities: vec![DirectoryCapability::Read],
            workspace_modes: Some(vec![WorkspaceMode::Direct]),
            default_workspace_mode: Some(WorkspaceMode::Direct),
            lock_policy: Some(DirectoryLockPolicy::Shared),
            direct_mode_requires_explicit_task_opt_in: Some(true),
        })
        .unwrap();
    assert!(executable.exists() || executable.ends_with("missing-provider"));
    Fixture {
        _registry_temp: registry_temp,
        providers_dir,
        runtime_store: RuntimeStore::default(),
        agent_store,
        directory_store,
        task_store,
        task_event_service,
    }
}

fn acp_fixture(root: &Path, executable: PathBuf) -> Fixture {
    let (registry_temp, providers_dir) =
        temp_registry_with_provider("test-provider", valid_acp_manifest_json());
    let store_config = StoreConfig::new(root.join(format!("{}.sqlite3", unique_name())));
    let agent_store = AgentProfileStore::open(store_config.clone()).unwrap();
    let directory_store = DirectoryGrantStore::open(store_config.clone()).unwrap();
    let event_bus = std::sync::Arc::new(TaskEventBus::default());
    let task_store = TaskStore::open(store_config)
        .unwrap()
        .with_event_bus(event_bus.clone());
    let task_event_service =
        TaskEventService::new(task_store.clone(), event_bus, Duration::from_secs(1));
    agent_store
        .create(CreateAgentProfile {
            id: "agent".to_owned(),
            name: "Agent".to_owned(),
            owner_product_id: "product".to_owned(),
            provider_id: "test-provider".to_owned(),
            model: "test-model".to_owned(),
            instructions: None,
            execution_policy: ExecutionPolicy {
                default_workspace_mode: crate::agent::profile::WorkspaceMode::Direct,
                allow_direct_directory: true,
            },
            provider_config: ProviderConfig::default(),
        })
        .unwrap();
    fs::create_dir_all(root.join("project/.git")).unwrap();
    directory_store
        .create(CreateDirectoryGrant {
            product_id: "product".to_owned(),
            agent_id: "agent".to_owned(),
            path: root.join("project"),
            capabilities: vec![DirectoryCapability::Read],
            workspace_modes: Some(vec![WorkspaceMode::Direct]),
            default_workspace_mode: Some(WorkspaceMode::Direct),
            lock_policy: Some(DirectoryLockPolicy::Shared),
            direct_mode_requires_explicit_task_opt_in: Some(true),
        })
        .unwrap();
    assert!(executable.exists());
    Fixture {
        _registry_temp: registry_temp,
        providers_dir,
        runtime_store: RuntimeStore::default(),
        agent_store,
        directory_store,
        task_store,
        task_event_service,
    }
}

fn acp_fixture_with_event_bus(root: &Path, executable: PathBuf) -> Fixture {
    acp_fixture(root, executable)
}

async fn enqueue_fixture_task(
    state: &Fixture,
    _root: &Path,
    timeout: Option<u64>,
) -> crate::task::model::Task {
    let grant = state
        .directory_store
        .list(Default::default())
        .unwrap()
        .remove(0);
    crate::scheduler::service::SchedulerService::new(
        state.task_store.clone(),
        state.agent_store.clone(),
        state.directory_store.clone(),
        SchedulerConfig::default(),
    )
    .enqueue_task(CreateTask {
        owner_product_id: "product".to_owned(),
        agent_id: "agent".to_owned(),
        directory_id: grant.id,
        prompt: "Do phase 6".to_owned(),
        required_capabilities: Some(vec![DirectoryCapability::Read]),
        workspace_mode: Some(WorkspaceMode::Direct),
        direct_mode_task_opt_in: true,
        metadata: None,
        provider_id: None,
        model: None,
        permission_mode: None,
        timeout_seconds: timeout,
    })
    .unwrap()
}

fn execution_service(state: &Fixture) -> SchedulerExecutionService<FakeWorkspacePreparer> {
    SchedulerExecutionService::new(
        state.providers_dir.clone(),
        state.runtime_store.clone(),
        state.task_store.clone(),
        state.agent_store.clone(),
        state.directory_store.clone(),
        SchedulerConfig::default(),
        FakeWorkspacePreparer::new(state.providers_dir.join("workspaces")),
    )
}

fn directory_grant(root: &Path) -> crate::security::directory::DirectoryGrant {
    DirectoryGrantStore::open(StoreConfig::new(
        root.join(format!("{}.sqlite3", unique_name())),
    ))
    .unwrap()
    .create(CreateDirectoryGrant {
        product_id: "product".to_owned(),
        agent_id: "agent".to_owned(),
        path: root.to_path_buf(),
        capabilities: vec![DirectoryCapability::Read],
        workspace_modes: Some(vec![WorkspaceMode::Direct]),
        default_workspace_mode: Some(WorkspaceMode::Direct),
        lock_policy: Some(DirectoryLockPolicy::Shared),
        direct_mode_requires_explicit_task_opt_in: Some(true),
    })
    .unwrap()
}

fn manifest_with_execution(
    command_name: &str,
    args: &[&str],
    input_mode: &str,
    secret_keys: &[&str],
) -> Value {
    let mut manifest = valid_manifest_json();
    manifest["execution"]["command"] = json!(command_name);
    manifest["execution"]["args"] = json!(args);
    manifest["execution"]["input_mode"] = json!(input_mode);
    manifest["environment"]["required"] = json!(secret_keys);
    manifest
}

fn merged_event_text(
    events: &[crate::runtime::adapter::RuntimeOutputEvent],
    event_type: TaskEventType,
) -> String {
    events
        .iter()
        .filter(|event| event.kind == event_type)
        .filter_map(|event| event.payload["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn write_fake_command(root: &Path, name: &str, body: &str) -> PathBuf {
    let path = root.join(command_file_name(name));
    fs::write(&path, script_contents(body)).unwrap();
    make_executable(&path);
    path
}

fn command_file_name(name: &str) -> String {
    #[cfg(windows)]
    {
        format!("{name}.cmd")
    }
    #[cfg(not(windows))]
    {
        name.to_owned()
    }
}

fn script_contents(body: &str) -> String {
    #[cfg(windows)]
    {
        format!("@echo off\r\n{body}\r\n")
    }
    #[cfg(not(windows))]
    {
        format!("#!/bin/sh\n{body}\n")
    }
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

fn fake_provider_body() -> &'static str {
    r#"
echo "PWD:$(pwd)"
echo "ERR:diagnostic" >&2
case "$1" in
  --arg-prompt)
    echo "ARG:$2"
    ;;
  --stdin-prompt)
    read input
    echo "STDIN:$input"
    ;;
  --file-prompt)
    echo "FILE:$(cat "$2")"
    ;;
esac
if [ "$3" = "--flag-from-profile" ] || [ "$2" = "--flag-from-profile" ]; then
  echo "CUSTOM_ARG:yes"
fi
if [ -n "$SECRET_PROVIDER_TOKEN" ]; then
  echo "SECRET_VISIBLE:yes"
else
  echo "SECRET_VISIBLE:no"
fi
if [ -n "$OPENDAEMON_ALLOWED_TEST_ENV" ]; then
  echo "ALLOWED_ENV:$OPENDAEMON_ALLOWED_TEST_ENV"
else
  echo "ALLOWED_ENV:hidden"
fi
"#
}

fn unique_name() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    NEXT_ID.fetch_add(1, Ordering::Relaxed).to_string()
}
