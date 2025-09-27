//! sim: deterministic combat simulation runtime (engine only).
//!
//! Consumes core data/rules/combat types and runs a fixed-tick pipeline over a
//! lightweight ECS. Rendering is out of scope.

pub mod components;
pub mod events;
pub mod rng;
pub mod runner;
pub mod scheduler;
pub mod state;
pub mod systems;
pub mod types;
