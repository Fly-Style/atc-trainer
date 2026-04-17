//! atc-shared — protocol DTOs, domain enums, scenario file types, validation helpers.
//!
//! This crate must not depend on any runtime or UI code. It is consumed by
//! `atc-server`, `atc-client`, and `atc-test-support`.

pub mod ids;
pub mod role;
pub mod session;
pub mod aircraft;
pub mod sector;
pub mod scenario;
pub mod protocol;
pub mod validation;

pub use ids::*;
pub use role::*;
pub use session::*;
pub use aircraft::*;
pub use sector::*;
pub use scenario::*;
pub use protocol::*;
