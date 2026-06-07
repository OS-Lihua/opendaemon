use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::{product::ApiScope, registry::ProviderManifest, runtime};

use super::{AppState, AuthError, ErrorBody, ErrorResponse, ProductAuth};

pub type RuntimeResponse = runtime::model::RuntimeView;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RuntimeListResponse {
    pub runtimes: Vec<RuntimeResponse>,
}

pub async fn list(
    auth: ProductAuth,
    State(state): State<AppState>,
) -> Result<Json<RuntimeListResponse>, ApiError> {
    auth.require_scope(ApiScope::RuntimesRead)?;
    let providers = load_provider_manifests(&state)?;
    let runtimes = state.runtime_store().list_for_providers(&providers).await;

    Ok(Json(RuntimeListResponse { runtimes }))
}

pub async fn detect(
    auth: ProductAuth,
    State(state): State<AppState>,
) -> Result<Json<RuntimeListResponse>, ApiError> {
    auth.require_scope(ApiScope::RuntimesRead)?;
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
    Auth(AuthError),
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
