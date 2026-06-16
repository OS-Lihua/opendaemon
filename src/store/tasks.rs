use std::{path::PathBuf, sync::Arc};

use rusqlite::{Connection, OptionalExtension, ToSql, params};
use serde_json::Value;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    config::StoreConfig,
    scheduler::locks::{DirectoryLock, LockMode, LockRequest},
    security::directory::WorkspaceMode,
    task::{
        event::{
            PermissionDecision, PermissionDecisionEvent, PermissionRequestEvent, TaskEvent,
            TaskEventType,
        },
        model::{CreateTask, Task, TaskModelError, TaskStatus},
        permission::{PermissionRequestRecord, PermissionRequestStatus, PermissionResolution},
        result::TaskResult,
        service::SharedTaskEventBus,
        state::{TaskStateError, TaskTransition, validate_transition},
    },
};

use self::codec::{
    row_to_event, row_to_lock, row_to_permission_request, row_to_task, serialize_json,
    serialize_optional_json,
};
use super::sqlite;

mod codec;

#[derive(Debug, Clone)]
pub struct TaskStore {
    sqlite_path: Arc<PathBuf>,
    event_bus: Option<SharedTaskEventBus>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskFilters {
    pub owner_product_id: Option<String>,
    pub agent_id: Option<String>,
    pub directory_id: Option<String>,
    pub status: Option<TaskStatus>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PermissionRequestFilters {
    pub owner_product_id: Option<String>,
    pub status: Option<PermissionRequestStatus>,
}

#[derive(Debug)]
pub enum TaskStoreError {
    NotFound,
    PermissionRequestNotFound,
    PermissionRequestNotPending,
    PermissionRequestAlreadyResolved,
    Model(TaskModelError),
    State(TaskStateError),
    Store(anyhow::Error),
}

impl From<rusqlite::Error> for TaskStoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

impl TaskStore {
    #[must_use]
    pub fn configured(config: StoreConfig) -> Self {
        Self {
            sqlite_path: Arc::new(config.sqlite_path),
            event_bus: None,
        }
    }

    #[must_use]
    pub fn with_event_bus(mut self, event_bus: SharedTaskEventBus) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    pub fn open(config: StoreConfig) -> Result<Self, TaskStoreError> {
        sqlite::open_connection(&config.sqlite_path).map_err(store_error)?;
        Ok(Self::configured(config))
    }

    #[must_use]
    pub fn event_bus(&self) -> Option<SharedTaskEventBus> {
        self.event_bus.clone()
    }

    pub fn create(&self, input: CreateTask) -> Result<Task, TaskStoreError> {
        input.validate().map_err(TaskStoreError::Model)?;
        let now = now_rfc3339()?;
        let required_capabilities = input.required_capabilities();
        let workspace_mode = input.workspace_mode.unwrap_or(WorkspaceMode::Worktree);
        let provider_id = input.provider_id.unwrap_or_default();
        let model = input.model.unwrap_or_default();
        let permission_mode = input
            .permission_mode
            .unwrap_or_else(|| "provider_default".to_owned());
        let metadata = input.metadata;
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(store_error)?;

        transaction
            .execute(
                "INSERT INTO tasks (
                    id,
                    owner_product_id,
                    agent_id,
                    directory_id,
                    prompt,
                    required_capabilities_json,
                    workspace_mode,
                    direct_mode_task_opt_in,
                    metadata_json,
                    provider_id,
                    model,
                    permission_mode,
                    timeout_seconds,
                    status,
                    result_json,
                    created_at,
                    updated_at
                ) VALUES ('', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, NULL, ?14, ?15)",
                params![
                    input.owner_product_id,
                    input.agent_id,
                    input.directory_id,
                    input.prompt,
                    serialize_json(&required_capabilities)?,
                    serialize_json(&workspace_mode)?,
                    input.direct_mode_task_opt_in,
                    serialize_optional_json(&metadata)?,
                    provider_id,
                    model,
                    permission_mode,
                    input.timeout_seconds,
                    serialize_json(&TaskStatus::Queued)?,
                    now,
                    now,
                ],
            )
            .map_err(store_error)?;
        let row_id = transaction.last_insert_rowid();
        let id = format!("task_{row_id}");
        transaction
            .execute(
                "UPDATE tasks SET id = ?1 WHERE rowid = ?2",
                params![id, row_id],
            )
            .map_err(store_error)?;
        insert_event_tx(
            &transaction,
            &id,
            1,
            TaskEventType::Queued,
            Value::Object(Default::default()),
            &now,
        )?;
        transaction.commit().map_err(store_error)?;
        self.publish_if_configured(self.event_by_sequence(&self.connection()?, &id, 1)?);
        self.get(&id)
    }

    pub fn list(&self, filters: TaskFilters) -> Result<Vec<Task>, TaskStoreError> {
        let tasks = self.all()?;
        Ok(tasks
            .into_iter()
            .filter(|task| {
                filters
                    .owner_product_id
                    .as_ref()
                    .is_none_or(|value| value == &task.owner_product_id)
                    && filters
                        .agent_id
                        .as_ref()
                        .is_none_or(|value| value == &task.agent_id)
                    && filters
                        .directory_id
                        .as_ref()
                        .is_none_or(|value| value == &task.directory_id)
                    && filters.status.is_none_or(|value| value == task.status)
            })
            .collect())
    }

    pub fn get(&self, id: &str) -> Result<Task, TaskStoreError> {
        let connection = self.connection()?;
        query_one(&connection, id)?.ok_or(TaskStoreError::NotFound)
    }

    pub fn transition(
        &self,
        id: &str,
        next: TaskStatus,
        payload: Option<Value>,
    ) -> Result<Task, TaskStoreError> {
        let current = self.get(id)?;
        match validate_transition(current.status, next).map_err(TaskStoreError::State)? {
            TaskTransition::Idempotent => return Ok(current),
            TaskTransition::Changed => {}
        }

        let now = now_rfc3339()?;
        let sequence = self.next_sequence(id)?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(store_error)?;
        transaction
            .execute(
                "UPDATE tasks
                 SET status = ?1,
                     updated_at = ?2,
                     started_at = COALESCE(started_at, ?3),
                     completed_at = COALESCE(completed_at, ?4),
                     cancelled_at = COALESCE(cancelled_at, ?5),
                     failed_at = COALESCE(failed_at, ?6)
                 WHERE id = ?7",
                params![
                    serialize_json(&next)?,
                    now,
                    timestamp_if(next == TaskStatus::Running, &now),
                    timestamp_if(next == TaskStatus::Completed, &now),
                    timestamp_if(next == TaskStatus::Cancelled, &now),
                    timestamp_if(
                        matches!(next, TaskStatus::Failed | TaskStatus::TimedOut),
                        &now
                    ),
                    id,
                ],
            )
            .map_err(store_error)?;
        insert_event_tx(
            &transaction,
            id,
            sequence,
            event_type_for_status(next),
            payload.unwrap_or_else(|| Value::Object(Default::default())),
            &now,
        )?;
        if next.is_terminal() {
            release_locks_tx(&transaction, id, &now)?;
        }
        transaction.commit().map_err(store_error)?;
        self.publish_if_configured(self.event_by_sequence(&self.connection()?, id, sequence)?);
        self.get(id)
    }

    pub fn cancel(&self, id: &str) -> Result<Task, TaskStoreError> {
        self.transition(id, TaskStatus::Cancelled, None)
    }

    pub fn save_result(
        &self,
        id: &str,
        final_message: &str,
        changed_files: Vec<String>,
    ) -> Result<(), TaskStoreError> {
        let task = self.get(id)?;
        let now = now_rfc3339()?;
        let result = TaskResult {
            task_id: task.id.clone(),
            status: task.status,
            final_message: final_message.to_owned(),
            changed_files,
            diff: None,
            workspace_mode: task.workspace_mode,
            worktree_path: None,
            source_directory_id: task.directory_id,
            branch_name: None,
            commit_hash: None,
            session_id: None,
            provider_result: None,
            usage: None,
            artifacts: Vec::new(),
            error: None,
            created_at: now.clone(),
            updated_at: now,
        };
        let connection = self.connection()?;
        connection
            .execute(
                "UPDATE tasks SET result_json = ?1, updated_at = ?2 WHERE id = ?3",
                params![serialize_json(&result)?, result.updated_at, id],
            )
            .map_err(store_error)?;
        Ok(())
    }

    pub fn save_execution_result(&self, result: &TaskResult) -> Result<(), TaskStoreError> {
        let connection = self.connection()?;
        connection
            .execute(
                "UPDATE tasks SET result_json = ?1, updated_at = ?2 WHERE id = ?3",
                params![serialize_json(result)?, result.updated_at, result.task_id],
            )
            .map_err(store_error)?;
        Ok(())
    }

    pub fn append_event(
        &self,
        task_id: &str,
        event_type: TaskEventType,
        payload: Value,
    ) -> Result<TaskEvent, TaskStoreError> {
        self.get(task_id)?;
        let now = now_rfc3339()?;
        let sequence = self.next_sequence(task_id)?;
        let connection = self.connection()?;
        insert_event_tx(&connection, task_id, sequence, event_type, payload, &now)?;
        let event = self.event_by_sequence(&connection, task_id, sequence)?;
        self.publish_if_configured(event.clone());
        Ok(event)
    }

    pub fn list_events(&self, task_id: &str) -> Result<Vec<TaskEvent>, TaskStoreError> {
        self.list_events_after(task_id, 0)
    }

    pub fn list_events_after(
        &self,
        task_id: &str,
        cursor: i64,
    ) -> Result<Vec<TaskEvent>, TaskStoreError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT * FROM task_events
                 WHERE task_id = ?1 AND sequence > ?2
                 ORDER BY sequence ASC",
            )
            .map_err(store_error)?;
        statement
            .query_map(params![task_id, cursor], row_to_event)
            .map_err(store_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(store_error)?
            .into_iter()
            .collect()
    }

