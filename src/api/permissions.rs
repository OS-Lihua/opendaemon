use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    product::ApiScope,
    task::{
        event::PermissionDecision,
        permission::{PermissionRequestRecord, PermissionRequestStatus},
    },
};

use super::{
    AppState, AuthError, ErrorBody, ErrorResponse,
    auth::{AnyAuth, AuthContext},
};

#[derive(Debug, Deserialize)]
pub struct PermissionListQuery {
    pub status: Option<PermissionRequestStatus>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct PermissionListResponse {
    pub permissions: Vec<PermissionResponse>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct PermissionResponse {
    pub task_id: String,
    pub request_id: String,
    pub provider_id: String,
    pub permission_kind: String,
    pub summary: String,
    pub details: Option<Value>,
    pub options: Vec<PermissionDecision>,
    pub expires_at: Option<String>,
    pub created_at: String,
}

pub async fn list(
    auth: AnyAuth,
    State(state): State<AppState>,
    Query(query): Query<PermissionListQuery>,
) -> Result<Json<PermissionListResponse>, ApiError> {
    auth.require_scope(ApiScope::TasksRead)?;
    let owner_product_id = match &auth.0 {
        AuthContext::Bootstrap => None,
        AuthContext::Product(context) => Some(context.product_id.clone()),
    };
    let permissions = state
        .task_store()
        .list_permission_requests(crate::store::tasks::PermissionRequestFilters {
            owner_product_id,
            status: query.status,
        })?
        .into_iter()
        .map(PermissionResponse::from)
        .collect();

    Ok(Json(PermissionListResponse { permissions }))
}

impl From<PermissionRequestRecord> for PermissionResponse {
    fn from(record: PermissionRequestRecord) -> Self {
        Self {
            task_id: record.task_id,
            request_id: record.request_id,
            provider_id: record.provider_id,
            permission_kind: record.permission_kind,
            summary: record.request.summary,
            details: record.request.details,
            options: record.request.options,
            expires_at: record.request.expires_at,
            created_at: record.requested_at,
        }
    }
}

#[derive(Debug)]
pub enum ApiError {
    Auth(AuthError),
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
