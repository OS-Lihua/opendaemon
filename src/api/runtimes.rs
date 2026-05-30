use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::{registry::ProviderManifest, runtime};

use super::{AppState, ErrorBody, ErrorResponse};

pub type RuntimeResponse = runtime::model::RuntimeView;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RuntimeListResponse {
    pub runtimes: Vec<RuntimeResponse>,
}

pub async fn list(State(state): State<AppState>) -> Result<Json<RuntimeListResponse>, ApiError> {
    let providers = load_provider_manifests(&state)?;
    let runtimes = state.runtime_store().list_for_providers(&providers).await;

    Ok(Json(RuntimeListResponse { runtimes }))
}

pub async fn detect(State(state): State<AppState>) -> Result<Json<RuntimeListResponse>, ApiError> {
    let providers = load_provider_manifests(&state)?;
    let runtimes =
        runtime::detect::detect_providers(&providers, state.runtime_detection_config()).await;

    state.runtime_store().save_all(runtimes).await;

    let runtimes = state.runtime_store().list_for_providers(&providers).await;
    Ok(Json(RuntimeListResponse { runtimes }))
}

fn load_provider_manifests(state: &AppState) -> Result<Vec<ProviderManifest>, ApiError> {
    Ok(state
        .load_registry()?
        .providers()
        .iter()
        .map(|entry| entry.manifest.clone())
        .collect())
}

#[derive(Debug)]
pub enum ApiError {
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
