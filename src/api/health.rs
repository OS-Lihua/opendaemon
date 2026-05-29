use axum::Json;
use serde::Serialize;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
    pub version: &'static str,
}

#[must_use]
pub const fn response() -> HealthResponse {
    HealthResponse {
        status: "ok",
        service: "opendaemon",
        version: env!("CARGO_PKG_VERSION"),
    }
}

pub async fn health() -> Json<HealthResponse> {
    Json(response())
}
