//! core: production game types shared across server, client, and sim.
//!
//! This module tree hosts stable data schemas/loaders, SRD rules helpers,
//! and the combat model (FSM/state). The sim engine consumes these types.

pub mod data;
pub mod rules;
pub mod combat;

