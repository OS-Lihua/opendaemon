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
    agent::profile::{CreateAgentProfile, ExecutionPolicy, ProviderConfig},
    api::{
        AppState,
        directories::{CreateDirectoryGrantRequest, DirectoryListQuery},
        directory_create, directory_delete, directory_get, directory_list, directory_patch,
    },
    config::{RuntimeDetectionConfig, StoreConfig},
    runtime::store::RuntimeStore,
    security::{
        directory::{
            DirectoryAuthorizationRequest, DirectoryCapability, DirectoryGrantPolicy,
            DirectoryLockPolicy, WorkspaceMode,
        },
        path_guard::{PathGuardError, canonicalize_grant_path, ensure_child_path_within_grant},
    },
    store::{
        agent_profiles::AgentProfileStore,
        directory_grants::{
            CreateDirectoryGrant, DirectoryGrantFilters, DirectoryGrantStore, PatchDirectoryGrant,
        },
    },
    tests::TempDir,
};

#[test]
fn directory_enums_serialize_to_stable_values() {
    assert_eq!(
        serde_json::to_value(DirectoryCapability::Read).unwrap(),
        json!("read")
    );
    assert_eq!(
        serde_json::to_value(DirectoryCapability::Write).unwrap(),
        json!("write")
    );
    assert_eq!(
        serde_json::to_value(DirectoryCapability::Shell).unwrap(),
        json!("shell")
    );
    assert_eq!(
        serde_json::to_value(DirectoryCapability::Git).unwrap(),
        json!("git")
    );
    assert_eq!(
        serde_json::to_value(WorkspaceMode::Worktree).unwrap(),
        json!("worktree")
    );
    assert_eq!(
        serde_json::to_value(WorkspaceMode::Direct).unwrap(),
        json!("direct")
    );
    assert_eq!(
        serde_json::to_value(DirectoryLockPolicy::Exclusive).unwrap(),
        json!("exclusive")
    );
    assert_eq!(
        serde_json::to_value(DirectoryLockPolicy::Shared).unwrap(),
        json!("shared")
    );
    assert_eq!(
        serde_json::to_value(DirectoryLockPolicy::None).unwrap(),
        json!("none")
    );
}

#[test]
fn directory_grant_policy_validation_rejects_invalid_combinations() {
    assert!(
        DirectoryGrantPolicy::new(
            vec![],
            vec![WorkspaceMode::Direct],
            WorkspaceMode::Direct,
            DirectoryLockPolicy::Shared,
            true,
        )
        .is_err()
    );
    assert!(
        DirectoryGrantPolicy::new(
            vec![DirectoryCapability::Read],
            vec![],
            WorkspaceMode::Direct,
            DirectoryLockPolicy::Shared,
            true,
        )
        .is_err()
    );
    assert!(
        DirectoryGrantPolicy::new(
            vec![DirectoryCapability::Read],
            vec![WorkspaceMode::Worktree],
            WorkspaceMode::Direct,
            DirectoryLockPolicy::Shared,
            true,
        )
        .is_err()
    );
    assert!(
        DirectoryGrantPolicy::new(
            vec![DirectoryCapability::Write],
            vec![WorkspaceMode::Direct],
            WorkspaceMode::Direct,
            DirectoryLockPolicy::None,
            true,
        )
        .is_err()
    );
}

#[test]
fn path_guard_canonicalizes_and_rejects_invalid_paths() {
    let temp_dir = TempDir::new();
    let directory = temp_dir.path().join("project");
    let file = temp_dir.path().join("file.txt");
    fs::create_dir_all(&directory).unwrap();
    fs::write(&file, "not a directory").unwrap();

    assert_eq!(
        canonicalize_grant_path(temp_dir.path().join("missing")).unwrap_err(),
        PathGuardError::InvalidDirectoryPath
    );
    assert_eq!(
        canonicalize_grant_path(&file).unwrap_err(),
        PathGuardError::PathNotDirectory
    );

    let canonical = canonicalize_grant_path(&directory).unwrap();
    assert!(canonical.is_absolute());
    assert_eq!(canonical, fs::canonicalize(&directory).unwrap());
}

