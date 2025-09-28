//! render_wgpu: renderer crate
//!
//! This crate owns the `gfx` module that was previously in the root crate.
//! It also provides lightweight shims for `assets`, `core::data`, `ecs`,
//! `client`, and `server` used by the renderer so it can build independently.

// Use shared crates directly under their own names.
pub use client_core;
pub use ecs_core;
pub use server_core;

// Renderer modules live under `gfx/*` to preserve internal paths.
pub mod gfx;
pub use gfx::*;

// Renderer-specific extensions over server_core.
pub mod server_ext;
