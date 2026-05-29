use axum::{body::to_bytes, extract::Path, http::StatusCode, response::IntoResponse};
use serde_json::Value;

use crate::api::{HealthResponse, ProviderApiError, health, provider_get, provider_list, router};

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
    let response = provider_list().await.unwrap().0;
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
    let response = provider_get(Path("codex".to_owned())).await.unwrap().0;
    let json: Value = serde_json::to_value(response).unwrap();

    assert_eq!(json["provider"]["id"], "codex");
    assert_eq!(json["provider"]["manifest"]["id"], "codex");
}

#[tokio::test]
async fn provider_get_handler_returns_stable_404() {
    let error = provider_get(Path("missing-provider".to_owned()))
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
