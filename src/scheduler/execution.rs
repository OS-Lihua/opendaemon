use std::path::PathBuf;

use serde_json::json;

use crate::{
    config::SchedulerConfig,
    registry::{self, IntegrationType},
    runtime::{
        adapter::{AdapterSelector, RuntimeAdapter, RuntimeAdapterError, RuntimeExecutionOutcome},
        model::{RuntimeStatus, RuntimeView},
        store::RuntimeStore,
    },
    scheduler::{
        locks::{LockDecision, LockRequest},
        service::{SchedulerService, TaskValidationError},
        workspace::WorkspacePreparer,
    },
    store::{
        agent_profiles::AgentProfileStore, directory_grants::DirectoryGrantStore, tasks::TaskStore,
    },
    task::{model::Task, result::TaskResult},
};

const DEFAULT_TASK_TIMEOUT_SECONDS: u64 = 300;

#[derive(Debug, Clone)]
pub struct SchedulerExecutionService<P> {
    providers_dir: PathBuf,
    runtime_store: RuntimeStore,
    task_store: TaskStore,
    agent_profile_store: AgentProfileStore,
    directory_grant_store: DirectoryGrantStore,
    scheduler_config: SchedulerConfig,
    workspace_preparer: P,
    adapter_selector: AdapterSelector,
}

#[derive(Debug)]
pub enum ExecutionError {
    RuntimeUnavailable,
    Adapter(RuntimeAdapterError),
    Task(TaskValidationError),
    Registry(anyhow::Error),
    Store(anyhow::Error),
}

impl<P> SchedulerExecutionService<P>
where
    P: WorkspacePreparer + Clone + Send + Sync + 'static,
{
    #[must_use]
    pub fn new(
        providers_dir: PathBuf,
        runtime_store: RuntimeStore,
        task_store: TaskStore,
        agent_profile_store: AgentProfileStore,
        directory_grant_store: DirectoryGrantStore,
        scheduler_config: SchedulerConfig,
        workspace_preparer: P,
    ) -> Self {
        Self {
            providers_dir,
            runtime_store,
            task_store,
            agent_profile_store,
            directory_grant_store,
            scheduler_config,
            workspace_preparer,
            adapter_selector: AdapterSelector::default(),
        }
    }

    pub async fn execute_task(&self, task_id: &str) -> Result<Task, ExecutionError> {
        let task = self
            .task_store
            .get(task_id)
            .map_err(TaskValidationError::from)?;
        if task.status.is_terminal() {
            return Err(ExecutionError::Task(
                TaskValidationError::TaskAlreadyTerminal,
            ));
        }
        let profile = self
            .agent_profile_store
            .get(&task.agent_id)
            .map_err(map_agent_store)?;
        let grant = self
            .directory_grant_store
            .get(&task.directory_id)
            .map_err(map_directory_store)?;
        let registry = registry::load_registry_from_dir(&self.providers_dir)
            .map_err(ExecutionError::Registry)?;
        let manifest = registry
            .get(&profile.provider_id)
            .ok_or(ExecutionError::RuntimeUnavailable)?
            .manifest
            .clone();
        if manifest.integration_type != IntegrationType::Cli {
            return Err(ExecutionError::Adapter(
                self.adapter_selector.for_manifest(&manifest).unwrap_err(),
            ));
        }
        let runtime = self
            .runtime_store
            .get(&profile.provider_id)
            .await
            .filter(runtime_available)
            .ok_or(ExecutionError::RuntimeUnavailable)?;
        let executable = runtime
            .executable
            .clone()
            .ok_or(ExecutionError::RuntimeUnavailable)?;

        let scheduler = SchedulerService::new(
            self.task_store.clone(),
            self.agent_profile_store.clone(),
            self.directory_grant_store.clone(),
            self.scheduler_config,
        );
        match scheduler.try_acquire_directory_lock(&LockRequest::from_task(&task))? {
            LockDecision::Acquired | LockDecision::NotRequired => {}
            LockDecision::Waiting => {
                return Err(ExecutionError::Task(
                    TaskValidationError::DirectoryLockConflict,
                ));
            }
        }
        let workspace = scheduler.prepare_workspace(task_id, &self.workspace_preparer)?;
        scheduler.mark_running(task_id)?;

        let adapter = self.adapter_selector.for_manifest(&manifest)?;
        let outcome = adapter
            .execute(crate::runtime::adapter::RuntimeExecutionRequest {
                task_id: task.id.clone(),
                provider_id: profile.provider_id.clone(),
                runtime_id: runtime.id,
                executable,
                manifest,
                agent_profile: profile,
                directory_grant: grant,
                task: self
                    .task_store
                    .get(task_id)
                    .map_err(TaskValidationError::from)?,
                workspace: workspace.clone(),
                timeout_seconds: task.timeout_seconds.unwrap_or(DEFAULT_TASK_TIMEOUT_SECONDS),
                allow_agent_custom_env: self.scheduler_config.allow_agent_custom_env,
            })
            .await;

        self.persist_outcome(task_id, outcome, &workspace)
    }

    pub async fn cancel_running_task(&self, task_id: &str) -> Result<(), ExecutionError> {
        let task = self
            .task_store
            .get(task_id)
            .map_err(TaskValidationError::from)?;
        let profile = self
            .agent_profile_store
            .get(&task.agent_id)
            .map_err(map_agent_store)?;
        let registry = registry::load_registry_from_dir(&self.providers_dir)
            .map_err(ExecutionError::Registry)?;
        let manifest = registry
            .get(&profile.provider_id)
            .ok_or(ExecutionError::RuntimeUnavailable)?
            .manifest
            .clone();
        let adapter = self.adapter_selector.for_manifest(&manifest)?;
        let _ = adapter.cancel(task_id).await;
        Ok(())
    }

    fn persist_outcome(
        &self,
        task_id: &str,
        outcome: RuntimeExecutionOutcome,
        workspace: &crate::scheduler::workspace::PreparedWorkspace,
    ) -> Result<Task, ExecutionError> {
        for event in &outcome.events {
            self.task_store
                .append_event(task_id, event.kind, event.payload.clone())
                .map_err(TaskValidationError::from)?;
        }
        let terminal = outcome.status.task_status();
        self.task_store
            .transition(
                task_id,
                terminal,
                outcome.error.as_ref().map(|error| {
                    json!({
                        "error": error.code(),
                        "message": error.message()
                    })
                }),
            )
            .map_err(TaskValidationError::from)?;
        let task = self
            .task_store
            .get(task_id)
            .map_err(TaskValidationError::from)?;
        self.task_store
            .save_execution_result(&result_from_outcome(&task, &outcome, workspace))
            .map_err(TaskValidationError::from)?;
        self.task_store
            .get(task_id)
            .map_err(TaskValidationError::from)
            .map_err(Into::into)
    }
}

