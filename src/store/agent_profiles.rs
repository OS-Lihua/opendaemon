use std::{path::PathBuf, sync::Arc};

use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::{
    agent::profile::{
        AgentAuthorizationRequest, AgentProfile, AgentProfileError, CreateAgentProfile,
        ExecutionPolicy, ProviderConfig, WorkspaceMode, now_rfc3339,
    },
    config::StoreConfig,
};

use super::sqlite;

#[derive(Debug, Clone)]
pub struct AgentProfileStore {
    sqlite_path: Arc<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentProfileFilters {
    pub owner_product_id: Option<String>,
    pub provider_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PatchAgentProfile {
    pub name: Option<String>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub instructions: Option<Option<String>>,
    pub execution_policy: Option<ExecutionPolicy>,
    pub provider_config: Option<ProviderConfig>,
}

#[derive(Debug)]
pub enum AgentStoreError {
    Profile(AgentProfileError),
    Store(anyhow::Error),
}

impl AgentProfileStore {
    #[must_use]
    pub fn configured(config: StoreConfig) -> Self {
        Self {
            sqlite_path: Arc::new(config.sqlite_path),
        }
    }

    pub fn open(config: StoreConfig) -> Result<Self, AgentStoreError> {
        sqlite::open_connection(&config.sqlite_path).map_err(store_error)?;
        Ok(Self::configured(config))
    }

    pub fn create(&self, input: CreateAgentProfile) -> Result<AgentProfile, AgentStoreError> {
        input.validate().map_err(AgentStoreError::Profile)?;
        let now = now_rfc3339().map_err(AgentStoreError::Profile)?;
        let profile = input.into_profile(now.clone(), now);
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(store_error)?;
        transaction
            .execute(
                "INSERT INTO agent_profiles (
                    id,
                    name,
                    owner_product_id,
                    provider_id,
                    model,
                    instructions,
                    execution_policy_json,
                    provider_config_json,
                    created_at,
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    profile.id,
                    profile.name,
                    profile.owner_product_id,
                    profile.provider_id,
                    profile.model,
                    profile.instructions,
                    serialize_json(&profile.execution_policy)?,
                    serialize_json(&profile.provider_config)?,
                    profile.created_at,
                    profile.updated_at,
                ],
            )
            .map_err(map_sqlite_write_error)?;
        transaction.commit().map_err(store_error)?;

        self.get(&profile.id)
    }

    pub fn list(&self, filters: AgentProfileFilters) -> Result<Vec<AgentProfile>, AgentStoreError> {
        let connection = self.connection()?;
        match (filters.owner_product_id, filters.provider_id) {
            (Some(owner_product_id), Some(provider_id)) => query_profiles(
                &connection,
                "SELECT * FROM agent_profiles
                 WHERE owner_product_id = ?1 AND provider_id = ?2
                 ORDER BY rowid ASC",
                params![owner_product_id, provider_id],
            ),
            (Some(owner_product_id), None) => query_profiles(
                &connection,
                "SELECT * FROM agent_profiles
                 WHERE owner_product_id = ?1
                 ORDER BY rowid ASC",
                params![owner_product_id],
            ),
            (None, Some(provider_id)) => query_profiles(
                &connection,
                "SELECT * FROM agent_profiles
                 WHERE provider_id = ?1
                 ORDER BY rowid ASC",
                params![provider_id],
            ),
            (None, None) => query_profiles(
                &connection,
                "SELECT * FROM agent_profiles ORDER BY rowid ASC",
                [],
            ),
        }
    }

    pub fn get(&self, id: &str) -> Result<AgentProfile, AgentStoreError> {
        let connection = self.connection()?;
        query_one(&connection, id)?
            .ok_or(AgentStoreError::Profile(AgentProfileError::AgentNotFound))
    }

    pub fn patch(
        &self,
        id: &str,
        patch: PatchAgentProfile,
    ) -> Result<AgentProfile, AgentStoreError> {
        if patch.name.is_none()
            && patch.provider_id.is_none()
            && patch.model.is_none()
            && patch.instructions.is_none()
            && patch.execution_policy.is_none()
            && patch.provider_config.is_none()
        {
            return Err(AgentStoreError::Profile(
                AgentProfileError::InvalidAgentProfile,
            ));
        }

        let current = self.get(id)?;
        let next = CreateAgentProfile {
            id: current.id.clone(),
            name: patch.name.unwrap_or(current.name),
            owner_product_id: current.owner_product_id,
            provider_id: patch.provider_id.unwrap_or(current.provider_id),
            model: patch.model.unwrap_or(current.model),
            instructions: patch.instructions.unwrap_or(current.instructions),
            execution_policy: patch.execution_policy.unwrap_or(current.execution_policy),
            provider_config: patch.provider_config.unwrap_or(current.provider_config),
        };
        next.validate().map_err(AgentStoreError::Profile)?;
        let updated_at = now_rfc3339().map_err(AgentStoreError::Profile)?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(store_error)?;
        transaction
            .execute(
                "UPDATE agent_profiles
                 SET name = ?1,
                     provider_id = ?2,
                     model = ?3,
                     instructions = ?4,
                     execution_policy_json = ?5,
                     provider_config_json = ?6,
                     updated_at = ?7
                 WHERE id = ?8",
                params![
                    next.name,
                    next.provider_id,
                    next.model,
                    next.instructions,
                    serialize_json(&next.execution_policy)?,
                    serialize_json(&next.provider_config)?,
                    updated_at,
                    id,
                ],
            )
            .map_err(store_error)?;
        transaction.commit().map_err(store_error)?;

        self.get(id)
    }

    pub fn delete(&self, id: &str) -> Result<(), AgentStoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(store_error)?;
        let changed = transaction
            .execute("DELETE FROM agent_profiles WHERE id = ?1", params![id])
            .map_err(store_error)?;
        transaction.commit().map_err(store_error)?;
        if changed == 0 {
            return Err(AgentStoreError::Profile(AgentProfileError::AgentNotFound));
        }
        Ok(())
    }

