//! render_wgpu: renderer crate
//!
//! This crate owns the `gfx` module that was previously in the root crate.
//! It also provides lightweight shims for `assets`, `core::data`, `ecs`,
//! `client`, and `server` used by the renderer so it can build independently.

pub mod assets;
pub mod core;
pub mod ecs;
pub mod client;
pub mod server;

// Renderer modules live under `gfx/*` to preserve internal paths.
pub mod gfx;
pub use gfx::*;
