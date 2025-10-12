//! Renderer resize split from gfx/mod.rs for readability.

use winit::dpi::PhysicalSize;

use crate::gfx::{Renderer, gbuffer, hiz, util};

pub fn resize_impl(r: &mut Renderer, new_size: PhysicalSize<u32>) {
    if new_size.width == 0 || new_size.height == 0 {
        return;
    }
    let (w, h) = util::scale_to_max((new_size.width, new_size.height), r.max_dim);
    if (w, h) != (new_size.width, new_size.height) {
        log::debug!(
            "Resized {}x{} exceeds max {}, clamped to {}x{} (aspect kept)",
            new_size.width,
            new_size.height,
            r.max_dim,
            w,
            h
        );
    }
    r.size = PhysicalSize::new(w, h);
    r.config.width = w;
    r.config.height = h;
    r.surface.configure(&r.device, &r.config);
    // Rebuild attachments in one place
    let sc_fmt = r.config.format;
    // Match init: use Rgba8Unorm on wasm for compatibility; keep HDR on native
    #[cfg(target_arch = "wasm32")]
    let offscreen = wgpu::TextureFormat::Rgba8Unorm;
    #[cfg(not(target_arch = "wasm32"))]
    let offscreen = wgpu::TextureFormat::Rgba16Float;
    r.attachments.swapchain_format = sc_fmt;
    r.attachments.offscreen_format = offscreen;
    r.attachments
        .rebuild(&r.device, r.config.width, r.config.height);

    // Rebuild bind groups referencing resized textures via centralized bus
    // Lighting M1 resources
    r.gbuffer = Some(gbuffer::GBuffer::create(
        &r.device,
        r.config.width,
        r.config.height,
    ));
    r.hiz = Some(hiz::HiZPyramid::create(
        &r.device,
        r.config.width,
        r.config.height,
    ));
    // Temporarily move the bus out to avoid borrow conflicts when passing &mut r
    let bus = std::mem::replace(
        &mut r.rebuild_bus,
        crate::gfx::renderer::rebuild_bus::RebuildBus::new(),
    );
    bus.run_all(r);
    r.rebuild_bus = bus;
}
