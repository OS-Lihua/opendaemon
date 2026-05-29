use axum::{Router, routing::get};

mod health;
mod providers;

pub use health::{HealthResponse, health};
pub use providers::{
    ErrorBody, ErrorResponse, ProviderListResponse, ProviderResponse, SingleProviderResponse,
};

#[cfg(test)]
pub(crate) use providers::{
    ApiError as ProviderApiError, get as provider_get, list as provider_list,
};

pub fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/providers", get(providers::list))
        .route("/v1/providers/{provider_id}", get(providers::get))
}
