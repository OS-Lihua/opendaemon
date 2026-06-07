use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::sync::{broadcast, mpsc, oneshot};

use crate::{
    store::tasks::{TaskStore, TaskStoreError},
    task::{
        event::{PermissionDecisionEvent, TaskEvent, TaskEventType},
        permission::{PermissionResponseRequest, PermissionRequestStatus, PermissionResolution},
    },
};

#[derive(Debug)]
pub struct TaskEventBus {
    sender: broadcast::Sender<TaskEvent>,
    waiters: Mutex<HashMap<(String, String), oneshot::Sender<PermissionDecisionEvent>>>,
}

impl Default for TaskEventBus {
    fn default() -> Self {
        let (sender, _) = broadcast::channel(256);
        Self {
            sender,
            waiters: Mutex::new(HashMap::new()),
        }
    }
}

impl TaskEventBus {
    pub fn publish(&self, event: TaskEvent) {
        let _ = self.sender.send(event);
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<TaskEvent> {
        self.sender.subscribe()
    }

    #[must_use]
    pub fn has_waiter(&self, task_id: &str, request_id: &str) -> bool {
        self.waiters
            .lock()
            .expect("task event bus mutex poisoned")
            .contains_key(&(task_id.to_owned(), request_id.to_owned()))
    }

    pub fn register_waiter(
        &self,
        task_id: &str,
        request_id: &str,
    ) -> oneshot::Receiver<PermissionDecisionEvent> {
        let (sender, receiver) = oneshot::channel();
        self.waiters
            .lock()
            .expect("task event bus mutex poisoned")
            .insert((task_id.to_owned(), request_id.to_owned()), sender);
        receiver
    }

    pub fn notify_permission_resolution(
        &self,
        task_id: &str,
        decision: &PermissionDecisionEvent,
    ) {
        if let Some(waiter) = self
            .waiters
            .lock()
            .expect("task event bus mutex poisoned")
            .remove(&(task_id.to_owned(), decision.request_id.clone()))
        {
            let _ = waiter.send(decision.clone());
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskEventService {
    store: TaskStore,
    bus: SharedTaskEventBus,
    heartbeat_interval: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStreamFrame {
    Event(TaskEvent),
    Heartbeat,
}

#[derive(Debug)]
pub enum TaskEventServiceError {
    InvalidCursor,
    InvalidEventRequest,
    InvalidPermissionDecision,
    PermissionResponseNotSupported,
    StorePayload(anyhow::Error),
    Task(TaskStoreError),
}

impl From<TaskStoreError> for TaskEventServiceError {
    fn from(error: TaskStoreError) -> Self {
        Self::Task(error)
    }
}

impl TaskEventService {
    #[must_use]
    pub fn new(store: TaskStore, bus: SharedTaskEventBus, heartbeat_interval: Duration) -> Self {
        Self {
            store,
            bus,
            heartbeat_interval,
        }
    }

    pub fn parse_cursor(
        cursor: Option<&str>,
        last_event_id: Option<&str>,
    ) -> Result<i64, TaskEventServiceError> {
        let raw = cursor.or(last_event_id).unwrap_or("0");
        raw.parse::<i64>()
            .ok()
            .filter(|value| *value >= 0)
            .ok_or(TaskEventServiceError::InvalidCursor)
    }

    pub fn stream(
        &self,
        task_id: &str,
        cursor: i64,
    ) -> Result<mpsc::Receiver<TaskStreamFrame>, TaskEventServiceError> {
        let task = self.store.get(task_id)?;
        let mut live = self.bus.subscribe();
        let replay = self.store.list_events_after(task_id, cursor)?;
        let should_tail = !task.status.is_terminal()
            && !replay
                .last()
                .is_some_and(|event| is_terminal_event_type(event.event_type));
        let store = self.store.clone();
        let task_id = task_id.to_owned();
        let heartbeat_interval = self.heartbeat_interval;
        let (sender, receiver) = mpsc::channel(256);

        tokio::spawn(async move {
            let mut highest = cursor;

            for event in replay {
                highest = event.sequence;
                if sender.send(TaskStreamFrame::Event(event)).await.is_err() {
                    return;
                }
            }

            if !should_tail {
                return;
            }

            let mut heartbeat = tokio::time::interval(heartbeat_interval);
            heartbeat.tick().await;

            loop {
                tokio::select! {
                    _ = heartbeat.tick() => {
                        if sender.send(TaskStreamFrame::Heartbeat).await.is_err() {
                            return;
                        }
                    }
                    received = live.recv() => match received {
                        Ok(event) => {
                            if event.task_id != task_id || event.sequence <= highest {
                                continue;
                            }
                            highest = event.sequence;
                            let terminal = is_terminal_event_type(event.event_type);
                            if sender.send(TaskStreamFrame::Event(event)).await.is_err() {
                                return;
                            }
                            if terminal {
                                return;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            let Ok(events) = store.list_events_after(&task_id, highest) else {
                                return;
                            };
                            for event in events {
                                if event.sequence <= highest {
                                    continue;
                                }
                                highest = event.sequence;
                                let terminal = is_terminal_event_type(event.event_type);
                                if sender.send(TaskStreamFrame::Event(event)).await.is_err() {
                                    return;
                                }
                                if terminal {
                                    return;
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => return,
                    }
                }
            }
        });

        Ok(receiver)
    }

    pub async fn await_permission_decision(
        &self,
        task_id: &str,
        request_id: &str,
    ) -> Result<PermissionDecisionEvent, TaskEventServiceError> {
        let request = self.store.get_permission_request(task_id, request_id)?;
        if request.status != PermissionRequestStatus::Pending {
            return request.response.ok_or_else(|| {
                TaskEventServiceError::Task(TaskStoreError::PermissionRequestNotPending)
            });
        }

        let receiver = self.bus.register_waiter(task_id, request_id);
        receiver
            .await
            .map_err(|_| TaskEventServiceError::PermissionResponseNotSupported)
    }

    pub fn resolve_permission_response(
        &self,
        task_id: &str,
        response: PermissionResponseRequest,
    ) -> Result<PermissionResolution, TaskEventServiceError> {
        let existing = self.store.get_permission_request(task_id, &response.request_id)?;
        if existing.status == PermissionRequestStatus::Pending
            && !self.bus.has_waiter(task_id, &response.request_id)
        {
            return Err(TaskEventServiceError::PermissionResponseNotSupported);
        }

        let resolution = self.store.resolve_permission_request(
            task_id,
            &response.request_id,
            response.decision,
            response.reason,
        )?;
        let decided: PermissionDecisionEvent = serde_json::from_value(resolution.event.payload.clone())
            .map_err(|error| TaskEventServiceError::StorePayload(error.into()))?;
        if !resolution.duplicated {
            self.bus.publish(resolution.event.clone());
        }
        self.bus.notify_permission_resolution(task_id, &decided);
        Ok(resolution)
    }

    #[must_use]
    pub fn store(&self) -> &TaskStore {
        &self.store
    }

    pub fn publish(&self, event: TaskEvent) {
        self.bus.publish(event);
    }
}

#[must_use]
pub fn is_terminal_event_type(event_type: TaskEventType) -> bool {
    matches!(
        event_type,
        TaskEventType::Completed
            | TaskEventType::Failed
            | TaskEventType::Cancelled
            | TaskEventType::TimedOut
    )
}

pub type SharedTaskEventBus = Arc<TaskEventBus>;
