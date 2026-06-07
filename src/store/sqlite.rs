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
            allow_remote_execution INTEGER NOT NULL DEFAULT 0,
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

        CREATE TABLE IF NOT EXISTS tasks (
            id TEXT PRIMARY KEY,
            owner_product_id TEXT NOT NULL,
            agent_id TEXT NOT NULL,
            directory_id TEXT NOT NULL,
            prompt TEXT NOT NULL,
            required_capabilities_json TEXT NOT NULL,
            workspace_mode TEXT NOT NULL,
            direct_mode_task_opt_in INTEGER NOT NULL,
            metadata_json TEXT,
            provider_id TEXT NOT NULL,
            model TEXT NOT NULL,
            permission_mode TEXT NOT NULL,
            timeout_seconds INTEGER,
            status TEXT NOT NULL,
            result_json TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            started_at TEXT,
            completed_at TEXT,
            cancelled_at TEXT,
            failed_at TEXT
        );

        CREATE INDEX IF NOT EXISTS tasks_owner_product_idx ON tasks(owner_product_id);
        CREATE INDEX IF NOT EXISTS tasks_agent_idx ON tasks(agent_id);
        CREATE INDEX IF NOT EXISTS tasks_directory_idx ON tasks(directory_id);
        CREATE INDEX IF NOT EXISTS tasks_status_idx ON tasks(status);

        CREATE TABLE IF NOT EXISTS task_events (
            id TEXT PRIMARY KEY,
            task_id TEXT NOT NULL,
            sequence INTEGER NOT NULL,
            event_type TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(task_id, sequence)
        );

        CREATE INDEX IF NOT EXISTS task_events_task_idx
        ON task_events(task_id, sequence);

        CREATE TABLE IF NOT EXISTS task_permission_requests (
            request_id TEXT PRIMARY KEY,
            task_id TEXT NOT NULL,
            sequence INTEGER NOT NULL,
            provider_id TEXT NOT NULL,
            permission_kind TEXT NOT NULL,
            status TEXT NOT NULL,
            request_payload_json TEXT NOT NULL,
            response_payload_json TEXT,
            requested_at TEXT NOT NULL,
            responded_at TEXT
        );

        CREATE INDEX IF NOT EXISTS task_permission_requests_task_idx
        ON task_permission_requests(task_id, status);

        CREATE TABLE IF NOT EXISTS directory_locks (
            directory_id TEXT NOT NULL,
            task_id TEXT NOT NULL,
            mode TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL,
            released_at TEXT,
            PRIMARY KEY(directory_id, task_id)
        );

        CREATE INDEX IF NOT EXISTS directory_locks_active_idx
        ON directory_locks(directory_id, status);

        CREATE TABLE IF NOT EXISTS products (
            id TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            status TEXT NOT NULL,
            description TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS product_tokens (
            id TEXT PRIMARY KEY,
            product_id TEXT NOT NULL,
            label TEXT NOT NULL,
            scopes_json TEXT NOT NULL,
            token_prefix TEXT NOT NULL,
            token_digest_hex TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL,
            last_used_at TEXT,
            revoked_at TEXT
        );

        CREATE INDEX IF NOT EXISTS product_tokens_product_idx
        ON product_tokens(product_id, revoked_at);

        CREATE TABLE IF NOT EXISTS daemon_state (
            daemon_id TEXT PRIMARY KEY,
            control_plane_url TEXT NOT NULL,
            daemon_token TEXT NOT NULL,
            status TEXT NOT NULL,
            registered_at TEXT NOT NULL,
            last_heartbeat_at TEXT,
            last_error_code TEXT,
            session_id TEXT
        );
        "#,
    )
}
