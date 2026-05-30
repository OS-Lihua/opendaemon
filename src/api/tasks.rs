use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    scheduler::service::{SchedulerService, TaskValidationError},
    security::directory::{DirectoryCapability, WorkspaceMode},
    store::tasks::TaskFilters,
    task::model::{CreateTask, Task, TaskStatus},
};

use super::{AppState, ErrorBody, ErrorResponse};

pub type TaskResponse = Task;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct TaskListResponse {
    pub tasks: Vec<TaskResponse>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SingleTaskResponse {
    pub task: TaskResponse,
}

#[derive(Debug, Deserialize)]
pub struct TaskListQuery {
    pub owner_product_id: Option<String>,
    pub agent_id: Option<String>,
    pub directory_id: Option<String>,
    pub status: Option<TaskStatus>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub owner_product_id: String,
    pub agent_id: String,
    pub directory_id: String,
    pub prompt: String,
    pub required_capabilities: Option<Vec<DirectoryCapability>>,
    pub workspace_mode: Option<WorkspaceMode>,
    #[serde(default)]
    pub direct_mode_task_opt_in: bool,
    pub metadata: Option<Value>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub permission_mode: Option<String>,
    pub timeout_seconds: Option<u64>,
}

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<TaskListQuery>,
) -> Result<Json<TaskListResponse>, ApiError> {
    let tasks = state.task_store().list(TaskFilters {
        owner_product_id: query.owner_product_id,
        agent_id: query.agent_id,
        directory_id: query.directory_id,
        status: query.status,
    })?;

    Ok(Json(TaskListResponse { tasks }))
}

pub async fn create(
    State(state): State<AppState>,
    Json(request): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<SingleTaskResponse>), ApiError> {
    let service = scheduler_service(&state);
    let task = service.enqueue_task(request.into())?;

    Ok((StatusCode::CREATED, Json(SingleTaskResponse { task })))
}

pub async fn get(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<SingleTaskResponse>, ApiError> {
    let task = state.task_store().get(&task_id)?;

    Ok(Json(SingleTaskResponse { task }))
}

pub async fn cancel(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<SingleTaskResponse>, ApiError> {
    let task = scheduler_service(&state).cancel_task(&task_id)?;

    Ok(Json(SingleTaskResponse { task }))
}

fn scheduler_service(state: &AppState) -> SchedulerService {
    SchedulerService::new(
        state.task_store().clone(),
        state.agent_profile_store().clone(),
        state.directory_grant_store().clone(),
        state.scheduler_config(),
    )
}

impl From<CreateTaskRequest> for CreateTask {
    fn from(request: CreateTaskRequest) -> Self {
        Self {
            owner_product_id: request.owner_product_id,
            agent_id: request.agent_id,
            directory_id: request.directory_id,
            prompt: request.prompt,
            required_capabilities: request.required_capabilities,
            workspace_mode: request.workspace_mode,
            direct_mode_task_opt_in: request.direct_mode_task_opt_in,
            metadata: request.metadata,
            provider_id: request.provider_id,
            model: request.model,
            permission_mode: request.permission_mode,
            timeout_seconds: request.timeout_seconds,
        }
    }
}

#[derive(Debug)]
pub enum ApiError {
    Task(TaskValidationError),
}

impl From<TaskValidationError> for ApiError {
    fn from(error: TaskValidationError) -> Self {
        Self::Task(error)
    }
}

impl From<crate::store::tasks::TaskStoreError> for ApiError {
    fn from(error: crate::store::tasks::TaskStoreError) -> Self {
        Self::Task(error.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Task(error) => task_error_response(error),
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

fn task_error_response(error: TaskValidationError) -> (StatusCode, &'static str, String) {
    match error {
        TaskValidationError::TaskNotFound => (
            StatusCode::NOT_FOUND,
            "task_not_found",
            "task not found".to_owned(),
        ),
        TaskValidationError::InvalidTask => (
            StatusCode::BAD_REQUEST,
            "invalid_task",
            "invalid task".to_owned(),
        ),
        TaskValidationError::InvalidTaskPrompt => (
            StatusCode::BAD_REQUEST,
            "invalid_task_prompt",
            "invalid task prompt".to_owned(),
        ),
        TaskValidationError::InvalidTaskState => (
            StatusCode::BAD_REQUEST,
            "invalid_task_state",
            "invalid task state".to_owned(),
        ),
        TaskValidationError::AgentNotFound => (
            StatusCode::NOT_FOUND,
            "agent_not_found",
            "agent profile not found".to_owned(),
        ),
        TaskValidationError::DirectoryNotFound => (
            StatusCode::NOT_FOUND,
            "directory_not_found",
            "directory grant not found".to_owned(),
        ),
        TaskValidationError::AgentAuthorizationFailed => (
            StatusCode::FORBIDDEN,
            "agent_authorization_failed",
            "agent authorization failed".to_owned(),
        ),
        TaskValidationError::DirectoryAuthorizationFailed => (
            StatusCode::FORBIDDEN,
            "directory_authorization_failed",
            "directory authorization failed".to_owned(),
        ),
        TaskValidationError::CapabilityNotAllowed => (
            StatusCode::FORBIDDEN,
            "capability_not_allowed",
            "capability not allowed".to_owned(),
        ),
        TaskValidationError::WorkspaceModeNotAllowed => (
            StatusCode::FORBIDDEN,
            "workspace_mode_not_allowed",
            "workspace mode not allowed".to_owned(),
        ),
        TaskValidationError::DirectModeNotAllowed => (
            StatusCode::FORBIDDEN,
            "direct_mode_not_allowed",
            "direct mode not allowed".to_owned(),
        ),
        TaskValidationError::ProviderOverrideNotAllowed => (
            StatusCode::BAD_REQUEST,
            "provider_override_not_allowed",
            "provider override not allowed".to_owned(),
        ),
        TaskValidationError::ModelOverrideNotAllowed => (
            StatusCode::BAD_REQUEST,
            "model_override_not_allowed",
            "model override not allowed".to_owned(),
        ),
        TaskValidationError::PermissionModeOverrideNotAllowed => (
            StatusCode::BAD_REQUEST,
            "permission_mode_override_not_allowed",
            "permission mode override not allowed".to_owned(),
        ),
        TaskValidationError::DirectoryLockConflict => (
            StatusCode::CONFLICT,
            "directory_lock_conflict",
            "directory lock conflict".to_owned(),
        ),
        TaskValidationError::TaskAlreadyTerminal => (
            StatusCode::CONFLICT,
            "task_already_terminal",
            "task is already terminal".to_owned(),
        ),
        TaskValidationError::Store(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "store_error",
            error.to_string(),
        ),
    }
}
