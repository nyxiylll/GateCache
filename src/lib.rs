mod health_check;
mod metrics;
mod chat;
pub mod state;
pub mod embedding;
pub mod security;
pub mod rate_limit;

use axum::{Router, routing::{get, post}};
use std::sync::Arc;
use state::AppState;

pub fn app() -> Router {
    let state = Arc::new(AppState::new());

    Router::new()
        .route("/health_check", get(health_check::heath_check::health_check))
        .route("/metrics", get(metrics::metrics::metrics))
        .route("/chat", post(chat::chat::chat))
        .with_state(state)
}
