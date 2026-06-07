use crate::{
    config::ControlPlaneConfig,
    control_plane::model::{
        DaemonConnectionStatus, DaemonRegistrationAccepted, DaemonRegistrationRecord,
        DaemonRegistrationRequest, DaemonRuntimeSummary,
    },
    store::daemon_state::{DaemonStateStore, DaemonStateStoreError},
};

#[derive(Debug, Clone)]
pub struct DaemonRegistrationService {
    config: ControlPlaneConfig,
    store: DaemonStateStore,
}

#[derive(Debug)]
pub enum DaemonRegistrationError {
    ControlPlaneDisabled,
    Store(DaemonStateStoreError),
}

impl DaemonRegistrationService {
    #[must_use]
    pub fn new(config: ControlPlaneConfig, store: DaemonStateStore) -> Self {
        Self { config, store }
    }

    pub fn build_registration_request(
        &self,
        runtimes: Vec<DaemonRuntimeSummary>,
    ) -> Result<DaemonRegistrationRequest, DaemonRegistrationError> {
        if !self.config.enabled() {
            return Err(DaemonRegistrationError::ControlPlaneDisabled);
        }
        let persisted = match self.store.get_current() {
            Ok(record) => Some(record),
            Err(DaemonStateStoreError::NotFound) => None,
            Err(error) => return Err(DaemonRegistrationError::Store(error)),
        };
        Ok(DaemonRegistrationRequest {
            daemon_id: persisted.as_ref().map(|record| record.daemon_id.clone()),
            session_id: persisted
                .as_ref()
                .and_then(|record| record.session_id.clone()),
            enrollment_secret: self
                .config
                .enrollment_secret
                .clone()
                .expect("enabled control plane config must include enrollment secret"),
            daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
            platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
            capabilities: vec![
                "task_dispatch".to_owned(),
                "task_cancel".to_owned(),
                "runtime_status".to_owned(),
            ],
            runtimes,
        })
    }

    pub fn accept(
        &self,
        accepted: DaemonRegistrationAccepted,
    ) -> Result<DaemonRegistrationRecord, DaemonRegistrationError> {
        self.store
            .save_registration(DaemonRegistrationRecord {
                daemon_id: accepted.daemon_id,
                control_plane_url: self
                    .config
                    .endpoint
                    .clone()
                    .expect("enabled control plane config must include endpoint"),
                daemon_token: accepted.daemon_token,
                status: DaemonConnectionStatus::Online,
                registered_at: accepted.registered_at.clone(),
                last_heartbeat_at: Some(accepted.registered_at),
                last_error_code: None,
                session_id: accepted.session_id,
            })
            .map_err(DaemonRegistrationError::Store)
    }
}
