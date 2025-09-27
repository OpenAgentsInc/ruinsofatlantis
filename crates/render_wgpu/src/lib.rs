//! render_wgpu: renderer crate (facade)
//!
//! Temporary facade re-exporting the root crate's `gfx` module so we can
//! introduce the crate boundary without breaking existing imports.

pub use ruinsofatlantis::gfx::*;
