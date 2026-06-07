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
    product::ApiScope,
    security::directory::{
        DirectoryCapability, DirectoryGrant, DirectoryLockPolicy, WorkspaceMode,
    },
    store::directory_grants::{
        CreateDirectoryGrant, DirectoryGrantFilters, DirectoryStoreError, PatchDirectoryGrant,
    },
};

use super::{AppState, AuthError, ErrorBody, ErrorResponse, ProductAuth};

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
    pub allow_remote_execution: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct PatchDirectoryGrantRequest {
    pub capabilities: Option<Vec<DirectoryCapability>>,
    pub workspace_modes: Option<Vec<WorkspaceMode>>,
    pub default_workspace_mode: Option<WorkspaceMode>,
    pub lock_policy: Option<DirectoryLockPolicy>,
    pub direct_mode_requires_explicit_task_opt_in: Option<bool>,
    pub allow_remote_execution: Option<bool>,
}

pub async fn list(
    auth: ProductAuth,
    State(state): State<AppState>,
    Query(query): Query<DirectoryListQuery>,
) -> Result<Json<DirectoryListResponse>, ApiError> {
    auth.require_scope(ApiScope::DirectoriesRead)?;
    if let Some(product_id) = &query.product_id {
        auth.ensure_product(product_id)?;
    }
    let directories = state.directory_grant_store().list(DirectoryGrantFilters {
        product_id: Some(auth.product_id().to_owned()),
        agent_id: query.agent_id,
    })?;

    Ok(Json(DirectoryListResponse { directories }))
}

pub async fn create(
    auth: ProductAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateDirectoryGrantRequest>,
) -> Result<(StatusCode, Json<SingleDirectoryResponse>), ApiError> {
    auth.require_scope(ApiScope::DirectoriesGrant)?;
    auth.ensure_product(&request.product_id)?;
    require_direct_scope(
        &auth,
        &request.workspace_modes,
        request.default_workspace_mode,
    )?;
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
    auth: ProductAuth,
    State(state): State<AppState>,
    Path(directory_id): Path<String>,
) -> Result<Json<SingleDirectoryResponse>, ApiError> {
    let directory = state.directory_grant_store().get(&directory_id)?;
    auth.require_scope(ApiScope::DirectoriesRead)?;
    auth.ensure_product(&directory.product_id)?;

    Ok(Json(SingleDirectoryResponse { directory }))
}

pub async fn patch(
    auth: ProductAuth,
    State(state): State<AppState>,
    Path(directory_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<SingleDirectoryResponse>, ApiError> {
    auth.require_scope(ApiScope::DirectoriesGrant)?;
    reject_immutable_fields(&body)?;
    let request: PatchDirectoryGrantRequest =
        serde_json::from_value(body).map_err(|_| ApiError::BadPatch)?;
    let current = state.directory_grant_store().get(&directory_id)?;
    auth.ensure_product(&current.product_id)?;
    require_direct_scope(
        &auth,
        &request.workspace_modes,
        request.default_workspace_mode,
    )?;
    let directory = state
        .directory_grant_store()
        .patch(&directory_id, request.into())?;

    Ok(Json(SingleDirectoryResponse { directory }))
}

pub async fn delete(
    auth: ProductAuth,
    State(state): State<AppState>,
    Path(directory_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    auth.require_scope(ApiScope::DirectoriesGrant)?;
    let current = state.directory_grant_store().get(&directory_id)?;
    auth.ensure_product(&current.product_id)?;
    state.directory_grant_store().delete(&directory_id)?;

    Ok(StatusCode::NO_CONTENT)
}

fn require_direct_scope(
    auth: &ProductAuth,
    workspace_modes: &Option<Vec<WorkspaceMode>>,
    default_workspace_mode: Option<WorkspaceMode>,
) -> Result<(), ApiError> {
    let uses_direct = workspace_modes
        .as_ref()
        .is_some_and(|modes| modes.contains(&WorkspaceMode::Direct))
        || default_workspace_mode == Some(WorkspaceMode::Direct);
    if uses_direct {
        auth.require_scopes(&[ApiScope::DirectoriesGrant, ApiScope::DirectoriesDirect])?;
    }
    Ok(())
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
            allow_remote_execution: request.allow_remote_execution,
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
            allow_remote_execution: request.allow_remote_execution,
        }
    }
}

#[derive(Debug)]
pub enum ApiError {
    Auth(AuthError),
    Directory(DirectoryStoreError),
    Agent(AgentProfileError),
    AgentStore(anyhow::Error),
    BadPatch,
}

impl From<AuthError> for ApiError {
    fn from(error: AuthError) -> Self {
        Self::Auth(error)
    }
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
            Self::Auth(error) => (error.status(), error.code(), error.message().to_owned()),
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
