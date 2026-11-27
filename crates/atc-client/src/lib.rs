//! Desktop client for ATC training sessions and local Aurora sector inspection.

pub mod aurora;
pub mod core;
pub mod host;
mod sector_camera;
pub mod sector_viewer;
#[cfg(feature = "legacy-iced")]
pub mod ui;
