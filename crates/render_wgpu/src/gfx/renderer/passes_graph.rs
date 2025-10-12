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
            .pass("Main", |ctx: &mut ExecCtx| {
                let dc0 = ctx.renderer.draw_calls;
                let h0 = ctx.renderer.bg_cache.hits;
                let m0 = ctx.renderer.bg_cache.misses;
                let t0 = std::time::Instant::now();
                ctx.renderer.pass_main(ctx.encoder);
                let cpu_ms = t0.elapsed().as_secs_f32() * 1000.0;
                let draws = ctx.renderer.draw_calls.saturating_sub(dc0);
                let stats = crate::gfx::renderer::RenderStats {
                    name: "Main",
                    draws,
                    batches: 0,
                    cpu_ms,
                    bg_hits: ctx.renderer.bg_cache.hits.saturating_sub(h0),
                    bg_misses: ctx.renderer.bg_cache.misses.saturating_sub(m0),
                };
                ctx.renderer.render_stats.push(stats);
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
                let h0 = ctx.renderer.bg_cache.hits;
                let m0 = ctx.renderer.bg_cache.misses;
                let t0 = std::time::Instant::now();
                // Acquire, composite offscreen hdr_color to swapchain, and present
                let frame = match ctx.renderer.surface.get_current_texture() {
                    Ok(f) => f,
                    Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
                        // Surface lost/outdated — reconfigure using current size and attachments
                        log::warn!("present: surface lost/outdated; reconfiguring");
                        let size = ctx.renderer.size; // current logical size tracked by renderer
                        crate::gfx::renderer::resize::resize_impl(ctx.renderer, size);
                        return;
                    }
                    Err(wgpu::SurfaceError::Timeout) => {
                        // Timed out acquiring a frame — skip this frame quietly
                        log::warn!("present: acquire timeout; skipping frame");
                        return;
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        // OOM — log loudly and skip; caller may decide to stop rendering
                        log::error!("present: out of memory while acquiring frame");
                        return;
                    }
                    Err(e) => {
                        // Fallback: log and skip
                        log::error!("present: acquire failed: {:?}", e);
                        return;
                    }
                };
                let swap_view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                ctx.renderer.pass_present(ctx.encoder, &swap_view);
                frame.present();
                let cpu_ms = t0.elapsed().as_secs_f32() * 1000.0;
                let stats = crate::gfx::renderer::RenderStats {
                    name: "Present",
                    draws: 1,
                    batches: 1,
                    cpu_ms,
                    bg_hits: ctx.renderer.bg_cache.hits.saturating_sub(h0),
                    bg_misses: ctx.renderer.bg_cache.misses.saturating_sub(m0),
                };
                ctx.renderer.render_stats.push(stats);
            })
            .reads(color);
    }
}

pub struct ParticlesPass;
impl ParticlesPass {
    pub fn declare(builder: &mut GraphBuilder, color: Handle<Img>, _depth: Handle<Img>) {
        let _ = builder
            .pass("Particles", move |ctx: &mut ExecCtx| {
                let dc0 = ctx.renderer.draw_calls;
                let h0 = ctx.renderer.bg_cache.hits;
                let m0 = ctx.renderer.bg_cache.misses;
                let t0 = std::time::Instant::now();
                if ctx.renderer.fx_count > 0 {
                    let view = ctx.view_color(color).clone();
                    let mut rp = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("particles-pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
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
                let cpu_ms = t0.elapsed().as_secs_f32() * 1000.0;
                let draws = ctx.renderer.draw_calls.saturating_sub(dc0);
                let stats = crate::gfx::renderer::RenderStats {
                    name: "Particles",
                    draws,
                    batches: if draws > 0 { 1 } else { 0 },
                    cpu_ms,
                    bg_hits: ctx.renderer.bg_cache.hits.saturating_sub(h0),
                    bg_misses: ctx.renderer.bg_cache.misses.saturating_sub(m0),
                };
                ctx.renderer.render_stats.push(stats);
            })
            .writes(color);
    }
}

pub struct UiPass;
impl UiPass {
    pub fn declare(builder: &mut GraphBuilder, color: Handle<Img>) {
        let _ = builder
            .pass("UI", move |ctx: &mut ExecCtx| {
                let dc0 = ctx.renderer.draw_calls;
                let h0 = ctx.renderer.bg_cache.hits;
                let m0 = ctx.renderer.bg_cache.misses;
                let t0 = std::time::Instant::now();
                let (device, queue) = (&ctx.renderer.device, &ctx.renderer.queue);
                ctx.renderer.hud.queue(device, queue);
                let view = ctx.view_color(color).clone();
                ctx.renderer.hud.draw(ctx.encoder, &view);
                let cpu_ms = t0.elapsed().as_secs_f32() * 1000.0;
                let draws = ctx.renderer.draw_calls.saturating_sub(dc0);
                let stats = crate::gfx::renderer::RenderStats {
                    name: "UI",
                    draws,
                    batches: if draws > 0 { 1 } else { 0 },
                    cpu_ms,
                    bg_hits: ctx.renderer.bg_cache.hits.saturating_sub(h0),
                    bg_misses: ctx.renderer.bg_cache.misses.saturating_sub(m0),
                };
                ctx.renderer.render_stats.push(stats);
            })
            .writes(color);
    }
}