fn runtime_available(runtime: &RuntimeView) -> bool {
    runtime.status == RuntimeStatus::Available && runtime.executable.is_some()
}

fn result_from_outcome(
    task: &Task,
    outcome: &RuntimeExecutionOutcome,
    workspace: &crate::scheduler::workspace::PreparedWorkspace,
) -> TaskResult {
    let now =
        crate::agent::profile::now_rfc3339().unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned());
    TaskResult {
        task_id: task.id.clone(),
        status: task.status,
        final_message: outcome.final_message.clone(),
        changed_files: outcome.changed_files.clone(),
        diff: outcome.diff.clone(),
        workspace_mode: task.workspace_mode,
        worktree_path: workspace
            .worktree_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        source_directory_id: task.directory_id.clone(),
        branch_name: workspace.branch_name.clone(),
        commit_hash: None,
        session_id: outcome.session_id.clone(),
        provider_result: outcome.provider_result.clone(),
        usage: outcome.usage.clone(),
        artifacts: Vec::new(),
        error: outcome.error.as_ref().map(|error| {
            if matches!(error.code(), "command_timeout" | "command_cancelled") {
                error.code().to_owned()
            } else {
                error.message().to_owned()
            }
        }),
        created_at: now.clone(),
        updated_at: now,
    }
}

fn map_agent_store(error: crate::store::agent_profiles::AgentStoreError) -> ExecutionError {
    ExecutionError::Task(TaskValidationError::from(
        crate::store::tasks::TaskStoreError::Store(anyhow::anyhow!("{error:?}")),
    ))
}

fn map_directory_store(
    error: crate::store::directory_grants::DirectoryStoreError,
) -> ExecutionError {
    ExecutionError::Task(TaskValidationError::from(
        crate::store::tasks::TaskStoreError::Store(anyhow::anyhow!("{error:?}")),
    ))
}

impl From<TaskValidationError> for ExecutionError {
    fn from(error: TaskValidationError) -> Self {
        Self::Task(error)
    }
}

impl From<RuntimeAdapterError> for ExecutionError {
    fn from(error: RuntimeAdapterError) -> Self {
        Self::Adapter(error)
    }
}