    pub fn record_permission_request(
        &self,
        task_id: &str,
        request: PermissionRequestEvent,
    ) -> Result<TaskEvent, TaskStoreError> {
        self.get(task_id)?;
        let now = now_rfc3339()?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(store_error)?;
        let sequence = next_sequence_tx(&transaction, task_id)?;
        let payload = serde_json::to_value(&request).map_err(store_error)?;
        insert_event_tx(
            &transaction,
            task_id,
            sequence,
            TaskEventType::ProviderPermissionRequested,
            payload,
            &now,
        )?;
        transaction
            .execute(
                "INSERT INTO task_permission_requests (
                    request_id,
                    task_id,
                    sequence,
                    provider_id,
                    permission_kind,
                    status,
                    request_payload_json,
                    response_payload_json,
                    requested_at,
                    responded_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, NULL)",
                params![
                    request.request_id,
                    task_id,
                    sequence,
                    request.provider_id,
                    request.permission_kind,
                    serialize_json(&PermissionRequestStatus::Pending)?,
                    serialize_json(&request)?,
                    now,
                ],
            )
            .map_err(store_error)?;
        transaction.commit().map_err(store_error)?;
        let connection = self.connection()?;
        let event = self.event_by_sequence(&connection, task_id, sequence)?;
        self.publish_if_configured(event.clone());
        Ok(event)
    }

    pub fn get_permission_request(
        &self,
        task_id: &str,
        request_id: &str,
    ) -> Result<PermissionRequestRecord, TaskStoreError> {
        self.get(task_id)?;
        let connection = self.connection()?;
        query_permission_request(&connection, task_id, request_id)?
            .ok_or(TaskStoreError::PermissionRequestNotFound)
    }

    pub fn list_permission_requests(
        &self,
        filters: PermissionRequestFilters,
    ) -> Result<Vec<PermissionRequestRecord>, TaskStoreError> {
        let connection = self.connection()?;
        match (filters.owner_product_id, filters.status) {
            (Some(owner_product_id), Some(status)) => {
                let status_json = serialize_json(&status)?;
                query_permission_requests(
                    &connection,
                    "SELECT pr.* FROM task_permission_requests pr
                     JOIN tasks t ON t.id = pr.task_id
                     WHERE t.owner_product_id = ?1 AND pr.status = ?2
                     ORDER BY pr.rowid ASC",
                    &[&owner_product_id, &status_json],
                )
            }
            (Some(owner_product_id), None) => query_permission_requests(
                &connection,
                "SELECT pr.* FROM task_permission_requests pr
                 JOIN tasks t ON t.id = pr.task_id
                 WHERE t.owner_product_id = ?1
                 ORDER BY pr.rowid ASC",
                &[&owner_product_id],
            ),
            (None, Some(status)) => {
                let status_json = serialize_json(&status)?;
                query_permission_requests(
                    &connection,
                    "SELECT pr.* FROM task_permission_requests pr
                     JOIN tasks t ON t.id = pr.task_id
                     WHERE pr.status = ?1
                     ORDER BY pr.rowid ASC",
                    &[&status_json],
                )
            }
            (None, None) => query_permission_requests(
                &connection,
                "SELECT pr.* FROM task_permission_requests pr
                 JOIN tasks t ON t.id = pr.task_id
                 ORDER BY pr.rowid ASC",
                &[],
            ),
        }
    }

    pub fn resolve_permission_request(
        &self,
        task_id: &str,
        request_id: &str,
        decision: PermissionDecision,
        reason: Option<String>,
    ) -> Result<PermissionResolution, TaskStoreError> {
        self.get(task_id)?;
        let now = now_rfc3339()?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(store_error)?;
        let request = query_permission_request(&transaction, task_id, request_id)?
            .ok_or(TaskStoreError::PermissionRequestNotFound)?;

        match request.status {
            PermissionRequestStatus::Pending => {}
            PermissionRequestStatus::Approved | PermissionRequestStatus::Denied => {
                let response = request.response.clone().ok_or_else(|| {
                    store_error(anyhow::anyhow!("resolved request missing response"))
                })?;
                if response.decision == decision {
                    let event = query_permission_decision_event(&transaction, task_id, request_id)?
                        .ok_or_else(|| {
                            store_error(anyhow::anyhow!("resolved request missing decision event"))
                        })?;
                    return Ok(PermissionResolution {
                        task_id: task_id.to_owned(),
                        request_id: request_id.to_owned(),
                        status: PermissionRequestStatus::from_decision(decision),
                        decision,
                        event,
                        duplicated: true,
                    });
                }
                return Err(TaskStoreError::PermissionRequestAlreadyResolved);
            }
        }

        let response = PermissionDecisionEvent {
            request_id: request_id.to_owned(),
            decision,
            reason,
        };
        let sequence = next_sequence_tx(&transaction, task_id)?;
        insert_event_tx(
            &transaction,
            task_id,
            sequence,
            TaskEventType::ProviderPermissionDecided,
            serde_json::to_value(&response).map_err(store_error)?,
            &now,
        )?;
        let status = PermissionRequestStatus::from_decision(decision);
        transaction
            .execute(
                "UPDATE task_permission_requests
                 SET status = ?1,
                     response_payload_json = ?2,
                     responded_at = ?3
                 WHERE request_id = ?4 AND task_id = ?5",
                params![
                    serialize_json(&status)?,
                    serialize_json(&response)?,
                    now,
                    request_id,
                    task_id,
                ],
            )
            .map_err(store_error)?;
        transaction.commit().map_err(store_error)?;
        let connection = self.connection()?;
        let event = self.event_by_sequence(&connection, task_id, sequence)?;
        self.publish_if_configured(event.clone());
        Ok(PermissionResolution {
            task_id: task_id.to_owned(),
            request_id: request_id.to_owned(),
            status,
            decision,
            event,
            duplicated: false,
        })
    }

    pub fn acquire_lock(&self, request: &LockRequest) -> Result<bool, TaskStoreError> {
        let active = self.active_locks(&request.directory_id)?;
        let conflict = active
            .iter()
            .any(|lock| request.mode == LockMode::Exclusive || lock.mode == LockMode::Exclusive);
        if conflict {
            return Ok(false);
        }
        let now = now_rfc3339()?;
        let connection = self.connection()?;
        connection
            .execute(
                "INSERT OR REPLACE INTO directory_locks (
                    directory_id,
                    task_id,
                    mode,
                    status,
                    created_at,
                    released_at
                ) VALUES (?1, ?2, ?3, 'held', ?4, NULL)",
                params![
                    request.directory_id,
                    request.task_id,
                    serialize_json(&request.mode)?,
                    now,
                ],
            )
            .map_err(store_error)?;
        Ok(true)
    }

    pub fn active_locks(&self, directory_id: &str) -> Result<Vec<DirectoryLock>, TaskStoreError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT * FROM directory_locks WHERE directory_id = ?1 AND status = 'held'")
            .map_err(store_error)?;
        statement
            .query_map(params![directory_id], row_to_lock)
            .map_err(store_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(store_error)?
            .into_iter()
            .collect()
    }

    pub fn release_locks(&self, task_id: &str) -> Result<(), TaskStoreError> {
        let now = now_rfc3339()?;
        let connection = self.connection()?;
        release_locks_tx(&connection, task_id, &now)
    }

    fn all(&self) -> Result<Vec<Task>, TaskStoreError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT * FROM tasks ORDER BY rowid ASC")
            .map_err(store_error)?;
        statement
            .query_map([], row_to_task)
            .map_err(store_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(store_error)?
            .into_iter()
            .collect()
    }

    fn next_sequence(&self, task_id: &str) -> Result<i64, TaskStoreError> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT COALESCE(MAX(sequence), 0) + 1 FROM task_events WHERE task_id = ?1",
                params![task_id],
                |row| row.get(0),
            )
            .map_err(store_error)
    }

    fn connection(&self) -> Result<Connection, TaskStoreError> {
        sqlite::open_connection(&self.sqlite_path).map_err(store_error)
    }

    fn publish_if_configured(&self, event: TaskEvent) {
        if let Some(event_bus) = &self.event_bus {
            event_bus.publish(event);
        }
    }

    fn event_by_sequence(
        &self,
        connection: &Connection,
        task_id: &str,
        sequence: i64,
    ) -> Result<TaskEvent, TaskStoreError> {
        connection
            .query_row(
                "SELECT * FROM task_events WHERE task_id = ?1 AND sequence = ?2",
                params![task_id, sequence],
                row_to_event,
            )
            .map_err(store_error)?
    }
}

