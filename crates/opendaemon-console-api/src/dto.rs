use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceMode {
    Worktree,
    Direct,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    WaitingDirectoryLock,
    Preparing,
    Running,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecision {
    Approve,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderCapability {
    RemoteExecution,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub credential_type: String,
    pub product_id: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub product_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub service: String,
    pub version: String,
    pub status: String,
    pub control_plane: ControlPlaneStatus,
    pub scheduler: SchedulerStatus,
    pub runtimes: RuntimeStatusSummary,
    pub permissions: PermissionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlPlaneStatus {
    pub status: String,
    pub daemon_id: Option<String>,
    pub last_heartbeat_at: Option<String>,
    pub last_error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerStatus {
    pub queued: usize,
    pub running: usize,
    pub max_concurrent_tasks: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeStatusSummary {
    pub available: usize,
    pub unavailable: usize,
    pub error: usize,
    pub not_detected: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionStatus {
    pub pending: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Product {
    pub id: String,
    pub display_name: String,
    pub status: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductToken {
    pub id: String,
    pub product_id: String,
    pub label: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub token_prefix: String,
    pub status: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedProductToken {
    pub id: String,
    pub product_id: String,
    pub label: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub token_prefix: String,
    pub token: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub display_name: String,
    pub status: String,
    pub integration_type: String,
    pub description: String,
    pub manifest: Value,
}

impl Provider {
    #[must_use]
    pub fn capabilities(&self) -> Vec<ProviderCapability> {
        let Some(capabilities) = self.manifest.get("capabilities") else {
            return Vec::new();
        };
        let Some(object) = capabilities.as_object() else {
            return Vec::new();
        };
        if object
            .get("remote_execution")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            vec![ProviderCapability::RemoteExecution]
        } else {
            Vec::new()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeView {
    pub id: String,
    pub provider_id: String,
    pub kind: String,
    pub status: String,
    pub executable: Option<String>,
    pub version: Option<String>,
    pub detected_at: Option<String>,
    pub error: Option<RuntimeError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPolicy {
    pub default_workspace_mode: WorkspaceMode,
    pub allow_direct_directory: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProviderConfig {
    pub permission_mode: Option<String>,
    #[serde(default)]
    pub custom_args: Vec<String>,
    #[serde(default)]
    pub custom_env_keys: Vec<String>,
    pub mcp_config: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentProfile {
    pub id: String,
    pub name: String,
    pub owner_product_id: String,
    pub provider_id: String,
    pub model: String,
    pub instructions: Option<String>,
    pub execution_policy: ExecutionPolicy,
    pub provider_config: ProviderConfig,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryGrant {
    pub id: String,
    pub product_id: String,
    pub agent_id: String,
    pub path: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub workspace_modes: Vec<WorkspaceMode>,
    pub default_workspace_mode: WorkspaceMode,
    pub lock_policy: String,
    pub direct_mode_requires_explicit_task_opt_in: bool,
    pub allow_remote_execution: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub owner_product_id: String,
    pub agent_id: String,
    pub directory_id: String,
    pub prompt: String,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    pub workspace_mode: WorkspaceMode,
    pub direct_mode_task_opt_in: bool,
    pub metadata: Option<Value>,
    pub provider_id: String,
    pub model: String,
    pub permission_mode: String,
    pub timeout_seconds: Option<u64>,
    pub status: TaskStatus,
    pub result: Option<TaskResult>,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub cancelled_at: Option<String>,
    pub failed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub status: TaskStatus,
    pub final_message: String,
    #[serde(default)]
    pub changed_files: Vec<String>,
    pub diff: Option<String>,
    pub workspace_mode: WorkspaceMode,
    pub session_id: Option<String>,
    pub provider_result: Option<Value>,
    pub usage: Option<Value>,
    #[serde(default)]
    pub artifacts: Vec<Value>,
    pub error: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskEventView {
    pub task_id: String,
    pub sequence: u64,
    #[serde(rename = "type")]
    pub r#type: String,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub task_id: String,
    pub request_id: String,
    pub provider_id: String,
    pub permission_kind: String,
    pub summary: String,
    pub details: Option<Value>,
    #[serde(default)]
    pub options: Vec<PermissionDecision>,
    pub expires_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentProfileFormPayload {
    pub id: String,
    pub name: String,
    pub owner_product_id: String,
    pub provider_id: String,
    pub model: String,
    pub instructions: Option<String>,
    pub execution_policy: ExecutionPolicy,
    pub provider_config: ProviderConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryGrantFormPayload {
    pub product_id: String,
    pub agent_id: String,
    pub path: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub workspace_modes: Vec<WorkspaceMode>,
    pub default_workspace_mode: WorkspaceMode,
    pub lock_policy: String,
    pub direct_mode_requires_explicit_task_opt_in: bool,
    pub allow_remote_execution: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskCreatePayload {
    pub owner_product_id: String,
    pub agent_id: String,
    pub directory_id: String,
    pub prompt: String,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    pub workspace_mode: WorkspaceMode,
    pub direct_mode_task_opt_in: bool,
    pub metadata: Option<Value>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateProductPayload {
    pub id: String,
    pub display_name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateProductPayload {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateProductTokenPayload {
    pub label: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionResponsePayload {
    pub event_type: String,
    pub request_id: String,
    pub decision: PermissionDecision,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProductsEnvelope {
    pub products: Vec<Product>,
}

#[derive(Debug, Deserialize)]
pub struct ProductEnvelope {
    pub product: Product,
}

#[derive(Debug, Deserialize)]
pub struct ProductTokensEnvelope {
    pub tokens: Vec<ProductToken>,
}

#[derive(Debug, Deserialize)]
pub struct CreatedProductTokenEnvelope {
    pub token: CreatedProductToken,
}

#[derive(Debug, Deserialize)]
pub struct ProvidersEnvelope {
    pub providers: Vec<Provider>,
}

#[derive(Debug, Deserialize)]
pub struct RuntimesEnvelope {
    pub runtimes: Vec<RuntimeView>,
}

#[derive(Debug, Deserialize)]
pub struct AgentsEnvelope {
    pub agents: Vec<AgentProfile>,
}

#[derive(Debug, Deserialize)]
pub struct AgentEnvelope {
    pub agent: AgentProfile,
}

#[derive(Debug, Deserialize)]
pub struct DirectoriesEnvelope {
    pub directories: Vec<DirectoryGrant>,
}

#[derive(Debug, Deserialize)]
pub struct DirectoryEnvelope {
    pub directory: DirectoryGrant,
}

#[derive(Debug, Deserialize)]
pub struct TasksEnvelope {
    pub tasks: Vec<Task>,
}

#[derive(Debug, Deserialize)]
pub struct TaskEnvelope {
    pub task: Task,
}

#[derive(Debug, Deserialize)]
pub struct PermissionsEnvelope {
    pub permissions: Vec<PermissionRequest>,
}
