use std::path::PathBuf;

use axum::{
    Router,
    routing::{get, post},
};

pub(crate) mod agents;
pub(crate) mod directories;
mod health;
mod providers;
mod runtimes;
pub(crate) mod tasks;

pub use agents::{AgentListResponse, AgentResponse, SingleAgentResponse};
pub use directories::{DirectoryListResponse, DirectoryResponse, SingleDirectoryResponse};
pub use health::{HealthResponse, health};
pub use providers::{
    ErrorBody, ErrorResponse, ProviderListResponse, ProviderResponse, SingleProviderResponse,
};
pub use runtimes::{RuntimeListResponse, RuntimeResponse};
pub use tasks::{SingleTaskResponse, TaskListResponse, TaskResponse};

use crate::{
    config::{RuntimeDetectionConfig, SchedulerConfig, StoreConfig},
    registry::{self, ProviderRegistry},
    runtime::store::RuntimeStore,
    store::{
        agent_profiles::AgentProfileStore, directory_grants::DirectoryGrantStore, tasks::TaskStore,
    },
};

#[cfg(test)]
pub(crate) use agents::{
    create as agent_create, delete as agent_delete, get as agent_get, list as agent_list,
    patch as agent_patch,
};
#[cfg(test)]
pub(crate) use directories::{
    create as directory_create, delete as directory_delete, get as directory_get,
    list as directory_list, patch as directory_patch,
};
#[cfg(test)]
pub(crate) use providers::{
    ApiError as ProviderApiError, get as provider_get, list as provider_list,
};
#[cfg(test)]
pub(crate) use runtimes::{detect as runtime_detect, list as runtime_list};
#[cfg(test)]
pub(crate) use tasks::{
    cancel as task_cancel, create as task_create, get as task_get, list as task_list,
};

#[derive(Debug, Clone)]
pub struct AppState {
    providers_dir: PathBuf,
    runtime_store: RuntimeStore,
    runtime_detection_config: RuntimeDetectionConfig,
    directory_grant_store: DirectoryGrantStore,
    agent_profile_store: AgentProfileStore,
    task_store: TaskStore,
    scheduler_config: SchedulerConfig,
}

impl AppState {
    #[must_use]
    pub fn new(
        providers_dir: PathBuf,
        runtime_store: RuntimeStore,
        runtime_detection_config: RuntimeDetectionConfig,
    ) -> Self {
        Self::with_directory_grant_store(
            providers_dir,
            runtime_store,
            runtime_detection_config,
            DirectoryGrantStore::configured(StoreConfig::default()),
        )
    }

    #[must_use]
    pub fn with_directory_grant_store(
        providers_dir: PathBuf,
        runtime_store: RuntimeStore,
        runtime_detection_config: RuntimeDetectionConfig,
        directory_grant_store: DirectoryGrantStore,
    ) -> Self {
        Self::with_task_store(
            providers_dir,
            runtime_store,
            runtime_detection_config,
            directory_grant_store,
            AgentProfileStore::configured(StoreConfig::default()),
            TaskStore::configured(StoreConfig::default()),
            SchedulerConfig::default(),
        )
    }

    #[must_use]
    pub fn with_stores(
        providers_dir: PathBuf,
        runtime_store: RuntimeStore,
        runtime_detection_config: RuntimeDetectionConfig,
        directory_grant_store: DirectoryGrantStore,
        agent_profile_store: AgentProfileStore,
    ) -> Self {
        Self::with_task_store(
            providers_dir,
            runtime_store,
            runtime_detection_config,
            directory_grant_store,
            agent_profile_store,
            TaskStore::configured(StoreConfig::default()),
            SchedulerConfig::default(),
        )
    }

    #[must_use]
    pub fn with_task_store(
        providers_dir: PathBuf,
        runtime_store: RuntimeStore,
        runtime_detection_config: RuntimeDetectionConfig,
        directory_grant_store: DirectoryGrantStore,
        agent_profile_store: AgentProfileStore,
        task_store: TaskStore,
        scheduler_config: SchedulerConfig,
    ) -> Self {
        Self {
            providers_dir,
            runtime_store,
            runtime_detection_config,
            directory_grant_store,
            agent_profile_store,
            task_store,
            scheduler_config,
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

    #[must_use]
    pub fn directory_grant_store(&self) -> &DirectoryGrantStore {
        &self.directory_grant_store
    }

    #[must_use]
    pub fn agent_profile_store(&self) -> &AgentProfileStore {
        &self.agent_profile_store
    }

    #[must_use]
    pub fn task_store(&self) -> &TaskStore {
        &self.task_store
    }

    #[must_use]
    pub fn scheduler_config(&self) -> SchedulerConfig {
        self.scheduler_config
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::with_task_store(
            registry::default_providers_dir(),
            RuntimeStore::default(),
            RuntimeDetectionConfig::default(),
            DirectoryGrantStore::configured(StoreConfig::default()),
            AgentProfileStore::configured(StoreConfig::default()),
            TaskStore::configured(StoreConfig::default()),
            SchedulerConfig::default(),
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
        .route("/v1/tasks", get(tasks::list).post(tasks::create))
        .route("/v1/tasks/{task_id}", get(tasks::get))
        .route("/v1/tasks/{task_id}/cancel", post(tasks::cancel))
        .route("/v1/agents", get(agents::list).post(agents::create))
        .route(
            "/v1/agents/{agent_id}",
            get(agents::get).patch(agents::patch).delete(agents::delete),
        )
        .route("/v1/directories", get(directories::list))
        .route("/v1/directories/grant", post(directories::create))
        .route(
            "/v1/directories/{directory_id}",
            get(directories::get)
                .patch(directories::patch)
                .delete(directories::delete),
        )
        .with_state(state)
}
