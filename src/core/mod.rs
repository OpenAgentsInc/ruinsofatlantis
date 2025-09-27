//! core: production game types shared across server, client, and sim.
//!
//! This module tree hosts stable data schemas/loaders, SRD rules helpers,
//! and the combat model (FSM/state). The sim engine consumes these types.

pub use data_runtime as data;
pub use sim_core::combat;
pub use sim_core::rules;
