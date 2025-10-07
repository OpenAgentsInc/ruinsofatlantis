//! render_wgpu: renderer crate
//!
//! This crate owns the `gfx` module that was previously in the root crate.
//! It also provides lightweight shims for `assets`, `core::data`, `ecs`,
//! `client`, and `server` used by the renderer so it can build independently.

// Use shared crates directly under their own names.
pub use client_core;
pub use ecs_core;
#[cfg(feature = "vox_onepath_demo")]
pub use server_core;

// Renderer modules live under `gfx/*` to preserve internal paths.
pub mod gfx;
pub mod prelude {
    pub use crate::gfx::*;
}
pub use gfx::*;

// Renderer-specific extensions over server_core (unused in default build)
#[cfg(feature = "vox_onepath_demo")]
pub mod server_ext;
