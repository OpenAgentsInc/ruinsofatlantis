//! Update module split scaffolding.
//!
//! Phase-one: keep legacy monolithic implementation via `legacy` while
//! introducing submodules by concern. No behavior changes; external call
//! sites continue to `use crate::gfx::renderer::update::*`.

pub mod builder;
#[cfg(feature = "vox_onepath_demo")]
pub mod destructibles_demo;
pub mod math;
pub mod projectiles;

// Back-compat: legacy monolithic implementation moved to `legacy.rs` (git mv).
pub mod legacy;

#[allow(unused_imports)]
pub use builder::*;
#[cfg(feature = "vox_onepath_demo")]
#[allow(unused_imports)]
pub use destructibles_demo::*;
#[allow(unused_imports)]
pub use legacy::*;
#[allow(unused_imports)]
pub use math::*;
#[allow(unused_imports)]
pub use projectiles::*;
