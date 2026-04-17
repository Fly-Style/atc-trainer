pub use crate::config::ServerConfig;
use crate::auth::TokenStore;
use crate::sessions::SessionRegistry;
use axum::routing::{any, get, post};
use axum::Router;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<ServerConfig>,
    pub registry: Arc<SessionRegistry>,
    pub tokens: Arc<TokenStore>,
}

pub fn build_router(config: ServerConfig) -> (Router, AppState) {
    let state = AppState {
        config: Arc::new(config),
        registry: SessionRegistry::new(),
        tokens: TokenStore::new(),
    };
    let router = Router::new()
        .route("/api/v1/auth/trainer-login", post(crate::http::trainer_login))
        .route("/api/v1/sessions", post(crate::http::create_session))
        .route("/api/v1/sessions/{session_id}", get(crate::http::get_session))
        .route("/api/v1/sessions/{session_id}/student-position", post(crate::http::create_student_position))
        .route("/api/v1/sessions/{session_id}/start", post(crate::http::start_session))
        .route("/api/v1/sessions/join-student", post(crate::http::join_student))
        .route("/api/v1/sector/{sector_id}", get(crate::http::get_sector))
        .route("/api/v1/ws", any(crate::ws::ws_handler))
        .with_state(state.clone());
    (router, state)
}