#[test]
fn path_guard_rejects_traversal_and_symlink_escape() {
    let temp_dir = TempDir::new();
    let grant = temp_dir.path().join("grant");
    let outside = temp_dir.path().join("outside");
    fs::create_dir_all(grant.join("src")).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(grant.join("src/lib.rs"), "fn main() {}").unwrap();
    fs::write(outside.join("secret.txt"), "secret").unwrap();

    let inside = ensure_child_path_within_grant(&grant, "src/lib.rs").unwrap();
    assert_eq!(inside, fs::canonicalize(grant.join("src/lib.rs")).unwrap());

    assert_eq!(
        ensure_child_path_within_grant(&grant, "../outside/secret.txt").unwrap_err(),
        PathGuardError::PathOutsideGrant
    );

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, grant.join("outside-link")).unwrap();
        assert_eq!(
            ensure_child_path_within_grant(&grant, "outside-link/secret.txt").unwrap_err(),
            PathGuardError::SymlinkEscape
        );
    }
}

#[test]
fn sqlite_store_persists_lists_filters_patches_and_deletes_grants() {
    let temp_dir = TempDir::new();
    let store = temp_store(temp_dir.path());
    let project_a = create_project(temp_dir.path(), "project-a", false);
    let project_b = create_project(temp_dir.path(), "project-b", false);

    let first = store
        .create(create_grant(&project_a, "product-a", "agent-a"))
        .unwrap();
    let second = store
        .create(create_grant(&project_b, "product-b", "agent-a"))
        .unwrap();

    assert_eq!(first.id, "dir_1");
    assert_eq!(
        first.path,
        fs::canonicalize(&project_a).unwrap().display().to_string()
    );
    assert_eq!(first.lock_policy, DirectoryLockPolicy::Shared);
    time::OffsetDateTime::parse(
        &first.created_at,
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();

    let reopened = temp_store(temp_dir.path());
    let all = reopened.list(DirectoryGrantFilters::default()).unwrap();
    assert_eq!(
        all.iter()
            .map(|grant| grant.id.as_str())
            .collect::<Vec<_>>(),
        ["dir_1", "dir_2"]
    );

    let product_filtered = reopened
        .list(DirectoryGrantFilters {
            product_id: Some("product-a".to_owned()),
            agent_id: None,
        })
        .unwrap();
    assert_eq!(product_filtered, vec![first.clone()]);

    let agent_filtered = reopened
        .list(DirectoryGrantFilters {
            product_id: None,
            agent_id: Some("agent-a".to_owned()),
        })
        .unwrap();
    assert_eq!(agent_filtered.len(), 2);

    let patched = reopened
        .patch(
            &second.id,
            PatchDirectoryGrant {
                capabilities: Some(vec![DirectoryCapability::Read, DirectoryCapability::Write]),
                lock_policy: Some(DirectoryLockPolicy::Exclusive),
                ..PatchDirectoryGrant::default()
            },
        )
        .unwrap();
    assert!(patched.capabilities.contains(&DirectoryCapability::Write));
    assert_eq!(patched.lock_policy, DirectoryLockPolicy::Exclusive);
    assert_ne!(patched.updated_at, second.updated_at);

    reopened.delete(&first.id).unwrap();
    assert!(matches!(
        reopened.get(&first.id).unwrap_err(),
        crate::store::directory_grants::DirectoryStoreError::NotFound
    ));
}

#[test]
fn sqlite_store_validates_create_and_patch_policy() {
    let temp_dir = TempDir::new();
    let store = temp_store(temp_dir.path());
    let project = create_project(temp_dir.path(), "project", false);

    let worktree_only = CreateDirectoryGrant {
        workspace_modes: Some(vec![WorkspaceMode::Worktree]),
        default_workspace_mode: Some(WorkspaceMode::Worktree),
        ..create_grant(&project, "product", "agent")
    };
    assert!(store.create(worktree_only).is_err());

    let grant = store
        .create(create_grant(&project, "product", "agent"))
        .unwrap();
    let invalid_patch = PatchDirectoryGrant {
        capabilities: Some(vec![DirectoryCapability::Write]),
        lock_policy: Some(DirectoryLockPolicy::None),
        ..PatchDirectoryGrant::default()
    };
    assert!(store.patch(&grant.id, invalid_patch).is_err());
}

#[test]
fn authorization_helper_validates_scope_capabilities_and_workspace_policy() {
    let temp_dir = TempDir::new();
    let store = temp_store(temp_dir.path());
    let project = create_project(temp_dir.path(), "project", false);
    let grant = store
        .create(create_grant(&project, "product", "agent"))
        .unwrap();

    let request = DirectoryAuthorizationRequest {
        product_id: "product".to_owned(),
        agent_id: "agent".to_owned(),
        directory_id: grant.id.clone(),
        required_capabilities: vec![DirectoryCapability::Read],
        requested_workspace_mode: WorkspaceMode::Direct,
        direct_mode_task_opt_in: true,
    };
    assert_eq!(store.authorize(&request).unwrap(), grant);

    assert!(
        store
            .authorize(&DirectoryAuthorizationRequest {
                product_id: "wrong".to_owned(),
                ..request.clone()
            })
            .is_err()
    );
    assert!(
        store
            .authorize(&DirectoryAuthorizationRequest {
                agent_id: "wrong".to_owned(),
                ..request.clone()
            })
            .is_err()
    );
    assert!(
        store
            .authorize(&DirectoryAuthorizationRequest {
                required_capabilities: vec![DirectoryCapability::Write],
                ..request.clone()
            })
            .is_err()
    );
    assert!(
        store
            .authorize(&DirectoryAuthorizationRequest {
                requested_workspace_mode: WorkspaceMode::Worktree,
                ..request.clone()
            })
            .is_err()
    );
    assert!(
        store
            .authorize(&DirectoryAuthorizationRequest {
                direct_mode_task_opt_in: false,
                ..request
            })
            .is_err()
    );
}

#[tokio::test]
async fn directory_api_lists_creates_gets_patches_and_deletes_grants() {
    let temp_dir = TempDir::new();
    let state = test_state(temp_dir.path());
    let project = create_project(temp_dir.path(), "project", false);
    create_test_agent(temp_dir.path(), "agent", "product");

    let initial = directory_list(
        State(state.clone()),
        Query(DirectoryListQuery {
            product_id: None,
            agent_id: None,
        }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(initial.directories, vec![]);

    let created = directory_create(
        State(state.clone()),
        Json(
            serde_json::from_value::<CreateDirectoryGrantRequest>(json!({
                "product_id": "product",
                "agent_id": "agent",
                "path": project,
                "capabilities": ["read"],
                "workspace_modes": ["direct"],
                "default_workspace_mode": "direct",
                "lock_policy": "shared",
                "direct_mode_requires_explicit_task_opt_in": true
            }))
            .unwrap(),
        ),
    )
    .await
    .unwrap();
    assert_eq!(created.0, StatusCode::CREATED);
    let directory = created.1.0.directory;
    let body = serde_json::to_string(&directory).unwrap();
    assert_eq!(directory.id, "dir_1");
    assert!(!body.contains("secrets"));
    assert!(!body.contains("runtime"));
    assert!(!body.contains("tasks"));
    assert!(!body.contains("capacity"));

    let fetched = directory_get(State(state.clone()), AxumPath(directory.id.clone()))
        .await
        .unwrap()
        .0
        .directory;
    assert_eq!(fetched, directory);

    let patched = directory_patch(
        State(state.clone()),
        AxumPath(directory.id.clone()),
        Json(json!({
            "capabilities": ["read", "shell"],
            "lock_policy": "shared"
        })),
    )
    .await
    .unwrap()
    .0
    .directory;
    assert!(patched.capabilities.contains(&DirectoryCapability::Shell));

    let filtered = directory_list(
        State(state.clone()),
        Query(DirectoryListQuery {
            product_id: Some("product".to_owned()),
            agent_id: Some("agent".to_owned()),
        }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(filtered.directories.len(), 1);

    let delete_status = directory_delete(State(state.clone()), AxumPath(directory.id.clone()))
        .await
        .unwrap();
    assert_eq!(delete_status, StatusCode::NO_CONTENT);
    assert!(Path::new(&patched.path).exists());

    let error = directory_get(State(state), AxumPath(directory.id))
        .await
        .unwrap_err();
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "directory_not_found");
}

#[tokio::test]
async fn directory_api_returns_stable_errors_for_invalid_requests() {
    let temp_dir = TempDir::new();
    let state = test_state(temp_dir.path());
    create_test_agent(temp_dir.path(), "agent", "product");

    let error = directory_create(
        State(state.clone()),
        Json(
            serde_json::from_value::<CreateDirectoryGrantRequest>(json!({
                "product_id": "product",
                "agent_id": "agent",
                "path": temp_dir.path().join("missing"),
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
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "invalid_directory_path");

    let project = create_project(temp_dir.path(), "project", false);
    let created = directory_create(
        State(state.clone()),
        Json(
            serde_json::from_value::<CreateDirectoryGrantRequest>(json!({
                "product_id": "product",
                "agent_id": "agent",
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

    let error = directory_patch(
        State(state),
        AxumPath(created.id),
        Json(json!({"path": "/tmp"})),
    )
    .await
    .unwrap_err();
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "directory_authorization_failed");
}

fn temp_store(root: &Path) -> DirectoryGrantStore {
    DirectoryGrantStore::open(StoreConfig::new(root.join("opendaemon.sqlite3"))).unwrap()
}

fn test_state(root: &Path) -> AppState {
    AppState::with_stores(
        crate::registry::default_providers_dir(),
        RuntimeStore::default(),
        RuntimeDetectionConfig::default(),
        temp_store(root),
        temp_profile_store(root),
    )
}

fn temp_profile_store(root: &Path) -> AgentProfileStore {
    AgentProfileStore::open(StoreConfig::new(root.join("opendaemon.sqlite3"))).unwrap()
}

fn create_test_agent(root: &Path, agent_id: &str, product_id: &str) {
    temp_profile_store(root)
        .create(CreateAgentProfile {
            id: agent_id.to_owned(),
            name: format!("{agent_id} name"),
            owner_product_id: product_id.to_owned(),
            provider_id: "codex".to_owned(),
            model: "gpt-5-codex".to_owned(),
            instructions: None,
            execution_policy: ExecutionPolicy {
                default_workspace_mode: crate::agent::profile::WorkspaceMode::Direct,
                allow_direct_directory: true,
            },
            provider_config: ProviderConfig::default(),
        })
        .unwrap();
}

fn create_project(root: &Path, name: &str, git: bool) -> std::path::PathBuf {
    let path = root.join(name);
    fs::create_dir_all(&path).unwrap();
    if git {
        fs::create_dir_all(path.join(".git")).unwrap();
    }
    path
}

fn create_grant(path: &Path, product_id: &str, agent_id: &str) -> CreateDirectoryGrant {
    CreateDirectoryGrant {
        product_id: product_id.to_owned(),
        agent_id: agent_id.to_owned(),
        path: path.to_path_buf(),
        capabilities: vec![DirectoryCapability::Read],
        workspace_modes: Some(vec![WorkspaceMode::Direct]),
        default_workspace_mode: Some(WorkspaceMode::Direct),
        lock_policy: Some(DirectoryLockPolicy::Shared),
        direct_mode_requires_explicit_task_opt_in: Some(true),
    }
}
