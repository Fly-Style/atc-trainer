//! atc-client — desktop client for ATC training sessions.
//!
//! The crate is split into two layers so the UI can be driven through a
//! command/event pattern that is unit-testable in isolation:
//!
//! * `core` — pure state, command dispatch, side-effect descriptors, and
//!   adapter functions for HTTP and WebSocket I/O. Anything in here is
//!   independent of `iced` and can be exercised from tests without spinning up
//!   a window.
//! * `ui` (binary `atc-client`) — the iced application shell that maps user
//!   input to `AppCommand`s and renders the current `ClientState`.

pub mod core;
pub mod ui;
