use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::result::TaskResult;
use crate::security::directory::{DirectoryCapability, WorkspaceMode};

pub const MAX_TASK_TIMEOUT_SECONDS: u64 = 86_400;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    WaitingDirectoryLock,
    Preparing,
    Running,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
}

impl TaskStatus {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::TimedOut
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub owner_product_id: String,
    pub agent_id: String,
    pub directory_id: String,
    pub status: TaskStatus,
    pub required_capabilities: Vec<DirectoryCapability>,
    pub workspace_mode: WorkspaceMode,
    pub direct_mode_task_opt_in: bool,
    pub prompt: String,
    pub metadata: Option<Value>,
    pub provider_id: String,
    pub model: String,
    pub permission_mode: String,
    pub timeout_seconds: Option<u64>,
    pub result: Option<TaskResult>,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub cancelled_at: Option<String>,
    pub failed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTask {
    pub owner_product_id: String,
    pub agent_id: String,
    pub directory_id: String,
    pub prompt: String,
    pub required_capabilities: Option<Vec<DirectoryCapability>>,
    pub workspace_mode: Option<WorkspaceMode>,
    pub direct_mode_task_opt_in: bool,
    pub metadata: Option<Value>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub permission_mode: Option<String>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskModelError {
    InvalidTask,
    InvalidPrompt,
}

impl CreateTask {
    pub fn validate(&self) -> Result<(), TaskModelError> {
        if self.owner_product_id.trim().is_empty()
            || self.agent_id.trim().is_empty()
            || self.directory_id.trim().is_empty()
        {
            return Err(TaskModelError::InvalidTask);
        }
        if self.prompt.trim().is_empty() {
            return Err(TaskModelError::InvalidPrompt);
        }
        if self.required_capabilities().is_empty() {
            return Err(TaskModelError::InvalidTask);
        }
        if let Some(metadata) = &self.metadata
            && !metadata.is_object()
        {
            return Err(TaskModelError::InvalidTask);
        }
        if self
            .timeout_seconds
            .is_some_and(|timeout| timeout == 0 || timeout > MAX_TASK_TIMEOUT_SECONDS)
        {
            return Err(TaskModelError::InvalidTask);
        }
        Ok(())
    }

    #[must_use]
    pub fn required_capabilities(&self) -> Vec<DirectoryCapability> {
        let capabilities = self
            .required_capabilities
            .clone()
            .unwrap_or_else(|| vec![DirectoryCapability::Read]);
        capabilities
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}
