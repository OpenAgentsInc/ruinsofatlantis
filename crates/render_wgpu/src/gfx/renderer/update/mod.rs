//! Update module split scaffolding.
//!
//! Phase-one: keep legacy monolithic implementation via `legacy` while
//! introducing submodules by concern. No behavior changes; external call
//! sites continue to `use crate::gfx::renderer::update::*`.

pub mod builder;
#[cfg(feature = "demo_destructibles")]
pub mod destructibles_demo;
pub mod math;
pub mod projectiles;

// Back-compat: include the original monolithic module.
#[path = "../update.rs"]
mod legacy;

pub use builder::*;
#[cfg(feature = "demo_destructibles")]
pub use destructibles_demo::*;
pub use legacy::*;
pub use math::*;
pub use projectiles::*;
