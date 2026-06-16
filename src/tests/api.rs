use std::{fs, path::Path};

use axum::{
    body::to_bytes,
    extract::{Path as AxumPath, State},
    http::{HeaderValue, Request, StatusCode},
    response::IntoResponse,
};
use serde_json::{Value, json};

use crate::{
    api::{
        AnyAuth, AppState, HealthResponse, ProductAuth, ProductAuthContext, ProviderApiError,
        health, provider_get, provider_list, router, router_with_state, runtime_detect,
        runtime_list,
    },
    config::{AuthConfig, RuntimeDetectionConfig, RuntimeEnvironment, StoreConfig},
    product::{ApiScope, ProductStatus},
    runtime::store::RuntimeStore,
    store::products::ProductStore,
    tests::{
        TempDir, temp_registry_with_provider, valid_http_manifest_json, valid_manifest_json,
        write_provider_fixture,
    },
};
use tower::ServiceExt;

#[tokio::test]
async fn health_handler_returns_stable_json() {
    let response = health().await.0;

    assert_eq!(
        response,
        HealthResponse {
            status: "ok",
            service: "opendaemon",
            version: env!("CARGO_PKG_VERSION"),
        }
    );
}

#[test]
fn router_builds_with_health_and_provider_routes() {
    let _router = router();
}

