use std::{fs, path::Path};

use axum::{
    body::to_bytes,
    http::{HeaderValue, Request, StatusCode},
};
use serde_json::{Value, json};

use crate::{
    agent::profile::{CreateAgentProfile, ExecutionPolicy, ProviderConfig},
    api::{AppState, router_with_state},
    config::{AuthConfig, RuntimeDetectionConfig, StoreConfig},
    runtime::store::RuntimeStore,
    security::directory::DirectoryCapability,
    store::{
        agent_profiles::AgentProfileStore,
        directory_grants::{CreateDirectoryGrant, DirectoryGrantStore},
        products::ProductStore,
        tasks::TaskStore,
    },
    task::{
        event::{PermissionDecision, PermissionRequestEvent},
        model::CreateTask,
    },
    tests::TempDir,
};
use tower::ServiceExt;

#[tokio::test]
async fn session_route_introspects_bootstrap_and_product_tokens_without_returning_raw_tokens() {
    let temp_dir = TempDir::new();
    let state = auth_fixture_state(temp_dir.path(), Some("bootstrap-secret".to_owned()));
    let app = router_with_state(state);

    let bootstrap = request_json(
        app.clone(),
        Request::builder()
            .uri("/v1/session")
            .header("authorization", "Bearer bootstrap-secret")
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(bootstrap.0, StatusCode::OK);
    assert_eq!(bootstrap.1["credential_type"], "bootstrap");
    assert!(bootstrap.1["product_id"].is_null());
    assert_eq!(bootstrap.1["scopes"].as_array().unwrap().len(), 0);
    assert!(!bootstrap.1.to_string().contains("bootstrap-secret"));

    let product_token = create_product_and_token(
        app.clone(),
        "bootstrap-secret",
        "product_a",
        &["providers:read", "runtimes:read", "tasks:read"],
    )
    .await;
    let product = request_json(
        app.clone(),
        Request::builder()
            .uri("/v1/session")
            .header(
                "authorization",
                HeaderValue::from_str(&format!("Bearer {product_token}")).unwrap(),
            )
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(product.0, StatusCode::OK);
    assert_eq!(product.1["credential_type"], "product");
    assert_eq!(product.1["product_id"], "product_a");
    assert_eq!(product.1["product_status"], "active");
    assert_eq!(
        product.1["scopes"],
        json!(["providers:read", "runtimes:read", "tasks:read"])
    );
    assert!(!product.1.to_string().contains(&product_token));

    let missing = app
        .oneshot(
            Request::builder()
                .uri("/v1/session")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn bootstrap_can_read_provider_and_runtime_setup_routes() {
    let temp_dir = TempDir::new();
    let state = auth_fixture_state(temp_dir.path(), Some("bootstrap-secret".to_owned()));
    let app = router_with_state(state);

    let providers = request_json(
        app.clone(),
        Request::builder()
            .uri("/v1/providers")
            .header("authorization", "Bearer bootstrap-secret")
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(providers.0, StatusCode::OK);
    assert!(!providers.1["providers"].as_array().unwrap().is_empty());

    let runtimes = request_json(
        app,
        Request::builder()
            .uri("/v1/runtimes")
            .header("authorization", "Bearer bootstrap-secret")
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(runtimes.0, StatusCode::OK);
    assert!(!runtimes.1["runtimes"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn daemon_status_summarizes_visible_state_without_tokens_or_directory_paths() {
    let temp_dir = TempDir::new();
    let state = auth_fixture_state(temp_dir.path(), Some("bootstrap-secret".to_owned()));
    create_agent_directory_task_and_permission(temp_dir.path(), &state, "product_a");
    create_agent_directory_task_and_permission(temp_dir.path(), &state, "product_b");
    let app = router_with_state(state);
    let product_token = create_product_and_token(
        app.clone(),
        "bootstrap-secret",
        "product_a",
        &["runtimes:read", "tasks:read"],
    )
    .await;

    let bootstrap = request_json(
        app.clone(),
        Request::builder()
            .uri("/v1/daemon/status")
            .header("authorization", "Bearer bootstrap-secret")
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(bootstrap.0, StatusCode::OK);
    assert_eq!(bootstrap.1["service"], "opendaemon");
    assert_eq!(bootstrap.1["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(bootstrap.1["status"], "online");
    assert_eq!(bootstrap.1["scheduler"]["queued"], 2);
    assert_eq!(bootstrap.1["permissions"]["pending"], 2);
    let bootstrap_body = bootstrap.1.to_string();
    assert!(!bootstrap_body.contains("bootstrap-secret"));
    assert!(!bootstrap_body.contains("odpk_"));
    assert!(!bootstrap_body.contains(temp_dir.path().to_string_lossy().as_ref()));

    let product = request_json(
        app,
        Request::builder()
            .uri("/v1/daemon/status")
            .header(
                "authorization",
                HeaderValue::from_str(&format!("Bearer {product_token}")).unwrap(),
            )
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(product.0, StatusCode::OK);
    assert_eq!(product.1["scheduler"]["queued"], 1);
    assert_eq!(product.1["permissions"]["pending"], 1);
    assert!(!product.1.to_string().contains(&product_token));
}

#[tokio::test]
async fn permission_inbox_lists_only_visible_pending_requests() {
    let temp_dir = TempDir::new();
    let state = auth_fixture_state(temp_dir.path(), Some("bootstrap-secret".to_owned()));
    create_agent_directory_task_and_permission(temp_dir.path(), &state, "product_a");
    create_agent_directory_task_and_permission(temp_dir.path(), &state, "product_b");
    let app = router_with_state(state);
    let product_token = create_product_and_token(
        app.clone(),
        "bootstrap-secret",
        "product_a",
        &["tasks:read"],
    )
    .await;

    let bootstrap = request_json(
        app.clone(),
        Request::builder()
            .uri("/v1/permissions?status=pending")
            .header("authorization", "Bearer bootstrap-secret")
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(bootstrap.0, StatusCode::OK);
    assert_eq!(bootstrap.1["permissions"].as_array().unwrap().len(), 2);

    let product = request_json(
        app,
        Request::builder()
            .uri("/v1/permissions?status=pending")
            .header(
                "authorization",
                HeaderValue::from_str(&format!("Bearer {product_token}")).unwrap(),
            )
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(product.0, StatusCode::OK);
    let permissions = product.1["permissions"].as_array().unwrap();
    assert_eq!(permissions.len(), 1);
    assert_eq!(permissions[0]["task_id"], "task_1");
    assert_eq!(permissions[0]["request_id"], "perm_product_a");
    assert_eq!(permissions[0]["options"], json!(["approve", "deny"]));
}

#[tokio::test]
async fn console_static_routes_are_public_without_weakening_api_authentication() {
    let temp_dir = TempDir::new();
    let state = auth_fixture_state(temp_dir.path(), Some("bootstrap-secret".to_owned()));
    let app = router_with_state(state);

    let console = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/console")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(console.status(), StatusCode::OK);
    let console_body = to_bytes(console.into_body(), usize::MAX).await.unwrap();
    let console_text = String::from_utf8(console_body.to_vec()).unwrap();
    assert!(console_text.contains("OpenDaemon Console"));

    let deep_link = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/console/tasks/task_1")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deep_link.status(), StatusCode::OK);

    let api = app
        .oneshot(
            Request::builder()
                .uri("/v1/session")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(api.status(), StatusCode::UNAUTHORIZED);
}

async fn request_json(
    app: axum::Router,
    request: Request<axum::body::Body>,
) -> (StatusCode, Value) {
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json = serde_json::from_slice(&body).unwrap_or_else(|error| {
        panic!(
            "expected json body, got error {error} and body {}",
            String::from_utf8_lossy(&body)
        )
    });
    (status, json)
}

async fn create_product_and_token(
    app: axum::Router,
    bootstrap_token: &str,
    product_id: &str,
    scopes: &[&str],
) -> String {
    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/products")
                .header("authorization", format!("Bearer {bootstrap_token}"))
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "id": product_id,
                        "display_name": product_id,
                        "description": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);

    let token = request_json(
        app,
        Request::builder()
            .method("POST")
            .uri(format!("/v1/products/{product_id}/tokens"))
            .header("authorization", format!("Bearer {bootstrap_token}"))
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                json!({
                    "label": "console",
                    "scopes": scopes
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(token.0, StatusCode::CREATED);
    token.1["token"]["token"].as_str().unwrap().to_owned()
}

fn auth_fixture_state(root: &Path, bootstrap_token: Option<String>) -> AppState {
    let store_config = StoreConfig::new(root.join("opendaemon.sqlite3"));
    AppState::with_task_store(
        crate::registry::default_providers_dir(),
        RuntimeStore::default(),
        RuntimeDetectionConfig::default(),
        AuthConfig { bootstrap_token },
        ProductStore::open(store_config.clone()).unwrap(),
        DirectoryGrantStore::open(store_config.clone()).unwrap(),
        AgentProfileStore::open(store_config.clone()).unwrap(),
        TaskStore::open(store_config).unwrap(),
        crate::config::SchedulerConfig::default(),
    )
}

fn create_agent_directory_task_and_permission(root: &Path, state: &AppState, product_id: &str) {
    let agent_id = format!("agent_{product_id}");
    state
        .agent_profile_store()
        .create(CreateAgentProfile {
            id: agent_id.clone(),
            name: format!("Agent {product_id}"),
            owner_product_id: product_id.to_owned(),
            provider_id: "codex".to_owned(),
            model: "gpt-5-codex".to_owned(),
            instructions: None,
            execution_policy: ExecutionPolicy::default(),
            provider_config: ProviderConfig::default(),
        })
        .unwrap();
    let directory_path = root.join(format!("project-{product_id}"));
    fs::create_dir_all(directory_path.join(".git")).unwrap();
    let directory = state
        .directory_grant_store()
        .create(CreateDirectoryGrant {
            product_id: product_id.to_owned(),
            agent_id: agent_id.clone(),
            path: directory_path,
            capabilities: vec![DirectoryCapability::Read],
            workspace_modes: None,
            default_workspace_mode: None,
            lock_policy: None,
            direct_mode_requires_explicit_task_opt_in: None,
            allow_remote_execution: None,
        })
        .unwrap();
    let task = state
        .task_store()
        .create(CreateTask {
            owner_product_id: product_id.to_owned(),
            agent_id,
            directory_id: directory.id,
            prompt: "Inspect this project.".to_owned(),
            required_capabilities: None,
            workspace_mode: None,
            direct_mode_task_opt_in: false,
            metadata: None,
            provider_id: None,
            model: None,
            permission_mode: None,
            timeout_seconds: None,
        })
        .unwrap();
    state
        .task_store()
        .record_permission_request(
            &task.id,
            PermissionRequestEvent {
                request_id: format!("perm_{product_id}"),
                provider_id: "codex".to_owned(),
                permission_kind: "shell".to_owned(),
                summary: format!("Run command for {product_id}"),
                details: Some(json!({"command": ["cargo", "test"]})),
                options: vec![PermissionDecision::Approve, PermissionDecision::Deny],
                expires_at: None,
            },
        )
        .unwrap();
}
