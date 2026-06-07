use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteDispatchTask {
    pub remote_task_id: String,
    pub owner_product_id: String,
    pub agent_id: String,
    pub directory_id: String,
    pub prompt: String,
    pub required_capabilities: Vec<String>,
    pub workspace_mode: String,
    pub timeout_seconds: Option<u64>,
    pub task_token: String,
    pub metadata: Value,
}
