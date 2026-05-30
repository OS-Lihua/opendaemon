use std::path::PathBuf;

use axum::{
    Router,
    routing::{get, post},
};

mod health;
mod providers;
mod runtimes;

pub use health::{HealthResponse, health};
pub use providers::{
    ErrorBody, ErrorResponse, ProviderListResponse, ProviderResponse, SingleProviderResponse,
};
pub use runtimes::{RuntimeListResponse, RuntimeResponse};

use crate::{
    config::RuntimeDetectionConfig,
    registry::{self, ProviderRegistry},
    runtime::store::RuntimeStore,
};

#[cfg(test)]
pub(crate) use providers::{
    ApiError as ProviderApiError, get as provider_get, list as provider_list,
};
#[cfg(test)]
pub(crate) use runtimes::{detect as runtime_detect, list as runtime_list};

#[derive(Debug, Clone)]
pub struct AppState {
    providers_dir: PathBuf,
    runtime_store: RuntimeStore,
    runtime_detection_config: RuntimeDetectionConfig,
}

impl AppState {
    #[must_use]
    pub fn new(
        providers_dir: PathBuf,
        runtime_store: RuntimeStore,
        runtime_detection_config: RuntimeDetectionConfig,
    ) -> Self {
        Self {
            providers_dir,
            runtime_store,
            runtime_detection_config,
        }
    }

    pub fn load_registry(&self) -> anyhow::Result<ProviderRegistry> {
        registry::load_registry_from_dir(&self.providers_dir)
    }

    #[must_use]
    pub fn runtime_store(&self) -> &RuntimeStore {
        &self.runtime_store
    }

    #[must_use]
    pub fn runtime_detection_config(&self) -> &RuntimeDetectionConfig {
        &self.runtime_detection_config
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(
            registry::default_providers_dir(),
            RuntimeStore::default(),
            RuntimeDetectionConfig::default(),
        )
    }
}

pub fn router() -> Router {
    router_with_state(AppState::default())
}

pub fn router_with_state(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/providers", get(providers::list))
        .route("/v1/providers/{provider_id}", get(providers::get))
        .route("/v1/runtimes", get(runtimes::list))
        .route("/v1/runtimes/detect", post(runtimes::detect))
        .with_state(state)
}
