use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::event::{
    PermissionDecision, PermissionDecisionEvent, PermissionRequestEvent, TaskEvent,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionRequestStatus {
    Pending,
    Approved,
    Denied,
}

impl PermissionRequestStatus {
    #[must_use]
    pub const fn from_decision(decision: PermissionDecision) -> Self {
        match decision {
            PermissionDecision::Approve => Self::Approved,
            PermissionDecision::Deny => Self::Denied,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionRequestRecord {
    pub request_id: String,
    pub task_id: String,
    pub sequence: i64,
    pub provider_id: String,
    pub permission_kind: String,
    pub status: PermissionRequestStatus,
    pub request: PermissionRequestEvent,
    pub response: Option<PermissionDecisionEvent>,
    pub requested_at: String,
    pub responded_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionResolution {
    pub task_id: String,
    pub request_id: String,
    pub status: PermissionRequestStatus,
    pub decision: PermissionDecision,
    pub event: TaskEvent,
    pub duplicated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionResponseRequest {
    pub request_id: String,
    pub decision: PermissionDecision,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionResponsePayload {
    pub request_id: String,
    pub decision: PermissionDecision,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionRequestDetails {
    pub details: Option<Value>,
}
