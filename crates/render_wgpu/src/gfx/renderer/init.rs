//! Delegate wrapper for new(). The full constructor body remains in gfx/mod.rs as new_core().

use winit::window::Window;

pub async fn new_renderer(window: &Window) -> anyhow::Result<crate::gfx::Renderer> {
    crate::gfx::Renderer::new_core(window).await
}

