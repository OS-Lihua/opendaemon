use crate::dto::{AgentProfile, DirectoryGrant, ProviderCapability, Session, WorkspaceMode};

#[must_use]
pub fn has_scope(session: &Session, scope: &str) -> bool {
    session.credential_type == "bootstrap"
        || session.scopes.iter().any(|candidate| candidate == scope)
}

#[must_use]
pub fn can_use_direct_mode(
    session: &Session,
    agent: &AgentProfile,
    grant: &DirectoryGrant,
) -> bool {
    has_scope(session, "directories:direct")
        && agent.execution_policy.allow_direct_directory
        && grant.workspace_modes.contains(&WorkspaceMode::Direct)
}

#[must_use]
pub fn can_use_remote_execution(
    session: &Session,
    grant: &DirectoryGrant,
    provider_capabilities: &[ProviderCapability],
) -> bool {
    has_scope(session, "tasks:remote_execution")
        && grant.allow_remote_execution
        && provider_capabilities.contains(&ProviderCapability::RemoteExecution)
}
