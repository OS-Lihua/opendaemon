use opendaemon_console_api::{
    dto::{
        AgentProfile, DirectoryGrant, ExecutionPolicy, ProviderCapability, ProviderConfig, Session,
        WorkspaceMode,
    },
    policy::{can_use_direct_mode, can_use_remote_execution, has_scope},
};

fn session(scopes: &[&str]) -> Session {
    Session {
        credential_type: "product".to_owned(),
        product_id: Some("product_a".to_owned()),
        scopes: scopes.iter().map(|scope| (*scope).to_owned()).collect(),
        product_status: Some("active".to_owned()),
    }
}

fn agent(allow_direct: bool) -> AgentProfile {
    AgentProfile {
        id: "agent_a".to_owned(),
        name: "Agent A".to_owned(),
        owner_product_id: "product_a".to_owned(),
        provider_id: "provider_a".to_owned(),
        model: "model-a".to_owned(),
        instructions: None,
        execution_policy: ExecutionPolicy {
            default_workspace_mode: WorkspaceMode::Worktree,
            allow_direct_directory: allow_direct,
        },
        provider_config: ProviderConfig::default(),
        created_at: None,
        updated_at: None,
    }
}

fn grant(allow_direct: bool, allow_remote: bool) -> DirectoryGrant {
    DirectoryGrant {
        id: "grant_a".to_owned(),
        product_id: "product_a".to_owned(),
        agent_id: "agent_a".to_owned(),
        path: "/tmp/work".to_owned(),
        capabilities: vec!["read".to_owned(), "write".to_owned()],
        workspace_modes: if allow_direct {
            vec![WorkspaceMode::Worktree, WorkspaceMode::Direct]
        } else {
            vec![WorkspaceMode::Worktree]
        },
        default_workspace_mode: WorkspaceMode::Worktree,
        lock_policy: "exclusive".to_owned(),
        direct_mode_requires_explicit_task_opt_in: true,
        allow_remote_execution: allow_remote,
        created_at: None,
        updated_at: None,
    }
}

#[test]
fn has_scope_matches_exact_scope() {
    assert!(has_scope(&session(&["tasks:read"]), "tasks:read"));
    assert!(!has_scope(&session(&["tasks:read"]), "tasks:create"));
}

#[test]
fn bootstrap_session_has_all_scopes() {
    let mut bootstrap = session(&[]);
    bootstrap.credential_type = "bootstrap".to_owned();
    assert!(has_scope(&bootstrap, "tasks:create"));
}

#[test]
fn direct_mode_requires_scope_agent_and_grant_support() {
    assert!(can_use_direct_mode(
        &session(&["directories:direct"]),
        &agent(true),
        &grant(true, false)
    ));
    assert!(!can_use_direct_mode(
        &session(&[]),
        &agent(true),
        &grant(true, false)
    ));
    assert!(!can_use_direct_mode(
        &session(&["directories:direct"]),
        &agent(false),
        &grant(true, false)
    ));
    assert!(!can_use_direct_mode(
        &session(&["directories:direct"]),
        &agent(true),
        &grant(false, false)
    ));
}

#[test]
fn remote_execution_requires_scope_grant_and_provider_capability() {
    assert!(can_use_remote_execution(
        &session(&["tasks:remote_execution"]),
        &grant(true, true),
        &[ProviderCapability::RemoteExecution]
    ));
    assert!(!can_use_remote_execution(
        &session(&[]),
        &grant(true, true),
        &[ProviderCapability::RemoteExecution]
    ));
    assert!(!can_use_remote_execution(
        &session(&["tasks:remote_execution"]),
        &grant(true, false),
        &[ProviderCapability::RemoteExecution]
    ));
    assert!(!can_use_remote_execution(
        &session(&["tasks:remote_execution"]),
        &grant(true, true),
        &[]
    ));
}
