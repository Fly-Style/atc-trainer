//! atc-server — HTTP + WebSocket runtime for ATC Training sessions.

pub mod app;
pub mod auth;
pub mod config;
pub mod error;
pub mod http;
pub mod logging;
pub mod scenario;
pub mod sessions;
pub mod ws;

pub use app::{build_router, AppState, ServerConfig};
