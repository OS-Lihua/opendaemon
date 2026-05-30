use std::{path::PathBuf, sync::Arc};

use rusqlite::{Connection, OptionalExtension, params};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    config::StoreConfig,
    security::{
        directory::{
            DirectoryAuthorizationRequest, DirectoryCapability, DirectoryGrant,
            DirectoryGrantPolicy, DirectoryLockPolicy, DirectorySecurityError, WorkspaceMode,
        },
        path_guard::{PathGuardError, canonicalize_grant_path},
    },
};

use super::sqlite;

#[derive(Debug, Clone)]
pub struct DirectoryGrantStore {
    sqlite_path: Arc<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirectoryGrantFilters {
    pub product_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateDirectoryGrant {
    pub product_id: String,
    pub agent_id: String,
    pub path: PathBuf,
    pub capabilities: Vec<DirectoryCapability>,
    pub workspace_modes: Option<Vec<WorkspaceMode>>,
    pub default_workspace_mode: Option<WorkspaceMode>,
    pub lock_policy: Option<DirectoryLockPolicy>,
    pub direct_mode_requires_explicit_task_opt_in: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct PatchDirectoryGrant {
    pub capabilities: Option<Vec<DirectoryCapability>>,
    pub workspace_modes: Option<Vec<WorkspaceMode>>,
    pub default_workspace_mode: Option<WorkspaceMode>,
    pub lock_policy: Option<DirectoryLockPolicy>,
    pub direct_mode_requires_explicit_task_opt_in: Option<bool>,
}

#[derive(Debug)]
pub enum DirectoryStoreError {
    NotFound,
    Path(PathGuardError),
    Security(DirectorySecurityError),
    Store(anyhow::Error),
}

impl DirectoryGrantStore {
    #[must_use]
    pub fn configured(config: StoreConfig) -> Self {
        Self {
            sqlite_path: Arc::new(config.sqlite_path),
        }
    }

    pub fn open(config: StoreConfig) -> Result<Self, DirectoryStoreError> {
        sqlite::open_connection(&config.sqlite_path).map_err(store_error)?;
        Ok(Self::configured(config))
    }

    pub fn create(
        &self,
        input: CreateDirectoryGrant,
    ) -> Result<DirectoryGrant, DirectoryStoreError> {
        let canonical_path =
            canonicalize_grant_path(&input.path).map_err(DirectoryStoreError::Path)?;
        let policy = policy_from_create(&input, is_git_repository(&canonical_path))?;
        let now = now_rfc3339()?;
        let path = canonical_path.to_string_lossy().into_owned();
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(store_error)?;

        transaction
            .execute(
                "INSERT INTO directory_grants (
                    id,
                    product_id,
                    agent_id,
                    path,
                    capabilities_json,
                    workspace_modes_json,
                    default_workspace_mode,
                    lock_policy,
                    direct_mode_requires_explicit_task_opt_in,
                    created_at,
                    updated_at
                ) VALUES ('', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    input.product_id,
                    input.agent_id,
                    path,
                    serialize_json(&policy.capabilities)?,
                    serialize_json(&policy.workspace_modes)?,
                    serialize_json(&policy.default_workspace_mode)?,
                    serialize_json(&policy.lock_policy)?,
                    policy.direct_mode_requires_explicit_task_opt_in,
                    now,
                    now,
                ],
            )
            .map_err(store_error)?;
        let row_id = transaction.last_insert_rowid();
        let id = format!("dir_{row_id}");
        transaction
            .execute(
                "UPDATE directory_grants SET id = ?1 WHERE rowid = ?2",
                params![id, row_id],
            )
            .map_err(store_error)?;
        transaction.commit().map_err(store_error)?;

        self.get(&id)
    }

    pub fn list(
        &self,
        filters: DirectoryGrantFilters,
    ) -> Result<Vec<DirectoryGrant>, DirectoryStoreError> {
        let connection = self.connection()?;
        match (filters.product_id, filters.agent_id) {
            (Some(product_id), Some(agent_id)) => query_grants(
                &connection,
                "SELECT * FROM directory_grants
                 WHERE product_id = ?1 AND agent_id = ?2
                 ORDER BY rowid ASC",
                params![product_id, agent_id],
            ),
            (Some(product_id), None) => query_grants(
                &connection,
                "SELECT * FROM directory_grants
                 WHERE product_id = ?1
                 ORDER BY rowid ASC",
                params![product_id],
            ),
            (None, Some(agent_id)) => query_grants(
                &connection,
                "SELECT * FROM directory_grants
                 WHERE agent_id = ?1
                 ORDER BY rowid ASC",
                params![agent_id],
            ),
            (None, None) => query_grants(
                &connection,
                "SELECT * FROM directory_grants ORDER BY rowid ASC",
                [],
            ),
        }
    }

    pub fn get(&self, id: &str) -> Result<DirectoryGrant, DirectoryStoreError> {
        let connection = self.connection()?;
        query_one(&connection, id)?.ok_or(DirectoryStoreError::NotFound)
    }

    pub fn patch(
        &self,
        id: &str,
        patch: PatchDirectoryGrant,
    ) -> Result<DirectoryGrant, DirectoryStoreError> {
        if patch.capabilities.is_none()
            && patch.workspace_modes.is_none()
            && patch.default_workspace_mode.is_none()
            && patch.lock_policy.is_none()
            && patch.direct_mode_requires_explicit_task_opt_in.is_none()
        {
            return Err(DirectoryStoreError::Security(
                DirectorySecurityError::AuthorizationFailed,
            ));
        }

        let current = self.get(id)?;
        let policy = current.policy();
        let policy = DirectoryGrantPolicy::new(
            patch.capabilities.unwrap_or(policy.capabilities),
            patch.workspace_modes.unwrap_or(policy.workspace_modes),
            patch
                .default_workspace_mode
                .unwrap_or(policy.default_workspace_mode),
            patch.lock_policy.unwrap_or(policy.lock_policy),
            patch
                .direct_mode_requires_explicit_task_opt_in
                .unwrap_or(policy.direct_mode_requires_explicit_task_opt_in),
        )
        .map_err(DirectoryStoreError::Security)?;
        let now = now_rfc3339()?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(store_error)?;

        transaction
            .execute(
                "UPDATE directory_grants
                 SET capabilities_json = ?1,
                     workspace_modes_json = ?2,
                     default_workspace_mode = ?3,
                     lock_policy = ?4,
                     direct_mode_requires_explicit_task_opt_in = ?5,
                     updated_at = ?6
                 WHERE id = ?7",
                params![
                    serialize_json(&policy.capabilities)?,
                    serialize_json(&policy.workspace_modes)?,
                    serialize_json(&policy.default_workspace_mode)?,
                    serialize_json(&policy.lock_policy)?,
                    policy.direct_mode_requires_explicit_task_opt_in,
                    now,
                    id,
                ],
            )
            .map_err(store_error)?;
        transaction.commit().map_err(store_error)?;

        self.get(id)
    }

    pub fn delete(&self, id: &str) -> Result<(), DirectoryStoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(store_error)?;
        let changed = transaction
            .execute("DELETE FROM directory_grants WHERE id = ?1", params![id])
            .map_err(store_error)?;
        transaction.commit().map_err(store_error)?;
        if changed == 0 {
            return Err(DirectoryStoreError::NotFound);
        }
        Ok(())
    }

    pub fn authorize(
        &self,
        request: &DirectoryAuthorizationRequest,
    ) -> Result<DirectoryGrant, DirectoryStoreError> {
        let grant = self.get(&request.directory_id)?;
        grant
            .authorize(request)
            .map_err(DirectoryStoreError::Security)?;
        Ok(grant)
    }

    fn connection(&self) -> Result<Connection, DirectoryStoreError> {
        sqlite::open_connection(&self.sqlite_path).map_err(store_error)
    }
}

impl From<StoreConfig> for DirectoryGrantStore {
    fn from(config: StoreConfig) -> Self {
        Self::configured(config)
    }
}

fn policy_from_create(
    input: &CreateDirectoryGrant,
    is_git_repository: bool,
) -> Result<DirectoryGrantPolicy, DirectoryStoreError> {
    let workspace_modes = match input.workspace_modes.clone() {
        Some(modes) => modes,
        None if is_git_repository => vec![WorkspaceMode::Worktree],
        None => {
            return Err(DirectoryStoreError::Security(
                DirectorySecurityError::InvalidWorkspaceMode,
            ));
        }
    };
    let default_workspace_mode = input.default_workspace_mode.unwrap_or_else(|| {
        if workspace_modes.contains(&WorkspaceMode::Worktree) {
            WorkspaceMode::Worktree
        } else {
            workspace_modes[0]
        }
    });
    let lock_policy = input.lock_policy.unwrap_or_else(|| {
        if input.capabilities.contains(&DirectoryCapability::Write) {
            DirectoryLockPolicy::Exclusive
        } else {
            DirectoryLockPolicy::Shared
        }
    });

    if workspace_modes == [WorkspaceMode::Worktree] && !is_git_repository {
        return Err(DirectoryStoreError::Security(
            DirectorySecurityError::InvalidWorkspaceMode,
        ));
    }

    DirectoryGrantPolicy::new(
        input.capabilities.clone(),
        workspace_modes,
        default_workspace_mode,
        lock_policy,
        input
            .direct_mode_requires_explicit_task_opt_in
            .unwrap_or(true),
    )
    .map_err(DirectoryStoreError::Security)
}

fn is_git_repository(path: &std::path::Path) -> bool {
    path.join(".git").exists()
}

fn query_one(
    connection: &Connection,
    id: &str,
) -> Result<Option<DirectoryGrant>, DirectoryStoreError> {
    connection
        .query_row(
            "SELECT * FROM directory_grants WHERE id = ?1",
            params![id],
            row_to_grant,
        )
        .optional()
        .map_err(store_error)
}

fn query_grants<P>(
    connection: &Connection,
    sql: &str,
    params: P,
) -> Result<Vec<DirectoryGrant>, DirectoryStoreError>
where
    P: rusqlite::Params,
{
    let mut statement = connection.prepare(sql).map_err(store_error)?;
    let rows = statement
        .query_map(params, row_to_grant)
        .map_err(store_error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(store_error)
}

fn row_to_grant(row: &rusqlite::Row<'_>) -> rusqlite::Result<DirectoryGrant> {
    let capabilities_json: String = row.get("capabilities_json")?;
    let workspace_modes_json: String = row.get("workspace_modes_json")?;
    let default_workspace_mode_json: String = row.get("default_workspace_mode")?;
    let lock_policy_json: String = row.get("lock_policy")?;

    Ok(DirectoryGrant {
        id: row.get("id")?,
        product_id: row.get("product_id")?,
        agent_id: row.get("agent_id")?,
        path: row.get("path")?,
        capabilities: deserialize_json(&capabilities_json)?,
        workspace_modes: deserialize_json(&workspace_modes_json)?,
        default_workspace_mode: deserialize_json(&default_workspace_mode_json)?,
        lock_policy: deserialize_json(&lock_policy_json)?,
        direct_mode_requires_explicit_task_opt_in: row
            .get("direct_mode_requires_explicit_task_opt_in")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn serialize_json<T: serde::Serialize>(value: &T) -> Result<String, DirectoryStoreError> {
    serde_json::to_string(value).map_err(|error| DirectoryStoreError::Store(error.into()))
}

fn deserialize_json<T: serde::de::DeserializeOwned>(json: &str) -> rusqlite::Result<T> {
    serde_json::from_str(json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn now_rfc3339() -> Result<String, DirectoryStoreError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| DirectoryStoreError::Store(error.into()))
}

fn store_error(error: rusqlite::Error) -> DirectoryStoreError {
    DirectoryStoreError::Store(error.into())
}
