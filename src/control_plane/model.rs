use serde::{Deserialize, Serialize};

use crate::runtime::model::{RuntimeKind, RuntimeStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonConnectionStatus {
    Online,
    Offline,
    Connecting,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonRegistrationRecord {
    pub daemon_id: String,
    pub control_plane_url: String,
    pub daemon_token: String,
    pub status: DaemonConnectionStatus,
    pub registered_at: String,
    pub last_heartbeat_at: Option<String>,
    pub last_error_code: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DaemonRuntimeSummary {
    pub provider_id: String,
    pub kind: RuntimeKind,
    pub status: RuntimeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DaemonRegistrationRequest {
    pub daemon_id: Option<String>,
    pub session_id: Option<String>,
    pub enrollment_secret: String,
    pub daemon_version: String,
    pub platform: String,
    pub capabilities: Vec<String>,
    pub runtimes: Vec<DaemonRuntimeSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonRegistrationAccepted {
    pub daemon_id: String,
    pub daemon_token: String,
    pub session_id: Option<String>,
    pub registered_at: String,
}
