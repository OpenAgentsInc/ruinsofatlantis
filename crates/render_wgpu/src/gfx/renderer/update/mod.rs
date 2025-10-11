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

// Back-compat: include the original monolithic module.
#[path = "../update.rs"]
mod legacy;

#[allow(unused_imports)]
pub(crate) use builder::*;
#[cfg(feature = "vox_onepath_demo")]
#[allow(unused_imports)]
pub(crate) use destructibles_demo::*;
#[allow(unused_imports)]
pub(crate) use legacy::*;
#[allow(unused_imports)]
pub(crate) use math::*;
#[allow(unused_imports)]
pub(crate) use projectiles::*;