impl From<StoreConfig> for TaskStore {
    fn from(config: StoreConfig) -> Self {
        Self::configured(config)
    }
}

fn query_one(connection: &Connection, id: &str) -> Result<Option<Task>, TaskStoreError> {
    connection
        .query_row(
            "SELECT * FROM tasks WHERE id = ?1",
            params![id],
            row_to_task,
        )
        .optional()
        .map_err(store_error)?
        .transpose()
}

fn query_permission_request(
    connection: &Connection,
    task_id: &str,
    request_id: &str,
) -> Result<Option<PermissionRequestRecord>, TaskStoreError> {
    connection
        .query_row(
            "SELECT * FROM task_permission_requests WHERE task_id = ?1 AND request_id = ?2",
            params![task_id, request_id],
            row_to_permission_request,
        )
        .optional()
        .map_err(store_error)?
        .transpose()
}

fn query_permission_requests(
    connection: &Connection,
    sql: &str,
    params: &[&dyn ToSql],
) -> Result<Vec<PermissionRequestRecord>, TaskStoreError> {
    let mut statement = connection.prepare(sql).map_err(store_error)?;
    statement
        .query_map(params, row_to_permission_request)
        .map_err(store_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(store_error)?
        .into_iter()
        .collect()
}

fn query_permission_decision_event(
    connection: &Connection,
    task_id: &str,
    request_id: &str,
) -> Result<Option<TaskEvent>, TaskStoreError> {
    connection
        .query_row(
            "SELECT * FROM task_events
             WHERE task_id = ?1
               AND event_type = ?2
               AND json_extract(payload_json, '$.request_id') = ?3
             ORDER BY sequence ASC
             LIMIT 1",
            params![
                task_id,
                serialize_json(&TaskEventType::ProviderPermissionDecided)?,
                request_id
            ],
            row_to_event,
        )
        .optional()
        .map_err(store_error)?
        .transpose()
}

fn insert_event_tx(
    connection: &Connection,
    task_id: &str,
    sequence: i64,
    event_type: TaskEventType,
    payload: Value,
    now: &str,
) -> Result<(), TaskStoreError> {
    let id = format!("evt_{task_id}_{sequence}");
    connection
        .execute(
            "INSERT INTO task_events (
                id,
                task_id,
                sequence,
                event_type,
                payload_json,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                task_id,
                sequence,
                serialize_json(&event_type)?,
                serialize_json(&payload)?,
                now,
            ],
        )
        .map_err(store_error)?;
    Ok(())
}

