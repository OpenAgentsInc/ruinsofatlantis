//! render_wgpu: renderer crate
//!
//! This crate owns the `gfx` module that was previously in the root crate.
//! It also provides lightweight shims for `assets`, `core::data`, `ecs`,
//! `client`, and `server` used by the renderer so it can build independently.

pub mod assets;
pub mod core;
// Re-export shared crates under the same module names the renderer expects.
pub use ecs_core as ecs;
pub use client_core as client;
pub use server_core as server;

// Renderer modules live under `gfx/*` to preserve internal paths.
pub mod gfx;
pub use gfx::*;

// Renderer-specific extensions over server_core.
pub mod server_ext;
