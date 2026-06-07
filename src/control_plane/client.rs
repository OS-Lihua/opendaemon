use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::{task::JoinHandle, time::MissedTickBehavior};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

use crate::{
    api::AppState,
    config::{ControlPlaneConfig, StoreConfig},
    control_plane::{
        dispatch::{ControlPlaneDispatchError, ControlPlaneDispatchService},
        model::{DaemonRegistrationAccepted, DaemonRuntimeSummary},
        registration::DaemonRegistrationService,
    },
    runtime::store::RuntimeStore,
    store::daemon_state::{DaemonStateStore, DaemonStateStoreError},
    task::{
        event::{TaskEvent, TaskEventType},
        model::Task,
        service::SharedTaskEventBus,
    },
};

#[derive(Debug, Clone)]
pub struct ControlPlaneMessageHandler {
    dispatch: ControlPlaneDispatchService,
    daemon_state: DaemonStateStore,
}

#[derive(Debug)]
pub enum ControlPlaneClientError {
    InvalidMessage,
    Dispatch(ControlPlaneDispatchError),
    Store(DaemonStateStoreError),
}

#[derive(Debug)]
pub enum HandledControlPlaneMessage {
    Ignored,
    TaskDispatched(Task),
    TaskCancelled(Task),
    HeartbeatAcknowledged,
}

#[derive(Debug)]
pub struct ControlPlaneClient {
    config: ControlPlaneConfig,
    registration: DaemonRegistrationService,
    handler: ControlPlaneMessageHandler,
    runtime_store: RuntimeStore,
    task_store: crate::store::tasks::TaskStore,
    task_event_bus: SharedTaskEventBus,
    callback_service: ControlPlaneCallbackService,
}

#[derive(Debug, Clone, Default)]
pub struct ControlPlaneCallbackService {
    delivered: Arc<Mutex<BTreeSet<(String, i64)>>>,
}

impl ControlPlaneMessageHandler {
    #[must_use]
    pub fn new(dispatch: ControlPlaneDispatchService, daemon_state: DaemonStateStore) -> Self {
        Self {
            dispatch,
            daemon_state,
        }
    }

    pub async fn handle_text(
        &self,
        payload: &str,
    ) -> Result<HandledControlPlaneMessage, ControlPlaneClientError> {
        let envelope: Value =
            serde_json::from_str(payload).map_err(|_| ControlPlaneClientError::InvalidMessage)?;
        let message_type = envelope
            .get("type")
            .and_then(Value::as_str)
            .ok_or(ControlPlaneClientError::InvalidMessage)?;
        match message_type {
            "task_dispatch" => {
                let task = serde_json::from_value(
                    envelope
                        .get("task")
                        .cloned()
                        .ok_or(ControlPlaneClientError::InvalidMessage)?,
                )
                .map_err(|_| ControlPlaneClientError::InvalidMessage)?;
                let task = self
                    .dispatch
                    .ingest(task)
                    .await
                    .map_err(ControlPlaneClientError::Dispatch)?;
                Ok(HandledControlPlaneMessage::TaskDispatched(task))
            }
            "task_cancel" => {
                let remote_task_id = envelope
                    .get("remote_task_id")
                    .and_then(Value::as_str)
                    .ok_or(ControlPlaneClientError::InvalidMessage)?;
                let task = self
                    .dispatch
                    .cancel_remote_task(remote_task_id)
                    .map_err(ControlPlaneClientError::Dispatch)?;
                Ok(HandledControlPlaneMessage::TaskCancelled(task))
            }
            "heartbeat_ack" => {
                let heartbeat_at = envelope
                    .get("heartbeat_at")
                    .and_then(Value::as_str)
                    .ok_or(ControlPlaneClientError::InvalidMessage)?;
                if let Ok(record) = self.daemon_state.get_current() {
                    self.daemon_state
                        .mark_heartbeat(&record.daemon_id, heartbeat_at, record.status)
                        .map_err(ControlPlaneClientError::Store)?;
                }
                Ok(HandledControlPlaneMessage::HeartbeatAcknowledged)
            }
            _ => Ok(HandledControlPlaneMessage::Ignored),
        }
    }
}

