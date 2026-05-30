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