#[tokio::test]
async fn provider_list_handler_returns_sorted_providers_without_runtime_state() {
    let response = provider_list(
        any_product_auth(&[ApiScope::ProvidersRead]),
        State(AppState::default()),
    )
    .await
    .unwrap()
    .0;
    let body = serde_json::to_string(&response).unwrap();
    let json: Value = serde_json::to_value(response).unwrap();
    let ids = json["providers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|provider| provider["id"].as_str().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(ids, ["claude", "codex", "generic-test-provider"]);
    assert!(!body.contains("runtime"));
    assert!(!body.contains("detected"));
    assert!(!body.contains("installed"));
    assert!(!body.contains("secrets"));
}

#[tokio::test]
async fn provider_get_handler_returns_provider() {
    let response = provider_get(
        any_product_auth(&[ApiScope::ProvidersRead]),
        State(AppState::default()),
        AxumPath("codex".to_owned()),
    )
    .await
    .unwrap()
    .0;
    let json: Value = serde_json::to_value(response).unwrap();

    assert_eq!(json["provider"]["id"], "codex");
    assert_eq!(json["provider"]["manifest"]["id"], "codex");
}

#[tokio::test]
async fn provider_get_handler_returns_stable_404() {
    let error = provider_get(
        any_product_auth(&[ApiScope::ProvidersRead]),
        State(AppState::default()),
        AxumPath("missing-provider".to_owned()),
    )
    .await
    .unwrap_err();

    assert!(matches!(error, ProviderApiError::ProviderNotFound));
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"]["code"], "provider_not_found");
    assert_eq!(json["error"]["message"], "provider not found");
}

#[tokio::test]
async fn runtimes_get_returns_not_detected_without_spawning_commands() {
    let temp_dir = TempDir::new();
    let marker_path = temp_dir.path().join("command-ran");
    let command_dir = temp_dir.path().join("bin");
    fs::create_dir_all(&command_dir).unwrap();
    write_fake_command(
        &command_dir,
        "test-provider",
        &format!("echo ran > {}", marker_path.display()),
    );
    let mut manifest = valid_manifest_json();
    manifest["detect"]["commands"] = json!(["test-provider"]);
    let (_temp_registry, providers_dir) = temp_registry_with_provider("test-provider", manifest);
    let state = test_state(
        providers_dir,
        RuntimeDetectionConfig::default().with_environment(RuntimeEnvironment::from_vars([(
            "PATH".to_owned(),
            command_dir.as_os_str().to_owned(),
        )])),
    );

    let response = runtime_list(any_product_auth(&[ApiScope::RuntimesRead]), State(state))
        .await
        .unwrap()
        .0;
    let body = serde_json::to_string(&response).unwrap();
    let json: Value = serde_json::to_value(response).unwrap();

    assert_eq!(json["runtimes"][0]["id"], "rt_test_provider_local_cli");
    assert_eq!(json["runtimes"][0]["provider_id"], "test-provider");
    assert_eq!(json["runtimes"][0]["kind"], "local_cli");
    assert_eq!(json["runtimes"][0]["status"], "not_detected");
    assert!(json["runtimes"][0]["detected_at"].is_null());
    assert!(!marker_path.exists());
    assert!(!body.contains("secrets"));
    assert!(!body.contains("grants"));
    assert!(!body.contains("tasks"));
    assert!(!body.contains("capacity"));
}

#[tokio::test]
async fn runtimes_detect_updates_store_and_reports_missing_provider_unavailable() {
    let _guard = crate::tests::runtime_detection_test_guard().await;
    let temp_dir = TempDir::new();
    let command_dir = temp_dir.path().join("bin");
    fs::create_dir_all(&command_dir).unwrap();
    write_fake_command(&command_dir, "test-provider", "echo test-provider 2.3.4");
    let manifest = valid_manifest_json();
    let (_temp_registry, providers_dir) = temp_registry_with_provider("test-provider", manifest);
    let mut missing_manifest = valid_manifest_json();
    missing_manifest["id"] = json!("missing-provider");
    missing_manifest["display_name"] = json!("Missing Provider");
    missing_manifest["detect"]["commands"] = json!(["missing-provider"]);
    write_provider_fixture(&providers_dir, "missing-provider", missing_manifest);
    let state = test_state(
        providers_dir,
        RuntimeDetectionConfig::default().with_environment(RuntimeEnvironment::from_vars([(
            "PATH".to_owned(),
            command_dir.as_os_str().to_owned(),
        )])),
    );

    let response = runtime_detect(
        any_product_auth(&[ApiScope::RuntimesRead]),
        State(state.clone()),
    )
    .await
    .unwrap()
    .0;
    let body = serde_json::to_string(&response).unwrap();
    let json: Value = serde_json::to_value(response).unwrap();

    assert_eq!(json["runtimes"][0]["provider_id"], "missing-provider");
    assert_eq!(json["runtimes"][0]["status"], "unavailable");
    assert_eq!(json["runtimes"][0]["error"]["code"], "command_not_found");
    assert_eq!(json["runtimes"][1]["provider_id"], "test-provider");
    assert_eq!(json["runtimes"][1]["status"], "available");
    assert!(
        json["runtimes"][1]["executable"]
            .as_str()
            .unwrap()
            .ends_with(&command_file_name("test-provider"))
    );
    assert_eq!(json["runtimes"][1]["version"], "test-provider 2.3.4");
    assert!(json["runtimes"][1]["detected_at"].as_str().is_some());
    assert!(!body.contains("secrets"));
    assert!(!body.contains("grants"));
    assert!(!body.contains("tasks"));
    assert!(!body.contains("capacity"));

    let latest = runtime_list(any_product_auth(&[ApiScope::RuntimesRead]), State(state))
        .await
        .unwrap()
        .0;
    let latest_json: Value = serde_json::to_value(latest).unwrap();
    assert_eq!(latest_json["runtimes"][1]["status"], "available");
    assert_eq!(latest_json["runtimes"][1]["version"], "test-provider 2.3.4");
}

#[tokio::test]
async fn runtimes_list_exposes_remote_http_runtime_without_detection_side_effects() {
    let manifest = valid_http_manifest_json();
    let (_temp_registry, providers_dir) = temp_registry_with_provider("test-provider", manifest);
    let state = test_state(providers_dir, RuntimeDetectionConfig::default());

    let response = runtime_list(any_product_auth(&[ApiScope::RuntimesRead]), State(state))
        .await
        .unwrap()
        .0;
    let json: Value = serde_json::to_value(response).unwrap();

    assert_eq!(json["runtimes"][0]["provider_id"], "test-provider");
    assert_eq!(json["runtimes"][0]["kind"], "remote_http");
    assert_eq!(json["runtimes"][0]["status"], "not_detected");
}

fn test_state(
    providers_dir: impl Into<std::path::PathBuf>,
    config: RuntimeDetectionConfig,
) -> AppState {
    let temp_dir = TempDir::new();
    let product_store =
        ProductStore::open(StoreConfig::new(temp_dir.path().join("opendaemon.sqlite3"))).unwrap();
    let state = AppState::with_directory_grant_store(
        providers_dir.into(),
        RuntimeStore::default(),
        config,
        AuthConfig::default(),
        product_store,
        crate::store::directory_grants::DirectoryGrantStore::configured(StoreConfig::new(
            temp_dir.path().join("opendaemon.sqlite3"),
        )),
    );
    std::mem::forget(temp_dir);
    state
}

fn product_auth(scopes: &[ApiScope]) -> ProductAuth {
    ProductAuth(ProductAuthContext {
        token_id: "ptok_test".to_owned(),
        product_id: "product_test".to_owned(),
        scopes: scopes.iter().copied().collect(),
    })
}

fn any_product_auth(scopes: &[ApiScope]) -> AnyAuth {
    AnyAuth(crate::api::auth::AuthContext::Product(
        product_auth(scopes).0,
    ))
}

#[tokio::test]
async fn auth_enforces_health_bootstrap_and_product_tokens() {
    let temp_dir = TempDir::new();
    let store_config = StoreConfig::new(temp_dir.path().join("opendaemon.sqlite3"));
    let product_store = ProductStore::open(store_config.clone()).unwrap();
    let state = AppState::with_directory_grant_store(
        crate::registry::default_providers_dir(),
        RuntimeStore::default(),
        RuntimeDetectionConfig::default(),
        AuthConfig {
            bootstrap_token: Some("bootstrap-secret".to_owned()),
        },
        product_store.clone(),
        crate::store::directory_grants::DirectoryGrantStore::configured(store_config),
    );
    let app = router_with_state(state);

    let health = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);

    let providers = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/providers")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(providers.status(), StatusCode::UNAUTHORIZED);
    let providers_body = to_bytes(providers.into_body(), usize::MAX).await.unwrap();
    let providers_json: Value = serde_json::from_slice(&providers_body).unwrap();
    assert_eq!(providers_json["error"]["code"], "missing_authentication");

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/providers")
                .header("authorization", "Bearer invalid")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::UNAUTHORIZED);

    let create_product = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/products")
                .header("authorization", "Bearer bootstrap-secret")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "id": "product_a",
                        "display_name": "Product A",
                        "description": "test"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_product.status(), StatusCode::CREATED);

    let mint_token = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/products/product_a/tokens")
                .header("authorization", "Bearer bootstrap-secret")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "label": "dev",
                        "scopes": ["providers:read"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mint_token.status(), StatusCode::CREATED);
    let mint_body = to_bytes(mint_token.into_body(), usize::MAX).await.unwrap();
    let mint_json: Value = serde_json::from_slice(&mint_body).unwrap();
    let token = mint_json["token"]["token"].as_str().unwrap();

    let provider_ok = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/providers")
                .header(
                    "authorization",
                    HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
                )
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(provider_ok.status(), StatusCode::OK);

    let bootstrap_setup_read = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/providers")
                .header("authorization", "Bearer bootstrap-secret")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bootstrap_setup_read.status(), StatusCode::OK);

    product_store
        .patch_product(
            "product_a",
            crate::product::PatchProduct {
                status: Some(ProductStatus::Disabled),
                ..Default::default()
            },
        )
        .unwrap();
    let disabled = app
        .oneshot(
            Request::builder()
                .uri("/v1/providers")
                .header(
                    "authorization",
                    HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
                )
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disabled.status(), StatusCode::UNAUTHORIZED);
    let disabled_body = to_bytes(disabled.into_body(), usize::MAX).await.unwrap();
    let disabled_json: Value = serde_json::from_slice(&disabled_body).unwrap();
    assert_eq!(disabled_json["error"]["code"], "product_disabled");
}

fn write_fake_command(dir: &Path, name: &str, body: &str) {
    let path = dir.join(command_file_name(name));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
    }

    #[cfg(windows)]
    {
        fs::write(&path, format!("@echo off\r\n{body}\r\n")).unwrap();
    }
}

fn command_file_name(name: &str) -> String {
    #[cfg(windows)]
    {
        format!("{name}.cmd")
    }

    #[cfg(not(windows))]
    {
        name.to_owned()
    }
}
