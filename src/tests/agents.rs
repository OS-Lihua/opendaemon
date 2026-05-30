use std::{fs, path::Path};

use axum::{
    Json,
    body::to_bytes,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::{Value, json};

use crate::{
    agent::profile::{
        AgentAuthorizationRequest, AgentProfile, CreateAgentProfile, ExecutionPolicy,
        ProviderConfig, WorkspaceMode,
    },
    api::{
        AppState, agent_create, agent_delete, agent_get, agent_list, agent_patch,
        agents::{AgentListQuery, CreateAgentRequest},
        directories::CreateDirectoryGrantRequest,
        directory_create, directory_get,
    },
    config::{RuntimeDetectionConfig, StoreConfig},
    runtime::store::RuntimeStore,
    store::{
        agent_profiles::{AgentProfileFilters, AgentProfileStore, PatchAgentProfile},
        directory_grants::DirectoryGrantStore,
    },
    tests::{TempDir, temp_registry_with_provider, valid_manifest_json},
};

#[test]
fn profile_model_validates_ids_policy_and_provider_config() {
    assert!(AgentProfile::validate_id("frontend-fixer").is_ok());
    assert!(AgentProfile::validate_id("").is_err());
    assert!(AgentProfile::validate_id(" ").is_err());
    assert!(AgentProfile::validate_id("bad/path").is_err());
    assert!(AgentProfile::validate_id(&"a".repeat(129)).is_err());

    assert_eq!(
        serde_json::to_value(WorkspaceMode::Worktree).unwrap(),
        json!("worktree")
    );
    assert_eq!(
        serde_json::to_value(WorkspaceMode::Direct).unwrap(),
        json!("direct")
    );
    assert_eq!(
        ExecutionPolicy::default(),
        ExecutionPolicy {
            default_workspace_mode: WorkspaceMode::Worktree,
            allow_direct_directory: false,
        }
    );
    assert!(
        ExecutionPolicy {
            default_workspace_mode: WorkspaceMode::Direct,
            allow_direct_directory: false,
        }
        .validate()
        .is_err()
    );

    assert_eq!(ProviderConfig::default().custom_args, Vec::<String>::new());
    assert_eq!(
        ProviderConfig::default().permission_mode,
        "provider_default".to_owned()
    );
    assert!(
        ProviderConfig {
            custom_args: vec!["".to_owned()],
            ..ProviderConfig::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        ProviderConfig {
            custom_args: vec!["--model".to_owned()],
            ..ProviderConfig::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        ProviderConfig {
            custom_args: vec!["--fast".to_owned(), "--fast".to_owned()],
            ..ProviderConfig::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        ProviderConfig {
            custom_env_keys: vec!["bad-name".to_owned()],
            ..ProviderConfig::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        ProviderConfig {
            custom_env_keys: vec!["OPENAI_API_KEY".to_owned(), "OPENAI_API_KEY".to_owned()],
            ..ProviderConfig::default()
        }
        .validate()
        .is_err()
    );
}

#[test]
fn profile_registry_validation_rejects_unknown_provider_model_and_permission_mode() {
    let mut manifest = valid_manifest_json();
    manifest["models"]["supported"] = json!(["test-model", "other-model"]);
    manifest["permissions"]["provider_permission_modes"] = json!(["default", "plan"]);
    let (_temp_registry, providers_dir) = temp_registry_with_provider("test-provider", manifest);
    let registry = crate::registry::load_registry_from_dir(&providers_dir).unwrap();

    let valid = create_profile("agent", "product");
    assert!(valid.validate_against_registry(&registry).is_ok());

    assert!(
        CreateAgentProfile {
            provider_id: "missing".to_owned(),
            ..valid.clone()
        }
        .validate_against_registry(&registry)
        .is_err()
    );
    assert!(
        CreateAgentProfile {
            model: "missing-model".to_owned(),
            ..valid.clone()
        }
        .validate_against_registry(&registry)
        .is_err()
    );
    assert!(
        CreateAgentProfile {
            provider_config: ProviderConfig {
                permission_mode: "unknown".to_owned(),
                ..ProviderConfig::default()
            },
            ..valid.clone()
        }
        .validate_against_registry(&registry)
        .is_err()
    );
    assert!(
        CreateAgentProfile {
            provider_config: ProviderConfig {
                permission_mode: "plan".to_owned(),
                ..ProviderConfig::default()
            },
            ..valid
        }
        .validate_against_registry(&registry)
        .is_ok()
    );
}

#[test]
fn sqlite_profile_store_persists_filters_patches_deletes_and_authorizes() {
    let temp_dir = TempDir::new();
    let store = temp_profile_store(temp_dir.path());
    let first = store
        .create(create_profile("agent-a", "product-a"))
        .unwrap();
    let second = store
        .create(create_profile("agent-b", "product-b"))
        .unwrap();

    assert_eq!(first.id, "agent-a");
    assert_eq!(first.execution_policy, ExecutionPolicy::default());
    time::OffsetDateTime::parse(
        &first.created_at,
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();

    let reopened = temp_profile_store(temp_dir.path());
    assert_eq!(
        reopened
            .list(AgentProfileFilters::default())
            .unwrap()
            .iter()
            .map(|profile| profile.id.as_str())
            .collect::<Vec<_>>(),
        ["agent-a", "agent-b"]
    );
    assert_eq!(
        reopened
            .list(AgentProfileFilters {
                owner_product_id: Some("product-a".to_owned()),
                provider_id: None,
            })
            .unwrap(),
        vec![first.clone()]
    );
    assert_eq!(
        reopened
            .list(AgentProfileFilters {
                owner_product_id: None,
                provider_id: Some("test-provider".to_owned()),
            })
            .unwrap()
            .len(),
        2
    );

    let patched = reopened
        .patch(
            &second.id,
            PatchAgentProfile {
                name: Some("Renamed".to_owned()),
                instructions: Some(Some("new instructions".to_owned())),
                provider_config: Some(ProviderConfig {
                    custom_args: vec!["--fast".to_owned()],
                    custom_env_keys: vec!["OPENDAEMON_TOKEN".to_owned()],
                    mcp_config: Some(json!({"servers": []})),
                    permission_mode: "provider_default".to_owned(),
                }),
                ..PatchAgentProfile::default()
            },
        )
        .unwrap();
    assert_eq!(patched.name, "Renamed");
    assert_eq!(patched.instructions.as_deref(), Some("new instructions"));
    assert_eq!(patched.provider_config.custom_args, ["--fast"]);
    assert_ne!(patched.updated_at, second.updated_at);

    let authorized = reopened
        .authorize(&AgentAuthorizationRequest {
            owner_product_id: "product-b".to_owned(),
            agent_id: "agent-b".to_owned(),
            provider_id_override: None,
            model_override: None,
            permission_mode_override: None,
            requested_workspace_mode: WorkspaceMode::Worktree,
        })
        .unwrap();
    assert_eq!(authorized.id, "agent-b");
    assert!(
        reopened
            .authorize(&AgentAuthorizationRequest {
                owner_product_id: "wrong".to_owned(),
                agent_id: "agent-b".to_owned(),
                provider_id_override: None,
                model_override: None,
                permission_mode_override: None,
                requested_workspace_mode: WorkspaceMode::Worktree,
            })
            .is_err()
    );
    assert!(
        reopened
            .authorize(&AgentAuthorizationRequest {
                owner_product_id: "product-b".to_owned(),
                agent_id: "agent-b".to_owned(),
                provider_id_override: Some("other".to_owned()),
                model_override: None,
                permission_mode_override: None,
                requested_workspace_mode: WorkspaceMode::Worktree,
            })
            .is_err()
    );
    assert!(
        reopened
            .authorize(&AgentAuthorizationRequest {
                owner_product_id: "product-b".to_owned(),
                agent_id: "agent-b".to_owned(),
                provider_id_override: None,
                model_override: Some("other".to_owned()),
                permission_mode_override: None,
                requested_workspace_mode: WorkspaceMode::Worktree,
            })
            .is_err()
    );
    assert!(
        reopened
            .authorize(&AgentAuthorizationRequest {
                owner_product_id: "product-b".to_owned(),
                agent_id: "agent-b".to_owned(),
                provider_id_override: None,
                model_override: None,
                permission_mode_override: Some("other".to_owned()),
                requested_workspace_mode: WorkspaceMode::Worktree,
            })
            .is_err()
    );
    assert!(
        reopened
            .authorize(&AgentAuthorizationRequest {
                owner_product_id: "product-b".to_owned(),
                agent_id: "agent-b".to_owned(),
                provider_id_override: None,
                model_override: None,
                permission_mode_override: None,
                requested_workspace_mode: WorkspaceMode::Direct,
            })
            .is_err()
    );

    reopened.delete(&first.id).unwrap();
    assert!(reopened.get(&first.id).is_err());
}

#[tokio::test]
async fn agent_api_creates_lists_gets_patches_deletes_and_filters_profiles() {
    let temp_dir = TempDir::new();
    let state = test_state(temp_dir.path());

    let initial = agent_list(
        State(state.clone()),
        Query(AgentListQuery {
            owner_product_id: None,
            provider_id: None,
        }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(initial.agents, vec![]);

    let created = agent_create(
        State(state.clone()),
        Json(
            serde_json::from_value::<CreateAgentRequest>(json!({
                "id": "frontend-fixer",
                "name": "Frontend Fixer",
                "owner_product_id": "product_example",
                "provider_id": "codex",
                "model": "gpt-5-codex",
                "instructions": "Fix frontend issues.",
                "provider_config": {
                    "custom_args": ["--fast"],
                    "custom_env_keys": ["OPENDAEMON_TOKEN"],
                    "mcp_config": {"servers": []},
                    "permission_mode": "provider_default"
                }
            }))
            .unwrap(),
        ),
    )
    .await
    .unwrap();
    assert_eq!(created.0, StatusCode::CREATED);
    let agent = created.1.0.agent;
    let body = serde_json::to_string(&agent).unwrap();
    assert_eq!(agent.id, "frontend-fixer");
    assert_eq!(agent.execution_policy, ExecutionPolicy::default());
    assert!(!body.contains("runtime"));
    assert!(!body.contains("\"path\""));
    assert!(!body.contains("directories"));
    assert!(!body.contains("task"));
    assert!(!body.contains("secret"));
    assert!(!body.contains("capacity"));

    let fetched = agent_get(State(state.clone()), AxumPath(agent.id.clone()))
        .await
        .unwrap()
        .0
        .agent;
    assert_eq!(fetched, agent);

    let patched = agent_patch(
        State(state.clone()),
        AxumPath(agent.id.clone()),
        Json(json!({
            "name": "Frontend Repair",
            "execution_policy": {
                "default_workspace_mode": "direct",
                "allow_direct_directory": true
            },
            "provider_config": {
                "custom_args": [],
                "custom_env_keys": [],
                "mcp_config": null,
                "permission_mode": "plan"
            }
        })),
    )
    .await
    .unwrap()
    .0
    .agent;
    assert_eq!(patched.name, "Frontend Repair");
    assert_eq!(
        patched.execution_policy.default_workspace_mode,
        WorkspaceMode::Direct
    );
    assert_eq!(patched.provider_config.permission_mode, "plan");

    let filtered = agent_list(
        State(state.clone()),
        Query(AgentListQuery {
            owner_product_id: Some("product_example".to_owned()),
            provider_id: Some("codex".to_owned()),
        }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(filtered.agents, vec![patched.clone()]);

    let delete_status = agent_delete(State(state.clone()), AxumPath(agent.id.clone()))
        .await
        .unwrap();
    assert_eq!(delete_status, StatusCode::NO_CONTENT);

    let error = agent_get(State(state), AxumPath(agent.id))
        .await
        .unwrap_err();
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "agent_not_found");
}

#[tokio::test]
async fn agent_api_returns_stable_errors_for_invalid_requests() {
    let temp_dir = TempDir::new();
    let state = test_state(temp_dir.path());

    let error = agent_create(
        State(state.clone()),
        Json(
            serde_json::from_value::<CreateAgentRequest>(json!({
                "id": "bad/path",
                "name": "Bad",
                "owner_product_id": "product",
                "provider_id": "codex",
                "model": "gpt-5-codex"
            }))
            .unwrap(),
        ),
    )
    .await
    .unwrap_err();
    assert_error(error, StatusCode::BAD_REQUEST, "invalid_agent_id").await;

    let error = agent_create(
        State(state.clone()),
        Json(
            serde_json::from_value::<CreateAgentRequest>(json!({
                "id": "agent",
                "name": "Bad",
                "owner_product_id": "product",
                "provider_id": "missing",
                "model": "gpt-5-codex"
            }))
            .unwrap(),
        ),
    )
    .await
    .unwrap_err();
    assert_error(error, StatusCode::BAD_REQUEST, "provider_not_found").await;

    let error = agent_patch(
        State(state),
        AxumPath("missing".to_owned()),
        Json(json!({"id": "other"})),
    )
    .await
    .unwrap_err();
    assert_error(error, StatusCode::BAD_REQUEST, "invalid_agent_profile").await;
}

#[tokio::test]
async fn directory_grant_creation_validates_agent_profile_scope_without_embedding_profile() {
    let temp_dir = TempDir::new();
    let state = test_state(temp_dir.path());
    let project = create_project(temp_dir.path(), "project");

    let missing_error = directory_create(
        State(state.clone()),
        Json(
            serde_json::from_value::<CreateDirectoryGrantRequest>(json!({
                "product_id": "product_example",
                "agent_id": "frontend-fixer",
                "path": project,
                "capabilities": ["read"],
                "workspace_modes": ["direct"],
                "default_workspace_mode": "direct",
                "lock_policy": "shared"
            }))
            .unwrap(),
        ),
    )
    .await
    .unwrap_err();
    let response = missing_error.into_response();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "agent_not_found");

    let _created = agent_create(
        State(state.clone()),
        Json(
            serde_json::from_value::<CreateAgentRequest>(json!({
                "id": "frontend-fixer",
                "name": "Frontend Fixer",
                "owner_product_id": "product_example",
                "provider_id": "codex",
                "model": "gpt-5-codex",
                "execution_policy": {
                    "default_workspace_mode": "direct",
                    "allow_direct_directory": true
                }
            }))
            .unwrap(),
        ),
    )
    .await
    .unwrap();

    let directory = directory_create(
        State(state.clone()),
        Json(
            serde_json::from_value::<CreateDirectoryGrantRequest>(json!({
                "product_id": "product_example",
                "agent_id": "frontend-fixer",
                "path": project,
                "capabilities": ["read"],
                "workspace_modes": ["direct"],
                "default_workspace_mode": "direct",
                "lock_policy": "shared"
            }))
            .unwrap(),
        ),
    )
    .await
    .unwrap()
    .1
    .0
    .directory;
    let body = serde_json::to_string(&directory).unwrap();
    assert_eq!(directory.agent_id, "frontend-fixer");
    assert!(!body.contains("provider_config"));
    assert!(!body.contains("instructions"));

    agent_delete(State(state.clone()), AxumPath("frontend-fixer".to_owned()))
        .await
        .unwrap();
    let still_exists = directory_get(State(state), AxumPath(directory.id))
        .await
        .unwrap()
        .0
        .directory;
    assert_eq!(still_exists.agent_id, "frontend-fixer");
}

async fn assert_error(error: crate::api::agents::ApiError, status: StatusCode, code: &str) {
    let response = error.into_response();
    assert_eq!(response.status(), status);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], code);
}

fn temp_profile_store(root: &Path) -> AgentProfileStore {
    AgentProfileStore::open(StoreConfig::new(root.join("opendaemon.sqlite3"))).unwrap()
}

fn temp_directory_store(root: &Path) -> DirectoryGrantStore {
    DirectoryGrantStore::open(StoreConfig::new(root.join("opendaemon.sqlite3"))).unwrap()
}

fn test_state(root: &Path) -> AppState {
    AppState::with_stores(
        crate::registry::default_providers_dir(),
        RuntimeStore::default(),
        RuntimeDetectionConfig::default(),
        temp_directory_store(root),
        temp_profile_store(root),
    )
}

fn create_profile(id: &str, product_id: &str) -> CreateAgentProfile {
    CreateAgentProfile {
        id: id.to_owned(),
        name: format!("{id} name"),
        owner_product_id: product_id.to_owned(),
        provider_id: "test-provider".to_owned(),
        model: "test-model".to_owned(),
        instructions: None,
        execution_policy: ExecutionPolicy::default(),
        provider_config: ProviderConfig::default(),
    }
}

fn create_project(root: &Path, name: &str) -> std::path::PathBuf {
    let path = root.join(name);
    fs::create_dir_all(&path).unwrap();
    path
}
