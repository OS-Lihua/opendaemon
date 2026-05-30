use std::{fs, path::Path};

use axum::{
    body::to_bytes,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::{Value, json};

use crate::{
    api::{
        AppState, HealthResponse, ProviderApiError, health, provider_get, provider_list, router,
        runtime_detect, runtime_list,
    },
    config::{RuntimeDetectionConfig, RuntimeEnvironment},
    runtime::store::RuntimeStore,
    tests::{TempDir, temp_registry_with_provider, valid_manifest_json, write_provider_fixture},
};

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
    let response = provider_list(State(AppState::default())).await.unwrap().0;
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
    let response = provider_get(State(AppState::default()), AxumPath("codex".to_owned()))
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

    let response = runtime_list(State(state)).await.unwrap().0;
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

    let response = runtime_detect(State(state.clone())).await.unwrap().0;
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

    let latest = runtime_list(State(state)).await.unwrap().0;
    let latest_json: Value = serde_json::to_value(latest).unwrap();
    assert_eq!(latest_json["runtimes"][1]["status"], "available");
    assert_eq!(latest_json["runtimes"][1]["version"], "test-provider 2.3.4");
}

fn test_state(
    providers_dir: impl Into<std::path::PathBuf>,
    config: RuntimeDetectionConfig,
) -> AppState {
    AppState::new(providers_dir.into(), RuntimeStore::default(), config)
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
