use std::{path::PathBuf, sync::Arc};

use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    config::StoreConfig,
    control_plane::model::{DaemonConnectionStatus, DaemonRegistrationRecord},
};

use super::sqlite;

#[derive(Debug, Clone)]
pub struct DaemonStateStore {
    sqlite_path: Arc<PathBuf>,
}

#[derive(Debug)]
pub enum DaemonStateStoreError {
    NotFound,
    Store(anyhow::Error),
}

impl DaemonStateStore {
    #[must_use]
    pub fn configured(config: StoreConfig) -> Self {
        Self {
            sqlite_path: Arc::new(config.sqlite_path),
        }
    }

    pub fn open(config: StoreConfig) -> Result<Self, DaemonStateStoreError> {
        sqlite::open_connection(&config.sqlite_path).map_err(store_error)?;
        Ok(Self::configured(config))
    }

    pub fn save_registration(
        &self,
        record: DaemonRegistrationRecord,
    ) -> Result<DaemonRegistrationRecord, DaemonStateStoreError> {
        let connection = self.connection()?;
        connection
            .execute(
                "INSERT INTO daemon_state (
                    daemon_id,
                    control_plane_url,
                    daemon_token,
                    status,
                    registered_at,
                    last_heartbeat_at,
                    last_error_code,
                    session_id
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(daemon_id) DO UPDATE SET
                    control_plane_url = excluded.control_plane_url,
                    daemon_token = excluded.daemon_token,
                    status = excluded.status,
                    registered_at = excluded.registered_at,
                    last_heartbeat_at = excluded.last_heartbeat_at,
                    last_error_code = excluded.last_error_code,
                    session_id = excluded.session_id",
                params![
                    record.daemon_id,
                    record.control_plane_url,
                    record.daemon_token,
                    serialize_status(record.status),
                    record.registered_at,
                    record.last_heartbeat_at,
                    record.last_error_code,
                    record.session_id,
                ],
            )
            .map_err(store_error)?;
        self.get(&record.daemon_id)
    }

    pub fn get(&self, daemon_id: &str) -> Result<DaemonRegistrationRecord, DaemonStateStoreError> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT * FROM daemon_state WHERE daemon_id = ?1",
                params![daemon_id],
                row_to_record,
            )
            .optional()
            .map_err(store_error)?
            .ok_or(DaemonStateStoreError::NotFound)
    }

    pub fn get_current(&self) -> Result<DaemonRegistrationRecord, DaemonStateStoreError> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT * FROM daemon_state ORDER BY rowid ASC LIMIT 1",
                [],
                row_to_record,
            )
            .optional()
            .map_err(store_error)?
            .ok_or(DaemonStateStoreError::NotFound)
    }

    pub fn mark_heartbeat(
        &self,
        daemon_id: &str,
        heartbeat_at: &str,
        status: DaemonConnectionStatus,
    ) -> Result<DaemonRegistrationRecord, DaemonStateStoreError> {
        let connection = self.connection()?;
        connection
            .execute(
                "UPDATE daemon_state
                 SET last_heartbeat_at = ?1, status = ?2
                 WHERE daemon_id = ?3",
                params![heartbeat_at, serialize_status(status), daemon_id],
            )
            .map_err(store_error)?;
        self.get(daemon_id)
    }

    fn connection(&self) -> Result<Connection, DaemonStateStoreError> {
        sqlite::open_connection(&self.sqlite_path).map_err(store_error)
    }
}

fn serialize_status(status: DaemonConnectionStatus) -> &'static str {
    match status {
        DaemonConnectionStatus::Online => "online",
        DaemonConnectionStatus::Offline => "offline",
        DaemonConnectionStatus::Connecting => "connecting",
        DaemonConnectionStatus::Error => "error",
    }
}

fn deserialize_status(status: &str) -> Result<DaemonConnectionStatus, DaemonStateStoreError> {
    match status {
        "online" => Ok(DaemonConnectionStatus::Online),
        "offline" => Ok(DaemonConnectionStatus::Offline),
        "connecting" => Ok(DaemonConnectionStatus::Connecting),
        "error" => Ok(DaemonConnectionStatus::Error),
        _ => Err(DaemonStateStoreError::Store(anyhow::anyhow!(
            "invalid daemon state status"
        ))),
    }
}

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<DaemonRegistrationRecord> {
    let status: String = row.get("status")?;
    let status = deserialize_status(&status).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("{error:?}"),
            )),
        )
    })?;
    Ok(DaemonRegistrationRecord {
        daemon_id: row.get("daemon_id")?,
        control_plane_url: row.get("control_plane_url")?,
        daemon_token: row.get("daemon_token")?,
        status,
        registered_at: row.get("registered_at")?,
        last_heartbeat_at: row.get("last_heartbeat_at")?,
        last_error_code: row.get("last_error_code")?,
        session_id: row.get("session_id")?,
    })
}

fn store_error(error: rusqlite::Error) -> DaemonStateStoreError {
    DaemonStateStoreError::Store(error.into())
}
