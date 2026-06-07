use std::{future::Future, path::PathBuf, pin::Pin};

use serde_json::Value;

use crate::{
    agent::profile::AgentProfile,
    registry::{IntegrationType, ProviderManifest},
    scheduler::workspace::PreparedWorkspace,
    security::directory::DirectoryGrant,
    task::{event::TaskEventType, model::TaskStatus},
};

#[derive(Debug, Clone)]
pub struct RuntimeExecutionRequest {
    pub task_id: String,
    pub provider_id: String,
    pub runtime_id: String,
    pub executable: PathBuf,
    pub manifest: ProviderManifest,
    pub agent_profile: AgentProfile,
    pub directory_grant: DirectoryGrant,
    pub task: crate::task::model::Task,
    pub workspace: PreparedWorkspace,
    pub timeout_seconds: u64,
    pub allow_agent_custom_env: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeExecutionStatus {
    Completed,
    Failed,
    Cancelled,
    TimedOut,
}

impl RuntimeExecutionStatus {
    #[must_use]
    pub const fn task_status(self) -> TaskStatus {
        match self {
            Self::Completed => TaskStatus::Completed,
            Self::Failed => TaskStatus::Failed,
            Self::Cancelled => TaskStatus::Cancelled,
            Self::TimedOut => TaskStatus::TimedOut,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeOutputEvent {
    pub kind: TaskEventType,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeExecutionOutcome {
    pub status: RuntimeExecutionStatus,
    pub exit_code: Option<i32>,
    pub final_message: String,
    pub changed_files: Vec<String>,
    pub diff: Option<String>,
    pub session_id: Option<String>,
    pub provider_result: Option<Value>,
    pub usage: Option<Value>,
    pub error: Option<RuntimeAdapterError>,
    pub events: Vec<RuntimeOutputEvent>,
}

impl RuntimeExecutionOutcome {
    #[must_use]
    pub fn completed(exit_code: Option<i32>, events: Vec<RuntimeOutputEvent>) -> Self {
        Self {
            status: RuntimeExecutionStatus::Completed,
            exit_code,
            final_message: "provider command completed".to_owned(),
            changed_files: Vec::new(),
            diff: None,
            session_id: None,
            provider_result: None,
            usage: None,
            error: None,
            events,
        }
    }

    #[must_use]
    pub fn failed(
        exit_code: Option<i32>,
        error: RuntimeAdapterError,
        events: Vec<RuntimeOutputEvent>,
    ) -> Self {
        Self {
            status: RuntimeExecutionStatus::Failed,
            exit_code,
            final_message: "provider command failed".to_owned(),
            changed_files: Vec::new(),
            diff: None,
            session_id: None,
            provider_result: None,
            usage: None,
            error: Some(error),
            events,
        }
    }

    #[must_use]
    pub fn timed_out(events: Vec<RuntimeOutputEvent>) -> Self {
        Self {
            status: RuntimeExecutionStatus::TimedOut,
            exit_code: None,
            final_message: "provider command timed out".to_owned(),
            changed_files: Vec::new(),
            diff: None,
            session_id: None,
            provider_result: None,
            usage: None,
            error: Some(RuntimeAdapterError::new(
                "command_timeout",
                "provider command timed out",
            )),
            events,
        }
    }

    #[must_use]
    pub fn cancelled(events: Vec<RuntimeOutputEvent>) -> Self {
        Self {
            status: RuntimeExecutionStatus::Cancelled,
            exit_code: None,
            final_message: "provider command cancelled".to_owned(),
            changed_files: Vec::new(),
            diff: None,
            session_id: None,
            provider_result: None,
            usage: None,
            error: Some(RuntimeAdapterError::new(
                "command_cancelled",
                "provider command was cancelled",
            )),
            events,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCancelOutcome {
    pub cancelled: bool,
    pub error: Option<RuntimeAdapterError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeAdapterError {
    code: &'static str,
    message: String,
}

impl RuntimeAdapterError {
    #[must_use]
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

pub trait RuntimeAdapter: Send + Sync {
    fn execute(
        &self,
        request: RuntimeExecutionRequest,
    ) -> Pin<Box<dyn Future<Output = RuntimeExecutionOutcome> + Send + '_>>;

    fn cancel(
        &self,
        task_id: &str,
    ) -> Pin<Box<dyn Future<Output = RuntimeCancelOutcome> + Send + '_>>;
}

#[derive(Debug, Clone, Default)]
pub struct AdapterSelector {
    cli: crate::runtime::cli::LocalCliAdapter,
}

impl AdapterSelector {
    pub fn for_manifest(
        &self,
        manifest: &ProviderManifest,
    ) -> Result<SelectedAdapter, RuntimeAdapterError> {
        match manifest.integration_type {
            IntegrationType::Cli => Ok(SelectedAdapter::Cli(self.cli.clone())),
            IntegrationType::Acp | IntegrationType::Native => Err(RuntimeAdapterError::new(
                "adapter_not_implemented",
                "provider adapter is not implemented",
            )),
            IntegrationType::Http => Err(RuntimeAdapterError::new(
                "remote_execution_not_allowed",
                "remote provider execution is not allowed",
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SelectedAdapter {
    Cli(crate::runtime::cli::LocalCliAdapter),
}

impl RuntimeAdapter for SelectedAdapter {
    fn execute(
        &self,
        request: RuntimeExecutionRequest,
    ) -> Pin<Box<dyn Future<Output = RuntimeExecutionOutcome> + Send + '_>> {
        match self {
            Self::Cli(adapter) => adapter.execute(request),
        }
    }

    fn cancel(
        &self,
        task_id: &str,
    ) -> Pin<Box<dyn Future<Output = RuntimeCancelOutcome> + Send + '_>> {
        match self {
            Self::Cli(adapter) => adapter.cancel(task_id),
        }
    }
}
