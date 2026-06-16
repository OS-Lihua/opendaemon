use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::{
    control_plane::model::DaemonConnectionStatus, product::ApiScope, runtime::model::RuntimeStatus,
    task::model::TaskStatus,
};

use super::{AppState, AuthError, ErrorBody, ErrorResponse, auth::AnyAuth};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct DaemonStatusResponse {
    pub service: &'static str,
    pub version: &'static str,
    pub status: DaemonConnectionStatus,
    pub control_plane: ControlPlaneStatus,
    pub scheduler: SchedulerStatus,
    pub runtimes: RuntimeStatusSummary,
    pub permissions: PermissionStatus,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ControlPlaneStatus {
    pub status: DaemonConnectionStatus,
    pub daemon_id: Option<String>,
    pub last_heartbeat_at: Option<String>,
    pub last_error_code: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SchedulerStatus {
    pub queued: usize,
    pub running: usize,
    pub max_concurrent_tasks: usize,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct RuntimeStatusSummary {
    pub available: usize,
    pub unavailable: usize,
    pub error: usize,
    pub not_detected: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct PermissionStatus {
    pub pending: usize,
}

pub async fn get(
    auth: AnyAuth,
    State(state): State<AppState>,
) -> Result<Json<DaemonStatusResponse>, ApiError> {
    auth.require_scopes(&[ApiScope::RuntimesRead, ApiScope::TasksRead])?;
    let owner_product_id = auth
        .product_context()
        .map(|context| context.product_id.clone());
    let tasks = state.task_store().list(crate::store::tasks::TaskFilters {
        owner_product_id: owner_product_id.clone(),
        ..crate::store::tasks::TaskFilters::default()
    })?;
    let queued = tasks
        .iter()
        .filter(|task| task.status == TaskStatus::Queued)
        .count();
    let running = tasks
        .iter()
        .filter(|task| task.status == TaskStatus::Running)
        .count();
    let providers = state
        .load_registry()
        .map_err(ApiError::Registry)?
        .providers()
        .iter()
        .map(|entry| entry.manifest.clone())
        .collect::<Vec<_>>();
    let runtimes = state.runtime_store().list_for_providers(&providers).await;
    let mut runtime_summary = RuntimeStatusSummary::default();
    for runtime in runtimes {
        match runtime.status {
            RuntimeStatus::Available => runtime_summary.available += 1,
            RuntimeStatus::Unavailable => runtime_summary.unavailable += 1,
            RuntimeStatus::Error => runtime_summary.error += 1,
            RuntimeStatus::NotDetected => runtime_summary.not_detected += 1,
        }
    }
    let pending_permissions = state
        .task_store()
        .list_permission_requests(crate::store::tasks::PermissionRequestFilters {
            owner_product_id,
            status: Some(crate::task::permission::PermissionRequestStatus::Pending),
        })?
        .len();

    Ok(Json(DaemonStatusResponse {
        service: "opendaemon",
        version: env!("CARGO_PKG_VERSION"),
        status: DaemonConnectionStatus::Online,
        control_plane: ControlPlaneStatus {
            status: DaemonConnectionStatus::Offline,
            daemon_id: None,
            last_heartbeat_at: None,
            last_error_code: None,
        },
        scheduler: SchedulerStatus {
            queued,
            running,
            max_concurrent_tasks: state.scheduler_config().max_concurrent_tasks,
        },
        runtimes: runtime_summary,
        permissions: PermissionStatus {
            pending: pending_permissions,
        },
    }))
}

#[derive(Debug)]
pub enum ApiError {
    Auth(AuthError),
    Registry(anyhow::Error),
    Task(crate::store::tasks::TaskStoreError),
}

impl From<AuthError> for ApiError {
    fn from(error: AuthError) -> Self {
        Self::Auth(error)
    }
}

impl From<crate::store::tasks::TaskStoreError> for ApiError {
    fn from(error: crate::store::tasks::TaskStoreError) -> Self {
        Self::Task(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Auth(error) => (error.status(), error.code(), error.message().to_owned()),
            Self::Registry(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "registry_error",
                error.to_string(),
            ),
            Self::Task(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "store_error",
                format!("{error:?}"),
            ),
        };

        (
            status,
            Json(ErrorResponse {
                error: ErrorBody { code, message },
            }),
        )
            .into_response()
    }
}
