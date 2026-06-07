use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DirectoryCapability {
    Read,
    Write,
    Shell,
    Git,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceMode {
    Worktree,
    Direct,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DirectoryLockPolicy {
    Exclusive,
    Shared,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DirectoryGrant {
    pub id: String,
    pub product_id: String,
    pub agent_id: String,
    pub path: String,
    pub capabilities: Vec<DirectoryCapability>,
    pub workspace_modes: Vec<WorkspaceMode>,
    pub default_workspace_mode: WorkspaceMode,
    pub lock_policy: DirectoryLockPolicy,
    pub direct_mode_requires_explicit_task_opt_in: bool,
    pub allow_remote_execution: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryGrantPolicy {
    pub capabilities: Vec<DirectoryCapability>,
    pub workspace_modes: Vec<WorkspaceMode>,
    pub default_workspace_mode: WorkspaceMode,
    pub lock_policy: DirectoryLockPolicy,
    pub direct_mode_requires_explicit_task_opt_in: bool,
    pub allow_remote_execution: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryAuthorizationRequest {
    pub product_id: String,
    pub agent_id: String,
    pub directory_id: String,
    pub required_capabilities: Vec<DirectoryCapability>,
    pub requested_workspace_mode: WorkspaceMode,
    pub direct_mode_task_opt_in: bool,
    pub remote_execution: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectorySecurityError {
    InvalidCapability,
    InvalidWorkspaceMode,
    InvalidLockPolicy,
    DirectModeNotAllowed,
    AuthorizationFailed,
}

impl DirectorySecurityError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidCapability => "invalid_capability",
            Self::InvalidWorkspaceMode => "invalid_workspace_mode",
            Self::InvalidLockPolicy => "invalid_lock_policy",
            Self::DirectModeNotAllowed => "direct_mode_not_allowed",
            Self::AuthorizationFailed => "directory_authorization_failed",
        }
    }

    #[must_use]
    pub const fn message(&self) -> &'static str {
        match self {
            Self::InvalidCapability => "invalid directory capability",
            Self::InvalidWorkspaceMode => "invalid workspace mode",
            Self::InvalidLockPolicy => "invalid directory lock policy",
            Self::DirectModeNotAllowed => "direct mode is not allowed",
            Self::AuthorizationFailed => "directory authorization failed",
        }
    }
}

impl DirectoryGrantPolicy {
    pub fn new(
        capabilities: Vec<DirectoryCapability>,
        workspace_modes: Vec<WorkspaceMode>,
        default_workspace_mode: WorkspaceMode,
        lock_policy: DirectoryLockPolicy,
        direct_mode_requires_explicit_task_opt_in: bool,
        allow_remote_execution: bool,
    ) -> Result<Self, DirectorySecurityError> {
        let capabilities = normalize_capabilities(capabilities)?;
        let workspace_modes = normalize_workspace_modes(workspace_modes)?;
        let policy = Self {
            capabilities,
            workspace_modes,
            default_workspace_mode,
            lock_policy,
            direct_mode_requires_explicit_task_opt_in,
            allow_remote_execution,
        };
        policy.validate()?;
        Ok(policy)
    }

    pub fn validate(&self) -> Result<(), DirectorySecurityError> {
        if self.capabilities.is_empty() {
            return Err(DirectorySecurityError::InvalidCapability);
        }
        if self.workspace_modes.is_empty()
            || !self.workspace_modes.contains(&self.default_workspace_mode)
        {
            return Err(DirectorySecurityError::InvalidWorkspaceMode);
        }
        if self.capabilities.contains(&DirectoryCapability::Write)
            && self.lock_policy == DirectoryLockPolicy::None
        {
            return Err(DirectorySecurityError::InvalidLockPolicy);
        }
        Ok(())
    }
}

impl DirectoryGrant {
    #[must_use]
    pub fn policy(&self) -> DirectoryGrantPolicy {
        DirectoryGrantPolicy {
            capabilities: self.capabilities.clone(),
            workspace_modes: self.workspace_modes.clone(),
            default_workspace_mode: self.default_workspace_mode,
            lock_policy: self.lock_policy,
            direct_mode_requires_explicit_task_opt_in: self
                .direct_mode_requires_explicit_task_opt_in,
            allow_remote_execution: self.allow_remote_execution,
        }
    }

    pub fn authorize(
        &self,
        request: &DirectoryAuthorizationRequest,
    ) -> Result<(), DirectorySecurityError> {
        if self.product_id != request.product_id
            || self.agent_id != request.agent_id
            || self.id != request.directory_id
        {
            return Err(DirectorySecurityError::AuthorizationFailed);
        }
        if request
            .required_capabilities
            .iter()
            .any(|capability| !self.capabilities.contains(capability))
        {
            return Err(DirectorySecurityError::AuthorizationFailed);
        }
        if !self
            .workspace_modes
            .contains(&request.requested_workspace_mode)
        {
            return Err(match request.requested_workspace_mode {
                WorkspaceMode::Direct => DirectorySecurityError::DirectModeNotAllowed,
                WorkspaceMode::Worktree => DirectorySecurityError::AuthorizationFailed,
            });
        }
        if request.requested_workspace_mode == WorkspaceMode::Direct
            && self.direct_mode_requires_explicit_task_opt_in
            && !request.direct_mode_task_opt_in
        {
            return Err(DirectorySecurityError::DirectModeNotAllowed);
        }
        if request.remote_execution && !self.allow_remote_execution {
            return Err(DirectorySecurityError::AuthorizationFailed);
        }
        Ok(())
    }
}

fn normalize_capabilities(
    capabilities: Vec<DirectoryCapability>,
) -> Result<Vec<DirectoryCapability>, DirectorySecurityError> {
    let normalized = capabilities
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        return Err(DirectorySecurityError::InvalidCapability);
    }
    Ok(normalized)
}

fn normalize_workspace_modes(
    workspace_modes: Vec<WorkspaceMode>,
) -> Result<Vec<WorkspaceMode>, DirectorySecurityError> {
    let normalized = workspace_modes
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        return Err(DirectorySecurityError::InvalidWorkspaceMode);
    }
    Ok(normalized)
}
