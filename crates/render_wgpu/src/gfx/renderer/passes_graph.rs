//! Framegraph pass declarations.
//!
//! Declares Sky, Main, Particles, UI, and Present passes against the
//! GraphBuilder API. Execution closures are small and branch‑free; all
//! toggles and UI string formatting are handled upstream so passes
//! only record GPU work.

#![allow(dead_code)]

use super::graph::{ExecCtx, GraphBuilder, Handle, Img};

pub struct SkyPass;
impl SkyPass {
    pub fn declare(builder: &mut GraphBuilder, color: Handle<Img>) {
        let _ = builder
            .pass("Sky", |_ctx: &mut ExecCtx| {
                // Execution remains in legacy path for Sky (PR16 moves to offscreen elsewhere)
            })
            .writes(color);
    }
}

pub struct MainPass;
impl MainPass {
    pub fn declare(builder: &mut GraphBuilder, color: Handle<Img>, depth: Handle<Img>) {
        let _ = builder
            .pass("Main", |_ctx: &mut ExecCtx| {
                // Execution remains in legacy path for Main (PR16 moves to offscreen elsewhere)
            })
            .writes(color)
            .writes(depth);
    }
}

pub struct PresentPass;
impl PresentPass {
    pub fn declare(builder: &mut GraphBuilder, color: Handle<Img>) {
        let _ = builder
            .pass("Present", |ctx: &mut ExecCtx| {
                // Acquire, composite offscreen hdr_color to swapchain, and present
                let frame = match ctx.renderer.surface.get_current_texture() {
                    Ok(f) => f,
                    Err(e) => {
                        log::error!("present: acquire failed: {:?}", e);
                        return;
                    }
                };
                let swap_view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                ctx.renderer.pass_present(ctx.encoder, &swap_view);
                frame.present();
            })
            .reads(color);
    }
}

pub struct ParticlesPass;
impl ParticlesPass {
    pub fn declare(builder: &mut GraphBuilder, color: Handle<Img>, depth: Handle<Img>) {
        let _ = builder
            .pass("Particles", |ctx: &mut ExecCtx| {
                if ctx.renderer.fx_count > 0 {
                    // Draw particles over offscreen scene color
                    let mut rp = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("particles-pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &ctx.renderer.attachments.scene_view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        timestamp_writes: None,
                    });
                    ctx.renderer.draw_particles(&mut rp);
                }
            })
            .writes(color)
            .writes(depth);
    }
}

pub struct UiPass;
impl UiPass {
    pub fn declare(builder: &mut GraphBuilder, color: Handle<Img>) {
        let _ = builder
            .pass("UI", |ctx: &mut ExecCtx| {
                // Queue/upload any UI buffers, then draw HUD to offscreen
                let (device, queue) = (&ctx.renderer.device, &ctx.renderer.queue);
                ctx.renderer.hud.queue(device, queue);
                ctx.renderer
                    .hud
                    .draw(ctx.encoder, &ctx.renderer.attachments.scene_view);
            })
            .writes(color);
    }
}
