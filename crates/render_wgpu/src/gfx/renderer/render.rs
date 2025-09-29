//! Renderer::render extracted from gfx/mod.rs

use wgpu::SurfaceError;

/// Delegate so gfx/mod.rs can just call into here.
pub fn render_impl(r: &mut crate::gfx::Renderer) -> Result<(), SurfaceError> {
    crate::gfx::Renderer::render_core(r)
}
