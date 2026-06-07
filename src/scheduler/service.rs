use crate::{
    config::SchedulerConfig,
    security::directory::{DirectoryAuthorizationRequest, DirectorySecurityError, WorkspaceMode},
    store::{
        agent_profiles::{AgentProfileStore, AgentStoreError},
        directory_grants::{DirectoryGrantStore, DirectoryStoreError},
        tasks::{TaskStore, TaskStoreError},
    },
    task::model::{CreateTask, Task, TaskModelError, TaskStatus},
};

use super::locks::{LockDecision, LockMode, LockRequest, mode_for_capabilities};
use super::workspace::{PreparedWorkspace, WorkspacePreparer};

#[derive(Debug, Clone)]
pub struct SchedulerService {
    task_store: TaskStore,
    agent_profile_store: AgentProfileStore,
    directory_grant_store: DirectoryGrantStore,
    config: SchedulerConfig,
}

#[derive(Debug)]
pub enum TaskValidationError {
    TaskNotFound,
    InvalidTask,
    InvalidTaskPrompt,
    InvalidTaskState,
    AgentNotFound,
    DirectoryNotFound,
    AgentAuthorizationFailed,
    DirectoryAuthorizationFailed,
    CapabilityNotAllowed,
    WorkspaceModeNotAllowed,
    DirectModeNotAllowed,
    ProviderOverrideNotAllowed,
    ModelOverrideNotAllowed,
    PermissionModeOverrideNotAllowed,
    DirectoryLockConflict,
    TaskAlreadyTerminal,
    Store(anyhow::Error),
}

impl SchedulerService {
    #[must_use]
    pub const fn new(
        task_store: TaskStore,
        agent_profile_store: AgentProfileStore,
        directory_grant_store: DirectoryGrantStore,
        config: SchedulerConfig,
    ) -> Self {
        Self {
            task_store,
            agent_profile_store,
            directory_grant_store,
            config,
        }
    }

    pub fn enqueue_task(&self, input: CreateTask) -> Result<Task, TaskValidationError> {
        input.validate().map_err(map_model_error)?;
        let grant = self
            .directory_grant_store
            .get(&input.directory_id)
            .map_err(map_directory_error)?;
        let requested_workspace_mode =
            select_workspace_mode(input.workspace_mode, &grant, input.direct_mode_task_opt_in)?;
        let profile = self
            .agent_profile_store
            .get(&input.agent_id)
            .map_err(map_agent_error)?;
        if profile.owner_product_id != input.owner_product_id {
            return Err(TaskValidationError::AgentAuthorizationFailed);
        }
        map_override_error(
            input.provider_id.as_ref(),
            &profile.provider_id,
            OverrideKind::Provider,
        )?;
        map_override_error(input.model.as_ref(), &profile.model, OverrideKind::Model)?;
        map_override_error(
            input.permission_mode.as_ref(),
            &profile.provider_config.permission_mode,
            OverrideKind::PermissionMode,
        )?;
        if requested_workspace_mode == WorkspaceMode::Direct
            && !profile.execution_policy.allow_direct_directory
        {
            return Err(TaskValidationError::DirectModeNotAllowed);
        }
        if input
            .required_capabilities()
            .iter()
            .any(|capability| !grant.capabilities.contains(capability))
        {
            return Err(TaskValidationError::CapabilityNotAllowed);
        }
        self.directory_grant_store
            .authorize(&DirectoryAuthorizationRequest {
                product_id: input.owner_product_id.clone(),
                agent_id: input.agent_id.clone(),
                directory_id: input.directory_id.clone(),
                required_capabilities: input.required_capabilities(),
                requested_workspace_mode,
                direct_mode_task_opt_in: input.direct_mode_task_opt_in,
            })
            .map_err(map_directory_error)?;

        let task = self.task_store.create(CreateTask {
            workspace_mode: Some(requested_workspace_mode),
            provider_id: Some(profile.provider_id),
            model: Some(profile.model),
            permission_mode: Some(profile.provider_config.permission_mode),
            ..input
        })?;
        Ok(task)
    }

