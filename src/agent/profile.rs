use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::registry::ProviderRegistry;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceMode {
    Worktree,
    Direct,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionPolicy {
    #[serde(default = "default_workspace_mode")]
    pub default_workspace_mode: WorkspaceMode,
    #[serde(default)]
    pub allow_direct_directory: bool,
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            default_workspace_mode: WorkspaceMode::Worktree,
            allow_direct_directory: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderConfig {
    #[serde(default)]
    pub custom_args: Vec<String>,
    #[serde(default)]
    pub custom_env_keys: Vec<String>,
    #[serde(default)]
    pub mcp_config: Option<Value>,
    #[serde(default = "default_permission_mode")]
    pub permission_mode: String,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            custom_args: Vec::new(),
            custom_env_keys: Vec::new(),
            mcp_config: None,
            permission_mode: default_permission_mode(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentProfile {
    pub id: String,
    pub name: String,
    pub owner_product_id: String,
    pub provider_id: String,
    pub model: String,
    pub instructions: Option<String>,
    pub execution_policy: ExecutionPolicy,
    pub provider_config: ProviderConfig,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAgentProfile {
    pub id: String,
    pub name: String,
    pub owner_product_id: String,
    pub provider_id: String,
    pub model: String,
    pub instructions: Option<String>,
    pub execution_policy: ExecutionPolicy,
    pub provider_config: ProviderConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentAuthorizationRequest {
    pub owner_product_id: String,
    pub agent_id: String,
    pub provider_id_override: Option<String>,
    pub model_override: Option<String>,
    pub permission_mode_override: Option<String>,
    pub requested_workspace_mode: WorkspaceMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentProfileError {
    AgentNotFound,
    InvalidAgentId,
    InvalidAgentProfile,
    InvalidExecutionPolicy,
    InvalidProviderConfig,
    ProviderNotFound,
    ModelNotSupported,
    PermissionModeNotSupported,
    AgentAuthorizationFailed,
}

impl AgentProfileError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::AgentNotFound => "agent_not_found",
            Self::InvalidAgentId => "invalid_agent_id",
            Self::InvalidAgentProfile => "invalid_agent_profile",
            Self::InvalidExecutionPolicy => "invalid_execution_policy",
            Self::InvalidProviderConfig => "invalid_provider_config",
            Self::ProviderNotFound => "provider_not_found",
            Self::ModelNotSupported => "model_not_supported",
            Self::PermissionModeNotSupported => "permission_mode_not_supported",
            Self::AgentAuthorizationFailed => "agent_authorization_failed",
        }
    }

    #[must_use]
    pub const fn message(&self) -> &'static str {
        match self {
            Self::AgentNotFound => "agent profile not found",
            Self::InvalidAgentId => "invalid agent id",
            Self::InvalidAgentProfile => "invalid agent profile",
            Self::InvalidExecutionPolicy => "invalid execution policy",
            Self::InvalidProviderConfig => "invalid provider config",
            Self::ProviderNotFound => "provider not found",
            Self::ModelNotSupported => "model not supported",
            Self::PermissionModeNotSupported => "permission mode not supported",
            Self::AgentAuthorizationFailed => "agent authorization failed",
        }
    }
}

impl AgentProfile {
    pub fn validate_id(id: &str) -> Result<(), AgentProfileError> {
        let id_pattern =
            Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9._-]{0,127}$").expect("valid profile id regex");
        if id_pattern.is_match(id) {
            Ok(())
        } else {
            Err(AgentProfileError::InvalidAgentId)
        }
    }
}

impl CreateAgentProfile {
    pub fn validate(&self) -> Result<(), AgentProfileError> {
        AgentProfile::validate_id(&self.id)?;
        validate_required_string(&self.name)?;
        validate_required_string(&self.owner_product_id)?;
        validate_required_string(&self.provider_id)?;
        validate_required_string(&self.model)?;
        if let Some(instructions) = &self.instructions {
            validate_optional_string(instructions)?;
        }
        self.execution_policy.validate()?;
        self.provider_config.validate()?;
        Ok(())
    }

    pub fn validate_against_registry(
        &self,
        registry: &ProviderRegistry,
    ) -> Result<(), AgentProfileError> {
        self.validate()?;
        let provider = registry
            .get(&self.provider_id)
            .ok_or(AgentProfileError::ProviderNotFound)?;
        if !provider.manifest.models.supported.contains(&self.model) {
            return Err(AgentProfileError::ModelNotSupported);
        }
        let permission_mode = self.provider_config.permission_mode.as_str();
        if permission_mode != default_permission_mode()
            && !provider
                .manifest
                .permissions
                .provider_permission_modes
                .iter()
                .any(|mode| mode == permission_mode)
        {
            return Err(AgentProfileError::PermissionModeNotSupported);
        }
        Ok(())
    }

    pub fn into_profile(self, created_at: String, updated_at: String) -> AgentProfile {
        AgentProfile {
            id: self.id,
            name: self.name,
            owner_product_id: self.owner_product_id,
            provider_id: self.provider_id,
            model: self.model,
            instructions: self.instructions,
            execution_policy: self.execution_policy,
            provider_config: self.provider_config,
            created_at,
            updated_at,
        }
    }
}

impl ExecutionPolicy {
    pub fn validate(&self) -> Result<(), AgentProfileError> {
        if self.default_workspace_mode == WorkspaceMode::Direct && !self.allow_direct_directory {
            return Err(AgentProfileError::InvalidExecutionPolicy);
        }
        Ok(())
    }
}

impl ProviderConfig {
    pub fn validate(&self) -> Result<(), AgentProfileError> {
        let mut seen_args = std::collections::BTreeSet::new();
        for arg in &self.custom_args {
            if arg.is_empty()
                || arg.contains('\0')
                || RESERVED_PROVIDER_ARGS.contains(&arg.as_str())
                || !seen_args.insert(arg)
            {
                return Err(AgentProfileError::InvalidProviderConfig);
            }
        }

        let env_key_pattern = Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*$").expect("valid env key regex");
        let mut seen = std::collections::BTreeSet::new();
        for key in &self.custom_env_keys {
            if !env_key_pattern.is_match(key) || !seen.insert(key) {
                return Err(AgentProfileError::InvalidProviderConfig);
            }
        }

        validate_required_string(&self.permission_mode)?;
        Ok(())
    }
}

pub fn now_rfc3339() -> Result<String, AgentProfileError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| AgentProfileError::InvalidAgentProfile)
}

fn validate_required_string(value: &str) -> Result<(), AgentProfileError> {
    if value.trim().is_empty() {
        Err(AgentProfileError::InvalidAgentProfile)
    } else {
        Ok(())
    }
}

fn validate_optional_string(value: &str) -> Result<(), AgentProfileError> {
    if value.contains('\0') {
        Err(AgentProfileError::InvalidAgentProfile)
    } else {
        Ok(())
    }
}

fn default_workspace_mode() -> WorkspaceMode {
    WorkspaceMode::Worktree
}

fn default_permission_mode() -> String {
    "provider_default".to_owned()
}

const RESERVED_PROVIDER_ARGS: &[&str] = &[
    "--provider",
    "--model",
    "--cwd",
    "--directory",
    "--workdir",
    "--permission-mode",
    "--dangerously-bypass-approvals-and-sandbox",
];
