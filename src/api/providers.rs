use axum::{
    Json,
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::registry::{self, IntegrationType, ProviderManifest, ProviderStatus};

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

pub async fn list() -> Result<Json<ProviderListResponse>, ApiError> {
    let registry = registry::load_default_registry()?;
    let providers = registry
        .providers()
        .iter()
        .map(|entry| ProviderResponse::from_manifest(entry.manifest.clone()))
        .collect();

    Ok(Json(ProviderListResponse { providers }))
}

pub async fn get(
    Path(provider_id): Path<String>,
) -> Result<Json<SingleProviderResponse>, ApiError> {
    let registry = registry::load_default_registry()?;
    let provider = registry
        .get(&provider_id)
        .ok_or(ApiError::ProviderNotFound)?
        .manifest
        .clone();

    Ok(Json(SingleProviderResponse {
        provider: ProviderResponse::from_manifest(provider),
    }))
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
    ProviderNotFound,
    Registry(anyhow::Error),
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        Self::Registry(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
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
