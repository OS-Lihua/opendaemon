use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskEventType {
    #[serde(rename = "task.queued")]
    Queued,
    #[serde(rename = "task.waiting_directory_lock")]
    WaitingDirectoryLock,
    #[serde(rename = "task.preparing")]
    Preparing,
    #[serde(rename = "task.running")]
    Running,
    #[serde(rename = "task.completed")]
    Completed,
    #[serde(rename = "task.failed")]
    Failed,
    #[serde(rename = "task.cancelled")]
    Cancelled,
    #[serde(rename = "task.timed_out")]
    TimedOut,
    #[serde(rename = "process.stdout")]
    ProcessStdout,
    #[serde(rename = "process.stderr")]
    ProcessStderr,
    #[serde(rename = "provider.permission_requested")]
    ProviderPermissionRequested,
    #[serde(rename = "provider.permission_decided")]
    ProviderPermissionDecided,
}

impl TaskEventType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "task.queued",
            Self::WaitingDirectoryLock => "task.waiting_directory_lock",
            Self::Preparing => "task.preparing",
            Self::Running => "task.running",
            Self::Completed => "task.completed",
            Self::Failed => "task.failed",
            Self::Cancelled => "task.cancelled",
            Self::TimedOut => "task.timed_out",
            Self::ProcessStdout => "process.stdout",
            Self::ProcessStderr => "process.stderr",
            Self::ProviderPermissionRequested => "provider.permission_requested",
            Self::ProviderPermissionDecided => "provider.permission_decided",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskEvent {
    pub id: String,
    pub task_id: String,
    pub sequence: i64,
    pub event_type: TaskEventType,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecision {
    Approve,
    Deny,
}

impl PermissionDecision {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Deny => "deny",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionRequestEvent {
    pub request_id: String,
    pub provider_id: String,
    pub permission_kind: String,
    pub summary: String,
    pub details: Option<Value>,
    pub options: Vec<PermissionDecision>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionDecisionEvent {
    pub request_id: String,
    pub decision: PermissionDecision,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskEventView {
    pub task_id: String,
    pub sequence: i64,
    #[serde(rename = "type")]
    pub event_type: TaskEventType,
    pub payload: Value,
    pub created_at: String,
}

impl From<TaskEvent> for TaskEventView {
    fn from(event: TaskEvent) -> Self {
        Self {
            task_id: event.task_id,
            sequence: event.sequence,
            event_type: event.event_type,
            payload: event.payload,
            created_at: event.created_at,
        }
    }
}
