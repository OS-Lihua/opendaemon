use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::model::TaskStatus;
use crate::security::directory::WorkspaceMode;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskResult {
    pub task_id: String,
    pub status: TaskStatus,
    pub final_message: String,
    pub changed_files: Vec<String>,
    pub diff: Option<String>,
    pub workspace_mode: WorkspaceMode,
    pub worktree_path: Option<String>,
    pub source_directory_id: String,
    pub branch_name: Option<String>,
    pub commit_hash: Option<String>,
    pub session_id: Option<String>,
    pub provider_result: Option<Value>,
    pub usage: Option<Value>,
    pub artifacts: Vec<Value>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