fn release_locks_tx(
    connection: &Connection,
    task_id: &str,
    now: &str,
) -> Result<(), TaskStoreError> {
    connection
        .execute(
            "UPDATE directory_locks
             SET status = 'released', released_at = ?1
             WHERE task_id = ?2 AND status = 'held'",
            params![now, task_id],
        )
        .map_err(store_error)?;
    Ok(())
}

fn next_sequence_tx(connection: &Connection, task_id: &str) -> Result<i64, TaskStoreError> {
    connection
        .query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM task_events WHERE task_id = ?1",
            params![task_id],
            |row| row.get(0),
        )
        .map_err(store_error)
}

fn event_type_for_status(status: TaskStatus) -> TaskEventType {
    match status {
        TaskStatus::Queued => TaskEventType::Queued,
        TaskStatus::WaitingDirectoryLock => TaskEventType::WaitingDirectoryLock,
        TaskStatus::Preparing => TaskEventType::Preparing,
        TaskStatus::Running => TaskEventType::Running,
        TaskStatus::Completed => TaskEventType::Completed,
        TaskStatus::Failed => TaskEventType::Failed,
        TaskStatus::Cancelled => TaskEventType::Cancelled,
        TaskStatus::TimedOut => TaskEventType::TimedOut,
    }
}

fn timestamp_if(condition: bool, value: &str) -> Option<&str> {
    condition.then_some(value)
}

fn now_rfc3339() -> Result<String, TaskStoreError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(store_error)
}

fn store_error(error: impl Into<anyhow::Error>) -> TaskStoreError {
    TaskStoreError::Store(error.into())
}
