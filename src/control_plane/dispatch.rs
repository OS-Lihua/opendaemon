use serde_json::json;

use crate::{
    api::AppState,
    scheduler::service::SchedulerService,
    security::directory::{DirectoryCapability, WorkspaceMode},
    task::{model::CreateTask, model::Task},
};

use super::protocol::RemoteDispatchTask;

#[derive(Debug, Clone)]
pub struct ControlPlaneDispatchService {
    state: AppState,
}

#[derive(Debug)]
pub enum ControlPlaneDispatchError {
    InvalidDispatch,
    Task(crate::scheduler::service::TaskValidationError),
}

impl ControlPlaneDispatchService {
    #[must_use]
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn ingest(
        &self,
        remote_task: RemoteDispatchTask,
    ) -> Result<Task, ControlPlaneDispatchError> {
        if let Some(existing) = self.find_by_remote_task_id(&remote_task.remote_task_id)? {
            return Ok(existing);
        }
        let required_capabilities = remote_task
            .required_capabilities
            .iter()
            .map(|capability| match capability.as_str() {
                "read" => Ok(DirectoryCapability::Read),
                "write" => Ok(DirectoryCapability::Write),
                "shell" => Ok(DirectoryCapability::Shell),
                "git" => Ok(DirectoryCapability::Git),
                _ => Err(ControlPlaneDispatchError::InvalidDispatch),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let workspace_mode = match remote_task.workspace_mode.as_str() {
            "worktree" => WorkspaceMode::Worktree,
            "direct" => WorkspaceMode::Direct,
            _ => return Err(ControlPlaneDispatchError::InvalidDispatch),
        };

        let metadata = json!({
            "control_plane": {
                "remote_task_id": remote_task.remote_task_id,
                "task_token": remote_task.task_token,
                "source": "control_plane"
            },
            "upstream": remote_task.metadata
        });

        SchedulerService::new(
            self.state.task_store().clone(),
            self.state.agent_profile_store().clone(),
            self.state.directory_grant_store().clone(),
            self.state.scheduler_config(),
        )
        .enqueue_task(CreateTask {
            owner_product_id: remote_task.owner_product_id,
            agent_id: remote_task.agent_id,
            directory_id: remote_task.directory_id,
            prompt: remote_task.prompt,
            required_capabilities: Some(required_capabilities),
            workspace_mode: Some(workspace_mode),
            direct_mode_task_opt_in: workspace_mode == WorkspaceMode::Direct,
            metadata: Some(metadata),
            provider_id: None,
            model: None,
            permission_mode: None,
            timeout_seconds: remote_task.timeout_seconds,
        })
        .map_err(ControlPlaneDispatchError::Task)
    }

    pub fn cancel_remote_task(
        &self,
        remote_task_id: &str,
    ) -> Result<Task, ControlPlaneDispatchError> {
        let Some(task) = self.find_by_remote_task_id(remote_task_id)? else {
            return Err(ControlPlaneDispatchError::InvalidDispatch);
        };
        SchedulerService::new(
            self.state.task_store().clone(),
            self.state.agent_profile_store().clone(),
            self.state.directory_grant_store().clone(),
            self.state.scheduler_config(),
        )
        .cancel_task(&task.id)
        .map_err(ControlPlaneDispatchError::Task)
    }

    fn find_by_remote_task_id(
        &self,
        remote_task_id: &str,
    ) -> Result<Option<Task>, ControlPlaneDispatchError> {
        let tasks = self
            .state
            .task_store()
            .list(Default::default())
            .map_err(crate::scheduler::service::TaskValidationError::from)
            .map_err(ControlPlaneDispatchError::Task)?;
        Ok(tasks.into_iter().find(|task| {
            task.metadata
                .as_ref()
                .and_then(|metadata| metadata.get("control_plane"))
                .and_then(|control_plane| control_plane.get("remote_task_id"))
                .and_then(serde_json::Value::as_str)
                == Some(remote_task_id)
        }))
    }
}
