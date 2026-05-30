use std::path::Path;

use rusqlite::Connection;

pub fn open_connection(path: &Path) -> Result<Connection, rusqlite::Error> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|_| rusqlite::Error::InvalidPath(path.into()))?;
    }

    let connection = Connection::open(path)?;
    initialize_schema(&connection)?;
    Ok(connection)
}

fn initialize_schema(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS directory_grants (
            id TEXT PRIMARY KEY,
            product_id TEXT NOT NULL,
            agent_id TEXT NOT NULL,
            path TEXT NOT NULL,
            capabilities_json TEXT NOT NULL,
            workspace_modes_json TEXT NOT NULL,
            default_workspace_mode TEXT NOT NULL,
            lock_policy TEXT NOT NULL,
            direct_mode_requires_explicit_task_opt_in INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS directory_grants_product_agent_idx
        ON directory_grants(product_id, agent_id);

        CREATE TABLE IF NOT EXISTS agent_profiles (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            owner_product_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            model TEXT NOT NULL,
            instructions TEXT,
            execution_policy_json TEXT NOT NULL,
            provider_config_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS agent_profiles_owner_product_idx
        ON agent_profiles(owner_product_id);

        CREATE INDEX IF NOT EXISTS agent_profiles_provider_idx
        ON agent_profiles(provider_id);
        "#,
    )
}
