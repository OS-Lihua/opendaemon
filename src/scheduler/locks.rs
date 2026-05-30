use serde::{Deserialize, Serialize};

use crate::{
    security::directory::{DirectoryCapability, DirectoryLockPolicy},
    task::model::Task,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LockMode {
    Exclusive,
    Shared,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryLock {
    pub directory_id: String,
    pub task_id: String,
    pub mode: LockMode,
    pub status: String,
    pub created_at: String,
    pub released_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockRequest {
    pub directory_id: String,
    pub task_id: String,
    pub mode: LockMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockDecision {
    Acquired,
    Waiting,
    NotRequired,
}

impl LockRequest {
    #[must_use]
    pub fn from_task(task: &Task) -> Self {
        Self {
            directory_id: task.directory_id.clone(),
            task_id: task.id.clone(),
            mode: mode_for_capabilities(
                &task.required_capabilities,
                DirectoryLockPolicy::Exclusive,
            ),
        }
    }
}

#[must_use]
pub fn mode_for_capabilities(
    capabilities: &[DirectoryCapability],
    policy: DirectoryLockPolicy,
) -> LockMode {
    if matches!(policy, DirectoryLockPolicy::None) {
        return LockMode::None;
    }
    if capabilities.iter().any(|capability| {
        matches!(
            capability,
            DirectoryCapability::Write | DirectoryCapability::Shell | DirectoryCapability::Git
        )
    }) {
        return LockMode::Exclusive;
    }
    match policy {
        DirectoryLockPolicy::Exclusive => LockMode::Exclusive,
        DirectoryLockPolicy::Shared => LockMode::Shared,
        DirectoryLockPolicy::None => LockMode::None,
    }
}
