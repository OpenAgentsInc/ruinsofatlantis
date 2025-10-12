//! Present pipeline re-exports (mechanical split).
//! Behavior-neutral shim to keep call-sites stable during migration.

pub use crate::gfx::pipeline::{create_blit_pipeline, create_present_bgl, create_present_pipeline};