    pub fn try_acquire_directory_lock(
        &self,
        request: &LockRequest,
    ) -> Result<LockDecision, TaskValidationError> {
        let task = self.task_store.get(&request.task_id)?;
        if self.running_or_preparing_count()? >= self.config.max_concurrent_tasks {
            self.task_store.transition(
                &task.id,
                TaskStatus::WaitingDirectoryLock,
                Some(serde_json::json!({"reason": "global_capacity"})),
            )?;
            return Ok(LockDecision::Waiting);
        }
        let grant = self
            .directory_grant_store
            .get(&request.directory_id)
            .map_err(map_directory_error)?;
        let mode = mode_for_capabilities(&task.required_capabilities, grant.lock_policy);
        if mode == LockMode::None {
            return Ok(LockDecision::NotRequired);
        }
        let acquired = self.task_store.acquire_lock(&LockRequest {
            mode,
            ..request.clone()
        })?;
        if acquired {
            self.task_store.transition(
                &task.id,
                TaskStatus::WaitingDirectoryLock,
                Some(serde_json::json!({"reason": "directory_lock_acquired"})),
            )?;
            Ok(LockDecision::Acquired)
        } else {
            self.task_store.transition(
                &task.id,
                TaskStatus::WaitingDirectoryLock,
                Some(serde_json::json!({"reason": "directory_lock"})),
            )?;
            Ok(LockDecision::Waiting)
        }
    }

    pub fn mark_preparing(&self, task_id: &str) -> Result<Task, TaskValidationError> {
        Ok(self
            .task_store
            .transition(task_id, TaskStatus::Preparing, None)?)
    }

    pub fn prepare_workspace<P: WorkspacePreparer>(
        &self,
        task_id: &str,
        preparer: &P,
    ) -> Result<PreparedWorkspace, TaskValidationError> {
        let task = self.task_store.get(task_id)?;
        let grant = self
            .directory_grant_store
            .get(&task.directory_id)
            .map_err(map_directory_error)?;
        if task.status == TaskStatus::WaitingDirectoryLock {
            self.mark_preparing(task_id)?;
        }
        match preparer.prepare(&task.id, &grant, task.workspace_mode) {
            Ok(workspace) => Ok(workspace),
            Err(_) => {
                self.fail_task(task_id, "workspace preparation failed")?;
                Err(TaskValidationError::WorkspaceModeNotAllowed)
            }
        }
    }

    pub fn mark_running(&self, task_id: &str) -> Result<Task, TaskValidationError> {
        Ok(self
            .task_store
            .transition(task_id, TaskStatus::Running, None)?)
    }

    pub fn complete_task(
        &self,
        task_id: &str,
        final_message: &str,
    ) -> Result<Task, TaskValidationError> {
        let task = self
            .task_store
            .transition(task_id, TaskStatus::Completed, None)?;
        self.task_store
            .save_result(task_id, final_message, Vec::new())?;
        Ok(task)
    }

    pub fn fail_task(&self, task_id: &str, error: &str) -> Result<Task, TaskValidationError> {
        Ok(self.task_store.transition(
            task_id,
            TaskStatus::Failed,
            Some(serde_json::json!({ "error": error })),
        )?)
    }

    pub fn cancel_task(&self, task_id: &str) -> Result<Task, TaskValidationError> {
        let task = self.task_store.get(task_id)?;
        if matches!(
            task.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::TimedOut
        ) {
            return Err(TaskValidationError::TaskAlreadyTerminal);
        }
        Ok(self.task_store.cancel(task_id)?)
    }

    pub fn release_locks(&self, task_id: &str) -> Result<(), TaskValidationError> {
        Ok(self.task_store.release_locks(task_id)?)
    }

    fn running_or_preparing_count(&self) -> Result<usize, TaskValidationError> {
        Ok(self
            .task_store
            .list(Default::default())?
            .into_iter()
            .filter(|task| matches!(task.status, TaskStatus::Preparing | TaskStatus::Running))
            .count())
    }
}

