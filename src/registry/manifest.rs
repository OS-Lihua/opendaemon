use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderManifest {
    pub schema_version: String,
    pub id: String,
    pub display_name: String,
    pub status: ProviderStatus,
    pub vendor: VendorInfo,
    pub integration_type: IntegrationType,
    pub description: String,
    pub install: InstallInstructions,
    pub detect: DetectConfig,
    pub execution: ExecutionConfig,
    pub models: ModelConfig,
    pub capabilities: ProviderCapabilities,
    pub permissions: ProviderPermissions,
    pub environment: EnvironmentConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderStatus {
    Community,
    Verified,
    FirstParty,
    Deprecated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VendorInfo {
    pub name: String,
    pub homepage: String,
    pub support_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationType {
    Cli,
    Acp,
    Http,
    Native,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InstallInstructions {
    pub macos: Vec<String>,
    pub linux: Vec<String>,
    pub windows: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DetectConfig {
    pub commands: Vec<String>,
    pub version_args: Vec<String>,
    pub version_regex: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExecutionConfig {
    pub command: String,
    pub args: Vec<String>,
    pub input_mode: ExecutionInputMode,
    pub working_directory: WorkingDirectoryMode,
    pub supports_streaming: bool,
    pub cancel_signal: CancelSignal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionInputMode {
    Arg,
    Stdin,
    TempFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkingDirectoryMode {
    Required,
    Optional,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum CancelSignal {
    #[serde(rename = "SIGTERM")]
    Sigterm,
    #[serde(rename = "SIGINT")]
    Sigint,
    #[serde(rename = "kill")]
    Kill,
    #[serde(rename = "none")]
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModelConfig {
    pub default: String,
    pub supported: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderCapabilities {
    pub filesystem_read: bool,
    pub filesystem_write: bool,
    pub shell: bool,
    pub git: bool,
    pub browser: bool,
    pub mcp: bool,
    pub remote_execution: bool,
    pub worktree: bool,
    pub direct_directory: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderPermissions {
    pub requires_directory_grant: bool,
    pub recommended_directory_lock: DirectoryLockMode,
    pub provider_permission_modes: Vec<String>,
    pub supports_permission_events: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DirectoryLockMode {
    Exclusive,
    Shared,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentConfig {
    pub required: Vec<String>,
    pub optional: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SecurityConfig {
    pub runs_locally: bool,
    pub sends_code_to_vendor: bool,
    pub data_policy_url: Option<String>,
    pub review_level: SecurityReviewLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SecurityReviewLevel {
    Standard,
    Strict,
    Experimental,
}
