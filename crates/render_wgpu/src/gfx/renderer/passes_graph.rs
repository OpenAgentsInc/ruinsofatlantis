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
                let pb0 = ctx.renderer.pipeline_binds_count;
                let bb0 = ctx.renderer.bg_binds_count;
                let vb0 = ctx.renderer.vb_ib_sets_count;
                let h0 = ctx.renderer.bg_cache.hits;
                let m0 = ctx.renderer.bg_cache.misses;
                let t0 = std::time::Instant::now();
                ctx.renderer.pass_main(ctx.encoder);
                let cpu_ms = t0.elapsed().as_secs_f32() * 1000.0;
                let draws = ctx.renderer.draw_calls.saturating_sub(dc0);
                let stats = crate::gfx::renderer::RenderStats {
                    name: "Main",
                    draws,
                    batches: ctx.renderer.main_batch_count_last,
                    cpu_ms,
                    bg_hits: ctx.renderer.bg_cache.hits.saturating_sub(h0),
                    bg_misses: ctx.renderer.bg_cache.misses.saturating_sub(m0),
                    pipeline_binds: ctx.renderer.pipeline_binds_count.saturating_sub(pb0),
                    bg_binds: ctx.renderer.bg_binds_count.saturating_sub(bb0),
                    vb_ib_sets: ctx.renderer.vb_ib_sets_count.saturating_sub(vb0),
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
            .pass("Present", move |ctx: &mut ExecCtx| {
                let h0 = ctx.renderer.bg_cache.hits;
                let m0 = ctx.renderer.bg_cache.misses;
                let t0 = std::time::Instant::now();
                // Acquire, composite offscreen hdr_color to swapchain, and present
                let frame = match ctx.renderer.surface.get_current_texture() {
                    Ok(f) => f,
                    Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
                        // Surface lost/outdated — reconfigure using current size and attachments
                        log::warn!("present: surface lost/outdated; reconfiguring");
                        ctx.renderer.present_recoveries =
                            ctx.renderer.present_recoveries.saturating_add(1);
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
                // Build a present BG from the graph HDR view
                let src_owned = ctx.view_color(color).clone();
                let key = crate::gfx::renderer::bindgroups::BgKey::new(
                    &ctx.renderer.present_bgl,
                    &[
                        &src_owned as *const _ as u64,
                        &ctx.renderer.point_sampler as *const _ as u64,
                        &ctx.renderer.attachments.depth_view as *const _ as u64,
                        &ctx.renderer.point_sampler as *const _ as u64,
                    ],
                );
                let present_bg = ctx.renderer.bg_cache.get_or_create(key, || {
                    ctx.renderer
                        .device
                        .create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("present-bg[graph]"),
                            layout: &ctx.renderer.present_bgl,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(&src_owned),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::Sampler(
                                        &ctx.renderer.point_sampler,
                                    ),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 2,
                                    resource: wgpu::BindingResource::TextureView(
                                        &ctx.renderer.attachments.depth_view,
                                    ),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 3,
                                    resource: wgpu::BindingResource::Sampler(
                                        &ctx.renderer.point_sampler,
                                    ),
                                },
                            ],
                        })
                });
                // Present full-screen draw
                let mut rp = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("present-pass(graph)"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &swap_view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });
                rp.set_pipeline(&ctx.renderer.present_pipeline);
                rp.set_bind_group(0, &present_bg, &[]);
                rp.draw(0..3, 0..1);
                // Defer present until after submission; store the frame on the renderer
                ctx.renderer.set_pending_frame(frame);
                let cpu_ms = t0.elapsed().as_secs_f32() * 1000.0;
                let stats = crate::gfx::renderer::RenderStats {
                    name: "Present",
                    draws: 1,
                    batches: 1,
                    cpu_ms,
                    bg_hits: ctx.renderer.bg_cache.hits.saturating_sub(h0),
                    bg_misses: ctx.renderer.bg_cache.misses.saturating_sub(m0),
                    pipeline_binds: 0,
                    bg_binds: 0,
                    vb_ib_sets: 0,
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
                    pipeline_binds: 0,
                    bg_binds: 0,
                    vb_ib_sets: 0,
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
                    pipeline_binds: 0,
                    bg_binds: 0,
                    vb_ib_sets: 0,
                };
                ctx.renderer.render_stats.push(stats);
            })
            .writes(color);
    }
}

