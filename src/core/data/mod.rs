//! core::data — serde-friendly schemas and loaders for authorable game data.
//!
//! Loaders should read from top-level `data/` and provide stable IDs + provenance.

pub mod ability;
pub mod class;
pub mod ids;
pub mod loader;
pub mod monster;
pub mod scenario;
pub mod spell;
