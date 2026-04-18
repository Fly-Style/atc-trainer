//! iced UI shell. Layered on top of `core::*`: every interaction is routed
//! through [`AppCommand`], and the runtime turns [`SideEffect`]s into iced
//! `Task`s and a per-session WebSocket subscription.

pub mod app;
pub mod sector_canvas;
pub mod start_screen;
pub mod session_screen;

pub use app::{AtcApp, Message};