impl ControlPlaneClient {
    #[must_use]
    pub fn new(
        config: ControlPlaneConfig,
        registration: DaemonRegistrationService,
        handler: ControlPlaneMessageHandler,
        runtime_store: RuntimeStore,
        task_store: crate::store::tasks::TaskStore,
        task_event_bus: SharedTaskEventBus,
    ) -> Self {
        Self {
            config,
            registration,
            handler,
            runtime_store,
            task_store,
            task_event_bus,
            callback_service: ControlPlaneCallbackService::default(),
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        if !self.config.enabled() {
            return Ok(());
        }
        let endpoint = self
            .config
            .endpoint
            .clone()
            .context("missing control plane endpoint")?;
        let (stream, _) = connect_async(endpoint.as_str())
            .await
            .context("failed to connect control plane websocket")?;
        let (mut writer, mut reader) = stream.split();
        let mut task_events = self.task_event_bus.subscribe();

        let registration = self
            .registration
            .build_registration_request(runtime_summaries(&self.runtime_store).await)?;
        writer
            .send(Message::Text(
                json!({
                    "type": "register",
                    "registration": registration,
                })
                .to_string(),
            ))
            .await
            .context("failed to send control plane registration")?;

        let mut heartbeat = tokio::time::interval(self.config.heartbeat_interval);
        heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = heartbeat.tick() => {
                    writer.send(Message::Text(build_heartbeat_message(&self.registration)?))
                        .await
                        .context("failed to send control plane heartbeat")?;
                }
                frame = reader.next() => {
                    match frame {
                        Some(Ok(Message::Text(text))) => {
                            if let Some(accepted) = parse_registration_accepted(&text)? {
                                self.registration.accept(accepted)?;
                                continue;
                            }
                            let _ = self.handler.handle_text(&text).await?;
                        }
                        Some(Ok(Message::Ping(payload))) => {
                            writer.send(Message::Pong(payload)).await
                                .context("failed to respond to control plane ping")?;
                        }
                        Some(Ok(Message::Close(_))) | None => break,
                        Some(Ok(_)) => {}
                        Some(Err(error)) => return Err(error).context("control plane websocket error"),
                    }
                }
                event = task_events.recv() => {
                    match event {
                        Ok(event) => {
                            if let Some(callback) = self.callback_service.callback_for_event(&self.task_store, &event)? {
                                writer.send(Message::Text(callback.to_string()))
                                    .await
                                    .context("failed to send control plane task callback")?;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }

        Ok(())
    }
}

pub fn spawn_if_enabled(state: AppState) -> anyhow::Result<Option<JoinHandle<()>>> {
    let config = ControlPlaneConfig::from_env();
    if !config.enabled() {
        return Ok(None);
    }
    let daemon_state = DaemonStateStore::open(StoreConfig::default())
        .map_err(|error| anyhow::anyhow!("failed to open daemon state store: {error:?}"))?;
    let registration = DaemonRegistrationService::new(config.clone(), daemon_state.clone());
    let handler = ControlPlaneMessageHandler::new(
        ControlPlaneDispatchService::new(state.clone()),
        daemon_state,
    );
    let runtime_store = state.runtime_store().clone();
    let task_store = state.task_store().clone();
    let task_event_bus = state.task_event_bus().clone();
    let client = ControlPlaneClient::new(
        config,
        registration,
        handler,
        runtime_store,
        task_store,
        task_event_bus,
    );
    Ok(Some(tokio::spawn(async move {
        if let Err(error) = client.run().await {
            tracing::warn!(%error, "control plane client stopped");
        }
    })))
}

impl ControlPlaneCallbackService {
    pub fn callback_for_event(
        &self,
        task_store: &crate::store::tasks::TaskStore,
        event: &TaskEvent,
    ) -> Result<Option<Value>, ControlPlaneClientError> {
        let callback_event = match event.event_type {
            TaskEventType::Completed => "task.completed",
            TaskEventType::Failed => "task.failed",
            TaskEventType::Cancelled => "task.cancelled",
            TaskEventType::TimedOut => "task.timed_out",
            _ => return Ok(None),
        };
        let task = task_store
            .get(&event.task_id)
            .map_err(crate::scheduler::service::TaskValidationError::from)
            .map_err(ControlPlaneDispatchError::Task)
            .map_err(ControlPlaneClientError::Dispatch)?;
        let Some(control_plane) = task
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("control_plane"))
        else {
            return Ok(None);
        };
        let remote_task_id = control_plane
            .get("remote_task_id")
            .and_then(Value::as_str)
            .ok_or(ControlPlaneClientError::InvalidMessage)?;
        let task_token = control_plane
            .get("task_token")
            .and_then(Value::as_str)
            .ok_or(ControlPlaneClientError::InvalidMessage)?;
        let dedupe_key = (task.id.clone(), event.sequence);
        let mut delivered = self
            .delivered
            .lock()
            .expect("control plane callback mutex poisoned");
        if delivered.contains(&dedupe_key) {
            return Ok(None);
        }
        delivered.insert(dedupe_key);
        Ok(Some(json!({
            "type": "task_callback",
            "event": callback_event,
            "remote_task_id": remote_task_id,
            "task_token": task_token,
            "task_id": task.id,
            "sequence": event.sequence,
            "status": task.status,
            "result": task.result,
            "error": event.payload.get("error").cloned(),
        })))
    }
}

async fn runtime_summaries(runtime_store: &RuntimeStore) -> Vec<DaemonRuntimeSummary> {
    runtime_store
        .snapshot()
        .await
        .into_iter()
        .map(|runtime| DaemonRuntimeSummary {
            provider_id: runtime.provider_id,
            kind: runtime.kind,
            status: runtime.status,
        })
        .collect()
}

fn parse_registration_accepted(
    payload: &str,
) -> Result<Option<DaemonRegistrationAccepted>, ControlPlaneClientError> {
    let envelope: Value =
        serde_json::from_str(payload).map_err(|_| ControlPlaneClientError::InvalidMessage)?;
    if envelope.get("type").and_then(Value::as_str) != Some("registration_accepted") {
        return Ok(None);
    }
    let accepted = serde_json::from_value(
        envelope
            .get("registration")
            .cloned()
            .ok_or(ControlPlaneClientError::InvalidMessage)?,
    )
    .map_err(|_| ControlPlaneClientError::InvalidMessage)?;
    Ok(Some(accepted))
}

fn build_heartbeat_message(
    registration: &DaemonRegistrationService,
) -> Result<String, ControlPlaneClientError> {
    let request = registration
        .build_registration_request(Vec::new())
        .map_err(|_| ControlPlaneClientError::InvalidMessage)?;
    Ok(json!({
        "type": "heartbeat",
        "daemon_id": request.daemon_id,
        "session_id": request.session_id,
        "sent_at": now_rfc3339(),
    })
    .to_string())
}

fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("UTC timestamps must format as RFC3339")
}

impl From<DaemonStateStoreError> for ControlPlaneClientError {
    fn from(error: DaemonStateStoreError) -> Self {
        Self::Store(error)
    }
}

impl From<crate::control_plane::registration::DaemonRegistrationError> for anyhow::Error {
    fn from(error: crate::control_plane::registration::DaemonRegistrationError) -> Self {
        anyhow::anyhow!("{error:?}")
    }
}

impl From<ControlPlaneClientError> for anyhow::Error {
    fn from(error: ControlPlaneClientError) -> Self {
        anyhow::anyhow!("{error:?}")
    }
}
