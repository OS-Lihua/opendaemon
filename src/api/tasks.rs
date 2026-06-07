use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response, Sse},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};

use crate::{
    product::ApiScope,
    scheduler::service::{SchedulerService, TaskValidationError},
    security::directory::{DirectoryCapability, WorkspaceMode},
    store::tasks::TaskFilters,
    task::{
        event::{PermissionDecision, TaskEventView},
        model::{CreateTask, Task, TaskStatus},
        permission::PermissionResponseRequest,
        service::{TaskEventServiceError, TaskStreamFrame},
    },
};

use super::{AppState, AuthError, ErrorBody, ErrorResponse, ProductAuth};

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

#[derive(Debug, Deserialize)]
pub struct TaskEventsQuery {
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TaskEventRequest {
    pub event_type: String,
    pub request_id: String,
    pub decision: String,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct TaskPermissionResponse {
    pub task_id: String,
    pub request_id: String,
    pub status: String,
    pub decision: PermissionDecision,
}

pub async fn list(
    auth: ProductAuth,
    State(state): State<AppState>,
    Query(query): Query<TaskListQuery>,
) -> Result<Json<TaskListResponse>, ApiError> {
    auth.require_scope(ApiScope::TasksRead)?;
    if let Some(owner_product_id) = &query.owner_product_id {
        auth.ensure_product(owner_product_id)?;
    }
    let tasks = state.task_store().list(TaskFilters {
        owner_product_id: Some(auth.product_id().to_owned()),
        agent_id: query.agent_id,
        directory_id: query.directory_id,
        status: query.status,
    })?;

    Ok(Json(TaskListResponse { tasks }))
}

pub async fn create(
    auth: ProductAuth,
    State(state): State<AppState>,
    Json(mut request): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<SingleTaskResponse>), ApiError> {
    auth.require_scope(ApiScope::TasksCreate)?;
    auth.ensure_product(&request.owner_product_id)?;
    if requests_remote_execution(&state, &request)? {
        auth.require_scope(ApiScope::TasksRemoteExecution)?;
        mark_remote_execution_approved(&mut request);
    }
    let service = scheduler_service(&state);
    let task = service.enqueue_task(request.into())?;

    Ok((StatusCode::CREATED, Json(SingleTaskResponse { task })))
}

fn requests_remote_execution(
    state: &AppState,
    request: &CreateTaskRequest,
) -> Result<bool, ApiError> {
    let Some(provider_id) = request.provider_id.as_deref() else {
        return Ok(false);
    };
    let registry = state.load_registry().map_err(ApiError::TaskRegistry)?;
    Ok(registry.get(provider_id).is_some_and(|provider| {
        provider.manifest.integration_type == crate::registry::IntegrationType::Http
    }))
}

fn mark_remote_execution_approved(request: &mut CreateTaskRequest) {
    let mut metadata = request
        .metadata
        .take()
        .unwrap_or_else(|| Value::Object(Default::default()));
    if let Some(object) = metadata.as_object_mut() {
        object.insert(
            "remote_execution".to_owned(),
            serde_json::json!({ "approved_by_scope": true }),
        );
    }
    request.metadata = Some(metadata);
}

pub async fn get(
    auth: ProductAuth,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<SingleTaskResponse>, ApiError> {
    let task = state.task_store().get(&task_id)?;
    auth.require_scope(ApiScope::TasksRead)?;
    auth.ensure_product(&task.owner_product_id)?;

    Ok(Json(SingleTaskResponse { task }))
}

pub async fn cancel(
    auth: ProductAuth,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<SingleTaskResponse>, ApiError> {
    auth.require_scope(ApiScope::TasksCancel)?;
    let current = state.task_store().get(&task_id)?;
    auth.ensure_product(&current.owner_product_id)?;
    let task = scheduler_service(&state).cancel_task(&task_id)?;

    Ok(Json(SingleTaskResponse { task }))
}

pub async fn events(
    auth: ProductAuth,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Query(query): Query<TaskEventsQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    auth.require_scope(ApiScope::TasksRead)?;
    let task = state.task_store().get(&task_id)?;
    auth.ensure_product(&task.owner_product_id)?;
    let last_event_id = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok());
    let service = state.task_event_service();
    let cursor = crate::task::service::TaskEventService::parse_cursor(
        query.cursor.as_deref(),
        last_event_id,
    )?;
    let receiver = service.stream(&task_id, cursor)?;
    let stream = ReceiverStream::new(receiver).map(|frame| match frame {
        TaskStreamFrame::Event(event) => Ok::<_, std::convert::Infallible>(
            axum::response::sse::Event::default()
                .id(event.sequence.to_string())
                .event(event.event_type.as_str())
                .data(serde_json::to_string(&TaskEventView::from(event)).unwrap()),
        ),
        TaskStreamFrame::Heartbeat => Ok::<_, std::convert::Infallible>(
            axum::response::sse::Event::default().comment("keep-alive"),
        ),
    });

    Ok(Sse::new(stream)
        .keep_alive(
            axum::response::sse::KeepAlive::new().interval(Duration::from_secs(365 * 24 * 60 * 60)),
        )
        .into_response())
}

pub async fn post_event(
    auth: ProductAuth,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Json(request): Json<TaskEventRequest>,
) -> Result<Json<TaskPermissionResponse>, ApiError> {
    auth.require_scope(ApiScope::TasksRead)?;
    let task = state.task_store().get(&task_id)?;
    auth.ensure_product(&task.owner_product_id)?;
    if request.event_type != "provider.permission_response" {
        return Err(ApiError::TaskEvents(
            TaskEventServiceError::InvalidEventRequest,
        ));
    }
    let decision = match request.decision.as_str() {
        "approve" => PermissionDecision::Approve,
        "deny" => PermissionDecision::Deny,
        _ => {
            return Err(ApiError::TaskEvents(
                TaskEventServiceError::InvalidPermissionDecision,
            ));
        }
    };
    let resolution = state.task_event_service().resolve_permission_response(
        &task_id,
        PermissionResponseRequest {
            request_id: request.request_id,
            decision,
            reason: request.reason,
        },
    )?;
    Ok(Json(TaskPermissionResponse {
        task_id,
        request_id: resolution.request_id,
        status: "resolved".to_owned(),
        decision: resolution.decision,
    }))
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
    Auth(AuthError),
    Task(TaskValidationError),
    TaskEvents(TaskEventServiceError),
    TaskRegistry(anyhow::Error),
}

impl From<AuthError> for ApiError {
    fn from(error: AuthError) -> Self {
        Self::Auth(error)
    }
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

impl From<TaskEventServiceError> for ApiError {
    fn from(error: TaskEventServiceError) -> Self {
        Self::TaskEvents(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Auth(error) => (error.status(), error.code(), error.message().to_owned()),
            Self::Task(error) => task_error_response(error),
            Self::TaskEvents(error) => task_event_error_response(error),
            Self::TaskRegistry(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "registry_error",
                error.to_string(),
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

fn task_event_error_response(error: TaskEventServiceError) -> (StatusCode, &'static str, String) {
    match error {
        TaskEventServiceError::InvalidCursor => (
            StatusCode::BAD_REQUEST,
            "invalid_event_cursor",
            "invalid event cursor".to_owned(),
        ),
        TaskEventServiceError::InvalidEventRequest => (
            StatusCode::BAD_REQUEST,
            "invalid_event_request",
            "invalid event request".to_owned(),
        ),
        TaskEventServiceError::InvalidPermissionDecision => (
            StatusCode::BAD_REQUEST,
            "invalid_permission_decision",
            "invalid permission decision".to_owned(),
        ),
        TaskEventServiceError::PermissionResponseNotSupported => (
            StatusCode::CONFLICT,
            "permission_response_not_supported",
            "permission response not supported".to_owned(),
        ),
        TaskEventServiceError::StorePayload(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "store_error",
            error.to_string(),
        ),
        TaskEventServiceError::Task(
            crate::store::tasks::TaskStoreError::PermissionRequestNotFound,
        ) => (
            StatusCode::NOT_FOUND,
            "permission_request_not_found",
            "permission request not found".to_owned(),
        ),
        TaskEventServiceError::Task(
            crate::store::tasks::TaskStoreError::PermissionRequestNotPending,
        ) => (
            StatusCode::CONFLICT,
            "permission_request_not_pending",
            "permission request not pending".to_owned(),
        ),
        TaskEventServiceError::Task(
            crate::store::tasks::TaskStoreError::PermissionRequestAlreadyResolved,
        ) => (
            StatusCode::CONFLICT,
            "permission_request_already_resolved",
            "permission request already resolved".to_owned(),
        ),
        TaskEventServiceError::Task(error) => task_error_response(error.into()),
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
        TaskValidationError::RemoteExecutionNotAllowed => (
            StatusCode::FORBIDDEN,
            "remote_execution_not_allowed",
            "remote execution not allowed".to_owned(),
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