pub struct PostAoPass;
impl PostAoPass {
    pub fn declare(builder: &mut GraphBuilder, color: Handle<Img>, depth: Handle<Img>) {
        let _ = builder
            .pass("PostAO", move |ctx: &mut ExecCtx| {
                let h0 = ctx.renderer.bg_cache.hits;
                let m0 = ctx.renderer.bg_cache.misses;
                let t0 = std::time::Instant::now();
                let target = ctx.view_color(color).clone();
                // Build a depth BG from graph view via BgCache
                let depth_view = ctx.view_depth(depth).clone();
                let key = crate::gfx::renderer::bindgroups::BgKey::new(
                    &ctx.renderer.post_ao_bgl,
                    &[
                        &depth_view as *const _ as u64,
                        &ctx.renderer.point_sampler as *const _ as u64,
                    ],
                );
                let post_ao_bg = ctx.renderer.bg_cache.get_or_create(key, || {
                    ctx.renderer
                        .device
                        .create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("post-ao-bg[graph]"),
                            layout: &ctx.renderer.post_ao_bgl,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(&depth_view),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::Sampler(
                                        &ctx.renderer.point_sampler,
                                    ),
                                },
                            ],
                        })
                });
                let mut rp = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("post-ao-pass(graph)"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target,
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
                rp.set_pipeline(&ctx.renderer.post_ao_pipeline);
                rp.set_bind_group(0, &ctx.renderer.globals_bg, &[]);
                rp.set_bind_group(1, &post_ao_bg, &[]);
                rp.draw(0..3, 0..1);
                let cpu_ms = t0.elapsed().as_secs_f32() * 1000.0;
                let stats = crate::gfx::renderer::RenderStats {
                    name: "PostAO",
                    draws: 1,
                    batches: 1,
                    cpu_ms,
                    bg_hits: ctx.renderer.bg_cache.hits.saturating_sub(h0),
                    bg_misses: ctx.renderer.bg_cache.misses.saturating_sub(m0),
                    pipeline_binds: 0,
                    bg_binds: 0,
                    vb_ib_sets: 0,
                };
                ctx.renderer.render_stats.push(stats);
            })
            .reads(depth)
            .writes(color);
    }
}

pub struct BlitSceneReadPass;
impl BlitSceneReadPass {
    pub fn declare(builder: &mut GraphBuilder, _color: Handle<Img>) {
        let _ = builder.pass("BlitSceneRead", |ctx: &mut ExecCtx| {
            // Copy SceneColor -> SceneRead when SSR/SSGI need it; no IO declared in graph yet
            ctx.renderer.pass_blit_scene_read(ctx.encoder);
            // No stats row; folded into post stats
        });
    }
}

pub struct SsgiPass;
impl SsgiPass {
    pub fn declare(builder: &mut GraphBuilder, color: Handle<Img>, depth: Handle<Img>) {
        let _ = builder
            .pass("SSGI", move |ctx: &mut ExecCtx| {
                let h0 = ctx.renderer.bg_cache.hits;
                let m0 = ctx.renderer.bg_cache.misses;
                let t0 = std::time::Instant::now();
                let target = ctx.view_color(color).clone();
                // Depth BG
                let depth_view = ctx.view_depth(depth).clone();
                let key_d = crate::gfx::renderer::bindgroups::BgKey::new(
                    &ctx.renderer.ssgi_depth_bgl,
                    &[
                        &depth_view as *const _ as u64,
                        &ctx.renderer.point_sampler as *const _ as u64,
                    ],
                );
                let ssgi_depth_bg = ctx.renderer.bg_cache.get_or_create(key_d, || {
                    ctx.renderer
                        .device
                        .create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("ssgi-depth-bg[graph]"),
                            layout: &ctx.renderer.ssgi_depth_bgl,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(&depth_view),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::Sampler(
                                        &ctx.renderer.point_sampler,
                                    ),
                                },
                            ],
                        })
                });
                // Scene BG (sample HDR)
                let scene_view = ctx.view_color(color).clone();
                let key_s = crate::gfx::renderer::bindgroups::BgKey::new(
                    &ctx.renderer.ssgi_scene_bgl,
                    &[
                        &scene_view as *const _ as u64,
                        &ctx.renderer._post_sampler as *const _ as u64,
                    ],
                );
                let ssgi_scene_bg = ctx.renderer.bg_cache.get_or_create(key_s, || {
                    ctx.renderer
                        .device
                        .create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("ssgi-scene-bg[graph]"),
                            layout: &ctx.renderer.ssgi_scene_bgl,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(&scene_view),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::Sampler(
                                        &ctx.renderer._post_sampler,
                                    ),
                                },
                            ],
                        })
                });
                let mut rp = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ssgi-pass(graph)"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target,
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
                rp.set_pipeline(&ctx.renderer.ssgi_pipeline);
                rp.set_bind_group(0, &ctx.renderer.ssgi_globals_bg, &[]);
                rp.set_bind_group(1, &ssgi_depth_bg, &[]);
                rp.set_bind_group(2, &ssgi_scene_bg, &[]);
                rp.draw(0..3, 0..1);
                let cpu_ms = t0.elapsed().as_secs_f32() * 1000.0;
                let stats = crate::gfx::renderer::RenderStats {
                    name: "SSGI",
                    draws: 1,
                    batches: 1,
                    cpu_ms,
                    bg_hits: ctx.renderer.bg_cache.hits.saturating_sub(h0),
                    bg_misses: ctx.renderer.bg_cache.misses.saturating_sub(m0),
                    pipeline_binds: 0,
                    bg_binds: 0,
                    vb_ib_sets: 0,
                };
                ctx.renderer.render_stats.push(stats);
            })
            .reads(depth)
            .writes(color);
    }
}

