use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    agent::profile::{AgentAuthorizationRequest, AgentProfileError},
    security::directory::{
        DirectoryCapability, DirectoryGrant, DirectoryLockPolicy, WorkspaceMode,
    },
    store::directory_grants::{
        CreateDirectoryGrant, DirectoryGrantFilters, DirectoryStoreError, PatchDirectoryGrant,
    },
};

use super::{AppState, ErrorBody, ErrorResponse};

pub type DirectoryResponse = DirectoryGrant;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct DirectoryListResponse {
    pub directories: Vec<DirectoryResponse>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SingleDirectoryResponse {
    pub directory: DirectoryResponse,
}

#[derive(Debug, Deserialize)]
pub struct DirectoryListQuery {
    pub product_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDirectoryGrantRequest {
    pub product_id: String,
    pub agent_id: String,
    pub path: std::path::PathBuf,
    pub capabilities: Vec<DirectoryCapability>,
    pub workspace_modes: Option<Vec<WorkspaceMode>>,
    pub default_workspace_mode: Option<WorkspaceMode>,
    pub lock_policy: Option<DirectoryLockPolicy>,
    pub direct_mode_requires_explicit_task_opt_in: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct PatchDirectoryGrantRequest {
    pub capabilities: Option<Vec<DirectoryCapability>>,
    pub workspace_modes: Option<Vec<WorkspaceMode>>,
    pub default_workspace_mode: Option<WorkspaceMode>,
    pub lock_policy: Option<DirectoryLockPolicy>,
    pub direct_mode_requires_explicit_task_opt_in: Option<bool>,
}

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<DirectoryListQuery>,
) -> Result<Json<DirectoryListResponse>, ApiError> {
    let directories = state.directory_grant_store().list(DirectoryGrantFilters {
        product_id: query.product_id,
        agent_id: query.agent_id,
    })?;

    Ok(Json(DirectoryListResponse { directories }))
}

pub async fn create(
    State(state): State<AppState>,
    Json(request): Json<CreateDirectoryGrantRequest>,
) -> Result<(StatusCode, Json<SingleDirectoryResponse>), ApiError> {
    state
        .agent_profile_store()
        .authorize(&AgentAuthorizationRequest {
            owner_product_id: request.product_id.clone(),
            agent_id: request.agent_id.clone(),
            provider_id_override: None,
            model_override: None,
            permission_mode_override: None,
            requested_workspace_mode: agent_workspace_mode(
                request
                    .default_workspace_mode
                    .unwrap_or(WorkspaceMode::Worktree),
            ),
        })?;
    let directory = state.directory_grant_store().create(request.into())?;

    Ok((
        StatusCode::CREATED,
        Json(SingleDirectoryResponse { directory }),
    ))
}

fn agent_workspace_mode(mode: WorkspaceMode) -> crate::agent::profile::WorkspaceMode {
    match mode {
        WorkspaceMode::Worktree => crate::agent::profile::WorkspaceMode::Worktree,
        WorkspaceMode::Direct => crate::agent::profile::WorkspaceMode::Direct,
    }
}

pub async fn get(
    State(state): State<AppState>,
    Path(directory_id): Path<String>,
) -> Result<Json<SingleDirectoryResponse>, ApiError> {
    let directory = state.directory_grant_store().get(&directory_id)?;

    Ok(Json(SingleDirectoryResponse { directory }))
}

pub async fn patch(
    State(state): State<AppState>,
    Path(directory_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<SingleDirectoryResponse>, ApiError> {
    reject_immutable_fields(&body)?;
    let request: PatchDirectoryGrantRequest =
        serde_json::from_value(body).map_err(|_| ApiError::BadPatch)?;
    let directory = state
        .directory_grant_store()
        .patch(&directory_id, request.into())?;

    Ok(Json(SingleDirectoryResponse { directory }))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(directory_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.directory_grant_store().delete(&directory_id)?;

    Ok(StatusCode::NO_CONTENT)
}

fn reject_immutable_fields(body: &Value) -> Result<(), ApiError> {
    let Some(object) = body.as_object() else {
        return Err(ApiError::BadPatch);
    };
    if object.keys().any(|key| {
        matches!(
            key.as_str(),
            "id" | "product_id" | "agent_id" | "path" | "created_at" | "updated_at"
        )
    }) {
        return Err(ApiError::BadPatch);
    }
    Ok(())
}

impl From<CreateDirectoryGrantRequest> for CreateDirectoryGrant {
    fn from(request: CreateDirectoryGrantRequest) -> Self {
        Self {
            product_id: request.product_id,
            agent_id: request.agent_id,
            path: request.path,
            capabilities: request.capabilities,
            workspace_modes: request.workspace_modes,
            default_workspace_mode: request.default_workspace_mode,
            lock_policy: request.lock_policy,
            direct_mode_requires_explicit_task_opt_in: request
                .direct_mode_requires_explicit_task_opt_in,
        }
    }
}

impl From<PatchDirectoryGrantRequest> for PatchDirectoryGrant {
    fn from(request: PatchDirectoryGrantRequest) -> Self {
        Self {
            capabilities: request.capabilities,
            workspace_modes: request.workspace_modes,
            default_workspace_mode: request.default_workspace_mode,
            lock_policy: request.lock_policy,
            direct_mode_requires_explicit_task_opt_in: request
                .direct_mode_requires_explicit_task_opt_in,
        }
    }
}

#[derive(Debug)]
pub enum ApiError {
    Directory(DirectoryStoreError),
    Agent(AgentProfileError),
    AgentStore(anyhow::Error),
    BadPatch,
}

impl From<DirectoryStoreError> for ApiError {
    fn from(error: DirectoryStoreError) -> Self {
        Self::Directory(error)
    }
}

impl From<crate::store::agent_profiles::AgentStoreError> for ApiError {
    fn from(error: crate::store::agent_profiles::AgentStoreError) -> Self {
        match error {
            crate::store::agent_profiles::AgentStoreError::Profile(error) => Self::Agent(error),
            crate::store::agent_profiles::AgentStoreError::Store(error) => Self::AgentStore(error),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::BadPatch => (
                StatusCode::BAD_REQUEST,
                "directory_authorization_failed",
                "invalid directory patch".to_owned(),
            ),
            Self::Directory(DirectoryStoreError::NotFound) => (
                StatusCode::NOT_FOUND,
                "directory_not_found",
                "directory grant not found".to_owned(),
            ),
            Self::Directory(DirectoryStoreError::Path(error)) => (
                StatusCode::BAD_REQUEST,
                error.code(),
                error.message().to_owned(),
            ),
            Self::Directory(DirectoryStoreError::Security(error)) => (
                match error {
                    crate::security::directory::DirectorySecurityError::DirectModeNotAllowed => {
                        StatusCode::FORBIDDEN
                    }
                    _ => StatusCode::BAD_REQUEST,
                },
                error.code(),
                error.message().to_owned(),
            ),
            Self::Directory(DirectoryStoreError::Store(error)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "store_error",
                error.to_string(),
            ),
            Self::Agent(error) => (
                match error {
                    AgentProfileError::AgentNotFound => StatusCode::NOT_FOUND,
                    AgentProfileError::AgentAuthorizationFailed => StatusCode::FORBIDDEN,
                    _ => StatusCode::BAD_REQUEST,
                },
                error.code(),
                error.message().to_owned(),
            ),
            Self::AgentStore(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "store_error",
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
