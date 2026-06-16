use rusqlite::Row;
use serde_json::Value;

use crate::{
    scheduler::locks::DirectoryLock,
    task::{
        event::{TaskEvent, TaskEventType},
        model::Task,
        permission::PermissionRequestRecord,
        result::TaskResult,
    },
};

use super::{TaskStoreError, store_error};

pub(super) fn row_to_task(row: &Row<'_>) -> Result<Result<Task, TaskStoreError>, rusqlite::Error> {
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
            result: deserialize_optional_json::<TaskResult>(result_json)?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
            started_at: row.get("started_at")?,
            completed_at: row.get("completed_at")?,
            cancelled_at: row.get("cancelled_at")?,
            failed_at: row.get("failed_at")?,
        })
    })())
}

pub(super) fn row_to_event(
    row: &Row<'_>,
) -> Result<Result<TaskEvent, TaskStoreError>, rusqlite::Error> {
    let event_type_json: String = row.get("event_type")?;
    let payload_json: String = row.get("payload_json")?;
    Ok((|| {
        Ok(TaskEvent {
            id: row.get("id")?,
            task_id: row.get("task_id")?,
            sequence: row.get("sequence")?,
            event_type: deserialize_json::<TaskEventType>(&event_type_json)?,
            payload: deserialize_json(&payload_json)?,
            created_at: row.get("created_at")?,
        })
    })())
}

pub(super) fn row_to_lock(
    row: &Row<'_>,
) -> Result<Result<DirectoryLock, TaskStoreError>, rusqlite::Error> {
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

pub(super) fn row_to_permission_request(
    row: &Row<'_>,
) -> Result<Result<PermissionRequestRecord, TaskStoreError>, rusqlite::Error> {
    let status_json: String = row.get("status")?;
    let request_payload_json: String = row.get("request_payload_json")?;
    let response_payload_json: Option<String> = row.get("response_payload_json")?;
    Ok((|| {
        Ok(PermissionRequestRecord {
            request_id: row.get("request_id")?,
            task_id: row.get("task_id")?,
            sequence: row.get("sequence")?,
            provider_id: row.get("provider_id")?,
            permission_kind: row.get("permission_kind")?,
            status: deserialize_json(&status_json)?,
            request: deserialize_json(&request_payload_json)?,
            response: deserialize_optional_json(response_payload_json)?,
            requested_at: row.get("requested_at")?,
            responded_at: row.get("responded_at")?,
        })
    })())
}

pub(super) fn serialize_json(value: &impl serde::Serialize) -> Result<String, TaskStoreError> {
    serde_json::to_string(value).map_err(store_error)
}

pub(super) fn serialize_optional_json(
    value: &Option<Value>,
) -> Result<Option<String>, TaskStoreError> {
    value.as_ref().map(serialize_json).transpose()
}

pub(super) fn deserialize_json<T: serde::de::DeserializeOwned>(
    value: &str,
) -> Result<T, TaskStoreError> {
    serde_json::from_str(value).map_err(store_error)
}

fn deserialize_optional_json<T: serde::de::DeserializeOwned>(
    value: Option<String>,
) -> Result<Option<T>, TaskStoreError> {
    value.map(|value| deserialize_json(&value)).transpose()
}
