//! Snapshot schema split scaffold.
//!
//! Phase-one: keep the original implementation via `legacy` while
//! introducing domain modules. External API remains unchanged by
//! re-exporting `legacy::*`.

pub mod encode {}
pub mod actors {}
pub mod projectiles {}
pub mod destructibles {}
pub mod hud {}

#[path = "../snapshot_body.rs"]
mod legacy;

pub use legacy::*;