pub struct SsrPass;
impl SsrPass {
    pub fn declare(builder: &mut GraphBuilder, color: Handle<Img>, depth: Handle<Img>) {
        let _ = builder
            .pass("SSR", move |ctx: &mut ExecCtx| {
                let h0 = ctx.renderer.bg_cache.hits;
                let m0 = ctx.renderer.bg_cache.misses;
                let t0 = std::time::Instant::now();
                let target = ctx.view_color(color).clone();
                let scene_view = ctx.view_color(color).clone();
                let key_s = crate::gfx::renderer::bindgroups::BgKey::new(
                    &ctx.renderer.ssr_scene_bgl,
                    &[
                        &scene_view as *const _ as u64,
                        &ctx.renderer._post_sampler as *const _ as u64,
                    ],
                );
                let ssr_scene_bg = ctx.renderer.bg_cache.get_or_create(key_s, || {
                    ctx.renderer
                        .device
                        .create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("ssr-scene-bg[graph]"),
                            layout: &ctx.renderer.ssr_scene_bgl,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(&scene_view),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::Sampler(
                                        &ctx.renderer._post_sampler,
                                    ),
                                },
                            ],
                        })
                });
                let mut rp = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ssr-pass(graph)"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target,
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
                rp.set_pipeline(&ctx.renderer.ssr_pipeline);
                rp.set_bind_group(0, &ctx.renderer.ssr_depth_bg, &[]);
                rp.set_bind_group(1, &ssr_scene_bg, &[]);
                rp.draw(0..3, 0..1);
                let cpu_ms = t0.elapsed().as_secs_f32() * 1000.0;
                let stats = crate::gfx::renderer::RenderStats {
                    name: "SSR",
                    draws: 1,
                    batches: 1,
                    cpu_ms,
                    bg_hits: ctx.renderer.bg_cache.hits.saturating_sub(h0),
                    bg_misses: ctx.renderer.bg_cache.misses.saturating_sub(m0),
                    pipeline_binds: 0,
                    bg_binds: 0,
                    vb_ib_sets: 0,
                };
                ctx.renderer.render_stats.push(stats);
            })
            .reads(depth)
            .writes(color);
    }
}

pub struct BloomPass;
impl BloomPass {
    pub fn declare(builder: &mut GraphBuilder, color: Handle<Img>) {
        let _ = builder
            .pass("Bloom", move |ctx: &mut ExecCtx| {
                let h0 = ctx.renderer.bg_cache.hits;
                let m0 = ctx.renderer.bg_cache.misses;
                let t0 = std::time::Instant::now();
                let target = ctx.view_color(color).clone();
                let mut rp = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("bloom-pass(graph)"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target,
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
                rp.set_pipeline(&ctx.renderer.bloom_pipeline);
                // Use existing bloom_bg (built at init); safe placeholder until dynamic layout stored
                rp.set_bind_group(0, &ctx.renderer.bloom_bg, &[]);
                rp.draw(0..3, 0..1);
                let cpu_ms = t0.elapsed().as_secs_f32() * 1000.0;
                let stats = crate::gfx::renderer::RenderStats {
                    name: "Bloom",
                    draws: 1,
                    batches: 1,
                    cpu_ms,
                    bg_hits: ctx.renderer.bg_cache.hits.saturating_sub(h0),
                    bg_misses: ctx.renderer.bg_cache.misses.saturating_sub(m0),
                    pipeline_binds: 0,
                    bg_binds: 0,
                    vb_ib_sets: 0,
                };
                ctx.renderer.render_stats.push(stats);
            })
            .writes(color);
    }
}

pub struct ResolvePass;
impl ResolvePass {
    pub fn declare(builder: &mut GraphBuilder, msaa: Handle<Img>, hdr: Handle<Img>) {
        let _ = builder
            .pass("Resolve", move |ctx: &mut ExecCtx| {
                // Resolve MSAA color into single-sample HDR by opening a pass with
                // msaa view as color attachment and hdr as resolve target. No draws needed.
                let msaa_view = ctx.view_color(msaa).clone();
                let hdr_view = ctx.view_color(hdr).clone();
                let _rp = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("resolve-pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &msaa_view,
                        resolve_target: Some(&hdr_view),
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
                // Drop closes pass and triggers resolve
            })
            .reads(msaa)
            .writes(hdr);
    }
}
