//! sim_core: rules + combat + deterministic sim runtime.
//!
//! Extracted from the root crate so clients can depend on a headless simulation
//! and SRD rules without pulling in renderer/platform crates.

pub mod combat;
pub mod rules;
pub mod sim;

// Back-compat re-exports so existing imports `ruinsofatlantis::sim::state` work.
pub use crate::sim::state;
pub use crate::sim::systems;