    pub fn authorize(
        &self,
        request: &AgentAuthorizationRequest,
    ) -> Result<AgentProfile, AgentStoreError> {
        let profile = self.get(&request.agent_id)?;
        if profile.owner_product_id != request.owner_product_id {
            return Err(AgentStoreError::Profile(
                AgentProfileError::AgentAuthorizationFailed,
            ));
        }
        if request
            .provider_id_override
            .as_ref()
            .is_some_and(|provider_id| provider_id != &profile.provider_id)
            || request
                .model_override
                .as_ref()
                .is_some_and(|model| model != &profile.model)
            || request
                .permission_mode_override
                .as_ref()
                .is_some_and(|permission_mode| {
                    permission_mode != &profile.provider_config.permission_mode
                })
        {
            return Err(AgentStoreError::Profile(
                AgentProfileError::AgentAuthorizationFailed,
            ));
        }
        if request.requested_workspace_mode == WorkspaceMode::Direct
            && !profile.execution_policy.allow_direct_directory
        {
            return Err(AgentStoreError::Profile(
                AgentProfileError::AgentAuthorizationFailed,
            ));
        }
        Ok(profile)
    }

    fn connection(&self) -> Result<Connection, AgentStoreError> {
        sqlite::open_connection(&self.sqlite_path).map_err(store_error)
    }
}

impl From<StoreConfig> for AgentProfileStore {
    fn from(config: StoreConfig) -> Self {
        Self::configured(config)
    }
}

fn query_one(connection: &Connection, id: &str) -> Result<Option<AgentProfile>, AgentStoreError> {
    connection
        .query_row(
            "SELECT * FROM agent_profiles WHERE id = ?1",
            params![id],
            row_to_profile,
        )
        .optional()
        .map_err(store_error)?
        .transpose()
}

fn query_profiles<P>(
    connection: &Connection,
    sql: &str,
    params: P,
) -> Result<Vec<AgentProfile>, AgentStoreError>
where
    P: rusqlite::Params,
{
    let mut statement = connection.prepare(sql).map_err(store_error)?;
    let profiles = statement
        .query_map(params, row_to_profile)
        .map_err(store_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(store_error)?;
    profiles.into_iter().collect()
}

fn row_to_profile(row: &Row<'_>) -> Result<Result<AgentProfile, AgentStoreError>, rusqlite::Error> {
    let execution_policy_json: String = row.get("execution_policy_json")?;
    let provider_config_json: String = row.get("provider_config_json")?;
    let execution_policy: Result<ExecutionPolicy, _> = serde_json::from_str(&execution_policy_json);
    let provider_config: Result<ProviderConfig, _> = serde_json::from_str(&provider_config_json);

    Ok(match (execution_policy, provider_config) {
        (Ok(execution_policy), Ok(provider_config)) => Ok(AgentProfile {
            id: row.get("id")?,
            name: row.get("name")?,
            owner_product_id: row.get("owner_product_id")?,
            provider_id: row.get("provider_id")?,
            model: row.get("model")?,
            instructions: row.get("instructions")?,
            execution_policy,
            provider_config,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        }),
        (Err(error), _) | (_, Err(error)) => Err(AgentStoreError::Store(error.into())),
    })
}

fn serialize_json(value: &impl serde::Serialize) -> Result<String, AgentStoreError> {
    serde_json::to_string(value).map_err(|error| AgentStoreError::Store(error.into()))
}

fn map_sqlite_write_error(error: rusqlite::Error) -> AgentStoreError {
    match error {
        rusqlite::Error::SqliteFailure(ref failure, _)
            if failure.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            AgentStoreError::Profile(AgentProfileError::InvalidAgentProfile)
        }
        _ => store_error(error),
    }
}

fn store_error(error: impl Into<anyhow::Error>) -> AgentStoreError {
    AgentStoreError::Store(error.into())
}
