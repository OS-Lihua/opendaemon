use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredSession {
    pub base_url: String,
    pub credential_mode: String,
    pub bearer_token: String,
    pub last_route: String,
    pub active_task_id: Option<String>,
}

#[must_use]
pub fn storage_key() -> &'static str {
    "opendaemon.console.session"
}
