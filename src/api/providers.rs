use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::registry::{IntegrationType, ProviderManifest, ProviderStatus};

use crate::product::ApiScope;

use super::{
    AppState, AuthError,
    auth::{AnyAuth, AuthContext},
};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ProviderListResponse {
    pub providers: Vec<ProviderResponse>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SingleProviderResponse {
    pub provider: ProviderResponse,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ProviderResponse {
    pub id: String,
    pub display_name: String,
    pub status: ProviderStatus,
    pub integration_type: IntegrationType,
    pub description: String,
    pub manifest: ProviderManifest,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ErrorResponse {
    pub error: ErrorBody,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
}

pub async fn list(
    auth: AnyAuth,
    State(state): State<AppState>,
) -> Result<Json<ProviderListResponse>, ApiError> {
    require_scope(&auth, ApiScope::ProvidersRead)?;
    let registry = state.load_registry()?;
    let providers = registry
        .providers()
        .iter()
        .map(|entry| ProviderResponse::from_manifest(entry.manifest.clone()))
        .collect();

    Ok(Json(ProviderListResponse { providers }))
}

pub async fn get(
    auth: AnyAuth,
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
) -> Result<Json<SingleProviderResponse>, ApiError> {
    require_scope(&auth, ApiScope::ProvidersRead)?;
    let registry = state.load_registry()?;
    let provider = registry
        .get(&provider_id)
        .ok_or(ApiError::ProviderNotFound)?
        .manifest
        .clone();

    Ok(Json(SingleProviderResponse {
        provider: ProviderResponse::from_manifest(provider),
    }))
}

fn require_scope(auth: &AnyAuth, scope: ApiScope) -> Result<(), AuthError> {
    match &auth.0 {
        AuthContext::Bootstrap => Ok(()),
        AuthContext::Product(_) => auth.require_scope(scope),
    }
}

impl ProviderResponse {
    fn from_manifest(manifest: ProviderManifest) -> Self {
        Self {
            id: manifest.id.clone(),
            display_name: manifest.display_name.clone(),
            status: manifest.status,
            integration_type: manifest.integration_type,
            description: manifest.description.clone(),
            manifest,
        }
    }
}

#[derive(Debug)]
pub enum ApiError {
    Auth(AuthError),
    ProviderNotFound,
    Registry(anyhow::Error),
}

impl From<AuthError> for ApiError {
    fn from(error: AuthError) -> Self {
        Self::Auth(error)
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        Self::Registry(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Auth(error) => (error.status(), error.code(), error.message().to_owned()),
            Self::ProviderNotFound => (
                StatusCode::NOT_FOUND,
                "provider_not_found",
                "provider not found".to_owned(),
            ),
            Self::Registry(error) => (
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