fn select_workspace_mode(
    requested: Option<WorkspaceMode>,
    grant: &crate::security::directory::DirectoryGrant,
    direct_mode_task_opt_in: bool,
) -> Result<WorkspaceMode, TaskValidationError> {
    let mode = requested.unwrap_or(grant.default_workspace_mode);
    if !grant.workspace_modes.contains(&mode) {
        return Err(match mode {
            WorkspaceMode::Direct => TaskValidationError::DirectModeNotAllowed,
            WorkspaceMode::Worktree => TaskValidationError::WorkspaceModeNotAllowed,
        });
    }
    if mode == WorkspaceMode::Direct
        && grant.direct_mode_requires_explicit_task_opt_in
        && !direct_mode_task_opt_in
    {
        return Err(TaskValidationError::DirectModeNotAllowed);
    }
    Ok(mode)
}

enum OverrideKind {
    Provider,
    Model,
    PermissionMode,
}

fn map_override_error(
    requested: Option<&String>,
    actual: &str,
    kind: OverrideKind,
) -> Result<(), TaskValidationError> {
    if requested.is_some_and(|requested| requested != actual) {
        return Err(match kind {
            OverrideKind::Provider => TaskValidationError::ProviderOverrideNotAllowed,
            OverrideKind::Model => TaskValidationError::ModelOverrideNotAllowed,
            OverrideKind::PermissionMode => TaskValidationError::PermissionModeOverrideNotAllowed,
        });
    }
    Ok(())
}

fn map_model_error(error: TaskModelError) -> TaskValidationError {
    match error {
        TaskModelError::InvalidPrompt => TaskValidationError::InvalidTaskPrompt,
        TaskModelError::InvalidTask => TaskValidationError::InvalidTask,
    }
}

fn map_agent_error(error: AgentStoreError) -> TaskValidationError {
    match error {
        AgentStoreError::Profile(crate::agent::profile::AgentProfileError::AgentNotFound) => {
            TaskValidationError::AgentNotFound
        }
        AgentStoreError::Profile(_) => TaskValidationError::AgentAuthorizationFailed,
        AgentStoreError::Store(error) => TaskValidationError::Store(error),
    }
}

fn map_directory_error(error: DirectoryStoreError) -> TaskValidationError {
    match error {
        DirectoryStoreError::NotFound => TaskValidationError::DirectoryNotFound,
        DirectoryStoreError::Security(DirectorySecurityError::DirectModeNotAllowed) => {
            TaskValidationError::DirectModeNotAllowed
        }
        DirectoryStoreError::Security(DirectorySecurityError::AuthorizationFailed) => {
            TaskValidationError::DirectoryAuthorizationFailed
        }
        DirectoryStoreError::Security(DirectorySecurityError::InvalidCapability) => {
            TaskValidationError::CapabilityNotAllowed
        }
        DirectoryStoreError::Security(_) => TaskValidationError::WorkspaceModeNotAllowed,
        DirectoryStoreError::Path(_) => TaskValidationError::DirectoryAuthorizationFailed,
        DirectoryStoreError::Store(error) => TaskValidationError::Store(error),
    }
}

impl From<TaskStoreError> for TaskValidationError {
    fn from(error: TaskStoreError) -> Self {
        match error {
            TaskStoreError::NotFound => Self::TaskNotFound,
            TaskStoreError::PermissionRequestNotFound
            | TaskStoreError::PermissionRequestNotPending
            | TaskStoreError::PermissionRequestAlreadyResolved => Self::InvalidTaskState,
            TaskStoreError::Model(error) => map_model_error(error),
            TaskStoreError::State(crate::task::state::TaskStateError::AlreadyTerminal) => {
                Self::TaskAlreadyTerminal
            }
            TaskStoreError::State(_) => Self::InvalidTaskState,
            TaskStoreError::Store(error) => Self::Store(error),
        }
    }
}
