//! Pure client core — state, commands, side-effect descriptors, and the
//! HTTP / WebSocket adapter functions that the iced UI calls into.
//!
//! The split is intentional: the UI layer holds an [`ClientState`] and forwards
//! user input as [`AppCommand`]s through [`process`]. The processor mutates
//! state and returns a list of [`SideEffect`]s describing the I/O the runtime
//! must perform. This keeps the UI thin and the command flow testable without
//! a running window or live server.

pub mod state;
pub mod command;
pub mod http;
pub mod ws;

pub use command::{process, AppCommand, HttpCorrelation, HttpRequestSpec, SideEffect};
pub use state::{AuthState, CachedSession, ClientState, ServerProfile, ViewState};
