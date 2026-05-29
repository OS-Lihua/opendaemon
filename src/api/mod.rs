use axum::{Router, routing::get};

mod health;

pub use health::{HealthResponse, health};

pub fn router() -> Router {
    Router::new().route("/health", get(health))
}
