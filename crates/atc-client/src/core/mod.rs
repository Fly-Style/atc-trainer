//! Client state, commands, side-effect descriptors, and HTTP and WebSocket adapters.
//!
//! [`process`] applies an [`AppCommand`] to [`ClientState`] and returns
//! [`SideEffect`]s for the caller to execute. This lets tests exercise commands
//! without a window or server.

pub mod state;
pub mod command;
pub mod http;
pub mod ws;

pub use command::{process, AppCommand, HttpCorrelation, HttpRequestSpec, SideEffect};
pub use state::{AuthState, CachedSession, ClientState, ServerProfile, ViewState};
