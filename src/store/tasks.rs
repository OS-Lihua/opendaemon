use std::{path::PathBuf, sync::Arc};

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json::Value;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    config::StoreConfig,
    scheduler::locks::{DirectoryLock, LockMode, LockRequest},
    security::directory::WorkspaceMode,
    task::{
        event::{TaskEvent, TaskEventType},
        model::{CreateTask, Task, TaskModelError, TaskStatus},
        result::TaskResult,
        state::{TaskStateError, TaskTransition, validate_transition},
    },
};

use super::sqlite;

#[derive(Debug, Clone)]
pub struct TaskStore {
    sqlite_path: Arc<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskFilters {
    pub owner_product_id: Option<String>,
    pub agent_id: Option<String>,
    pub directory_id: Option<String>,
    pub status: Option<TaskStatus>,
}

#[derive(Debug)]
pub enum TaskStoreError {
    NotFound,
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
        }
    }

    pub fn open(config: StoreConfig) -> Result<Self, TaskStoreError> {
        sqlite::open_connection(&config.sqlite_path).map_err(store_error)?;
        Ok(Self::configured(config))
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
        Ok(self
            .list_events(task_id)?
            .into_iter()
            .find(|event| event.sequence == sequence)
            .expect("inserted task event must be readable"))
    }

    pub fn list_events(&self, task_id: &str) -> Result<Vec<TaskEvent>, TaskStoreError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT * FROM task_events WHERE task_id = ?1 ORDER BY sequence ASC")
            .map_err(store_error)?;
        statement
            .query_map(params![task_id], row_to_event)
            .map_err(store_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(store_error)?
            .into_iter()
            .collect()
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

fn row_to_task(row: &Row<'_>) -> Result<Result<Task, TaskStoreError>, rusqlite::Error> {
    let required_capabilities_json: String = row.get("required_capabilities_json")?;
    let workspace_mode_json: String = row.get("workspace_mode")?;
    let status_json: String = row.get("status")?;
    let metadata_json: Option<String> = row.get("metadata_json")?;
    let result_json: Option<String> = row.get("result_json")?;

    Ok((|| {
        Ok(Task {
            id: row.get("id")?,
            owner_product_id: row.get("owner_product_id")?,
            agent_id: row.get("agent_id")?,
            directory_id: row.get("directory_id")?,
            prompt: row.get("prompt")?,
            required_capabilities: deserialize_json(&required_capabilities_json)?,
            workspace_mode: deserialize_json(&workspace_mode_json)?,
            direct_mode_task_opt_in: row.get("direct_mode_task_opt_in")?,
            metadata: deserialize_optional_json(metadata_json)?,
            provider_id: row.get("provider_id")?,
            model: row.get("model")?,
            permission_mode: row.get("permission_mode")?,
            timeout_seconds: row.get("timeout_seconds")?,
            status: deserialize_json(&status_json)?,
            result: deserialize_optional_json(result_json)?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
            started_at: row.get("started_at")?,
            completed_at: row.get("completed_at")?,
            cancelled_at: row.get("cancelled_at")?,
            failed_at: row.get("failed_at")?,
        })
    })())
}

fn row_to_event(row: &Row<'_>) -> Result<Result<TaskEvent, TaskStoreError>, rusqlite::Error> {
    let event_type_json: String = row.get("event_type")?;
    let payload_json: String = row.get("payload_json")?;
    Ok((|| {
        Ok(TaskEvent {
            id: row.get("id")?,
            task_id: row.get("task_id")?,
            sequence: row.get("sequence")?,
            event_type: deserialize_json(&event_type_json)?,
            payload: deserialize_json(&payload_json)?,
            created_at: row.get("created_at")?,
        })
    })())
}

fn row_to_lock(row: &Row<'_>) -> Result<Result<DirectoryLock, TaskStoreError>, rusqlite::Error> {
    let mode_json: String = row.get("mode")?;
    Ok((|| {
        Ok(DirectoryLock {
            directory_id: row.get("directory_id")?,
            task_id: row.get("task_id")?,
            mode: deserialize_json(&mode_json)?,
            status: row.get("status")?,
            created_at: row.get("created_at")?,
            released_at: row.get("released_at")?,
        })
    })())
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

fn serialize_json(value: &impl serde::Serialize) -> Result<String, TaskStoreError> {
    serde_json::to_string(value).map_err(store_error)
}

fn serialize_optional_json(value: &Option<Value>) -> Result<Option<String>, TaskStoreError> {
    value.as_ref().map(serialize_json).transpose()
}

fn deserialize_json<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, TaskStoreError> {
    serde_json::from_str(value).map_err(store_error)
}

fn deserialize_optional_json<T: serde::de::DeserializeOwned>(
    value: Option<String>,
) -> Result<Option<T>, TaskStoreError> {
    value.map(|value| deserialize_json(&value)).transpose()
}

fn store_error(error: impl Into<anyhow::Error>) -> TaskStoreError {
    TaskStoreError::Store(error.into())
}
