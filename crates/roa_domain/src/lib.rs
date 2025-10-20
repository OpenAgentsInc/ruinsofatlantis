//! roa_domain: Portable gameplay domain (ECS components, events, systems).
//!
//! Scope: character controller skeleton + input commands + simple sim time.
//!
//! This crate depends only on bevy_ecs, bevy_time, and bevy_reflect so it can be
//! reused outside the Bevy meta-crate/runtime.

pub mod character;
pub mod input;
pub mod npc;
pub mod sim_time;

pub use character::*;
pub use input::*;
pub use npc::*;
pub use sim_time::*;
