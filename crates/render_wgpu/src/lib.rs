//! render_wgpu: renderer crate
//!
//! This crate owns the `gfx` module that was previously in the root crate.
//! To ease migration, we also re-export a few root modules under the same
//! names so existing `crate::assets`/`crate::server`/`crate::client` paths in
//! the renderer continue to resolve within this crate.

// Bridge selected root modules so `crate::assets` etc. resolve here.
pub use ruinsofatlantis::{assets, client, core, ecs, server};

// Renderer modules live under `gfx/*` to preserve internal paths.
pub mod gfx;
pub use gfx::*;
