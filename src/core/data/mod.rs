//! core::data — serde-friendly schemas and loaders for authorable game data.
//!
//! Loaders should read from top-level `data/` and provide stable IDs + provenance.

pub mod ids;
pub mod ability;
pub mod spell;
pub mod loader;
pub mod scenario;
