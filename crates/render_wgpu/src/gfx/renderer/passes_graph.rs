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
    pub fn declare(builder: &mut GraphBuilder, hdr: Handle<Img>, msaa: Option<Handle<Img>>) {
        let _ = builder
            .pass("Sky", move |ctx: &mut ExecCtx| {
                // In Picker mode, just clear the background; do not draw the gradient sky.
                let picker_mode = ctx.renderer.is_picker_batches()
                    || std::env::var("ROA_ZONE")
                        .map(|s| s.is_empty() || s == "<picker>")
                        .unwrap_or(false);
                let h0 = ctx.renderer.bg_cache.hits;
                let m0 = ctx.renderer.bg_cache.misses;
                let t0 = std::time::Instant::now();
                let hdr_view = ctx.view_color(hdr).clone();
                if let Some(msaa_h) = msaa {
                    let msaa_view = ctx.view_color(msaa_h).clone();
                    let mut rp = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("sky-pass(graph)"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &msaa_view,
                            resolve_target: Some(&hdr_view),
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.02,
                                    g: 0.02,
                                    b: 0.04,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        timestamp_writes: None,
                    });
                    if !picker_mode {
                        rp.set_pipeline(&ctx.renderer.sky_pipeline);
                        rp.set_bind_group(0, &ctx.renderer.globals_bg, &[]);
                        rp.set_bind_group(1, &ctx.renderer.sky_bg, &[]);
                        rp.draw(0..3, 0..1);
                    }
                } else {
                    let mut rp = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("sky-pass(graph)"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &hdr_view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.02,
                                    g: 0.02,
                                    b: 0.04,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        timestamp_writes: None,
                    });
                    if !picker_mode {
                        rp.set_pipeline(&ctx.renderer.sky_pipeline);
                        rp.set_bind_group(0, &ctx.renderer.globals_bg, &[]);
                        rp.set_bind_group(1, &ctx.renderer.sky_bg, &[]);
                        rp.draw(0..3, 0..1);
                    }
                }
                ctx.renderer.draw_calls += 1;
                let cpu_ms = t0.elapsed().as_secs_f32() * 1000.0;
                let stats = crate::gfx::renderer::RenderStats {
                    name: "Sky",
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
            .writes(hdr)
            .writes(msaa.unwrap_or(hdr));
    }
}

pub struct MainPass;
impl MainPass {
    pub fn declare(
        builder: &mut GraphBuilder,
        hdr: Handle<Img>,
        depth: Handle<Img>,
        msaa: Option<Handle<Img>>,
    ) {
        let _ = builder
            .pass("Main", move |ctx: &mut ExecCtx| {
                let dc0 = ctx.renderer.draw_calls;
                let pb0 = ctx.renderer.pipeline_binds_count;
                let bb0 = ctx.renderer.bg_binds_count;
                let vb0 = ctx.renderer.vb_ib_sets_count;
                let h0 = ctx.renderer.bg_cache.hits;
                let m0 = ctx.renderer.bg_cache.misses;
                let t0 = std::time::Instant::now();
                let (color_view, resolve_to) = if let Some(msaa_h) = msaa {
                    (
                        ctx.view_color(msaa_h).clone(),
                        Some(ctx.view_color(hdr).clone()),
                    )
                } else {
                    (ctx.view_color(hdr).clone(), None)
                };
                let depth_view = ctx.view_depth(depth).clone();
                // Draw inside the same pass so resolve_target is honored
                let mut rp = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("main-pass(graph)"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &color_view,
                        resolve_target: resolve_to.as_ref(),
                        depth_slice: None,
                        ops: wgpu::Operations {
                            // Preserve Sky background; Sky clears color, Main should Load
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });
                ctx.renderer.main_draw_into(&mut rp);
                drop(rp);
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
            .writes(hdr)
            .writes(msaa.unwrap_or(hdr))
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
                // If a previous frame is still pending (e.g., last frame bailed early),
                // present it now to avoid holding a swapchain image across frames.
                if let Some(prev) = ctx.renderer.take_pending_frame() {
                    prev.present();
                }
                // Acquire, composite offscreen color to swapchain, and present
                let frame = match ctx.renderer.surface.get_current_texture() {
                    Ok(f) => f,
                    Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
                        // Surface lost/outdated — reconfigure using current size and attachments
                        log::warn!("present: surface lost/outdated; reconfiguring");
                        ctx.renderer.present_recoveries =
                            ctx.renderer.present_recoveries.saturating_add(1);
                        // Defer resize until after submit/present; do not reconfigure while a frame/encoder is active
                        let size = ctx.renderer.size;
                        ctx.renderer.deferred_resize = Some(size);
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
                        // Fallback: log and request a deferred resize; skip this frame
                        log::error!("present: acquire failed: {:?}", e);
                        ctx.renderer.deferred_resize = Some(ctx.renderer.size);
                        return;
                    }
                };
                log::debug!("present: acquired frame");
                let swap_view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                // Build a present BG from the graph HDR view
                let use_nodepth = std::env::var("RA_PRESENT_NO_DEPTH")
                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);
                let src_view = ctx.view_color(color);
                let src_owned = ctx.view_color(color).clone();
                let present_bg = if use_nodepth {
                    let key = crate::gfx::renderer::bindgroups::BgKey::new(
                        &ctx.renderer.present_bgl_nodepth,
                        &[
                            src_view as *const _ as u64,
                            &ctx.renderer.point_sampler as *const _ as u64,
                        ],
                    );
                    ctx.renderer.bg_cache.get_or_create(key, || {
                        ctx.renderer
                            .device
                            .create_bind_group(&wgpu::BindGroupDescriptor {
                                label: Some("present-bg-nodepth[graph]"),
                                layout: &ctx.renderer.present_bgl_nodepth,
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
                                ],
                            })
                    })
                } else {
                    let key = crate::gfx::renderer::bindgroups::BgKey::new(
                        &ctx.renderer.present_bgl,
                        &[
                            src_view as *const _ as u64,
                            &ctx.renderer.point_sampler as *const _ as u64,
                            &ctx.renderer.attachments.depth_view as *const _ as u64,
                            &ctx.renderer.point_sampler as *const _ as u64,
                        ],
                    );
                    ctx.renderer.bg_cache.get_or_create(key, || {
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
                    })
                };
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
                if use_nodepth {
                    rp.set_pipeline(&ctx.renderer.present_pipeline_nodepth);
                } else {
                    rp.set_pipeline(&ctx.renderer.present_pipeline);
                }
                // Layout: [globals_bgl, present_bgl]
                rp.set_bind_group(0, &ctx.renderer.globals_bg, &[]);
                rp.set_bind_group(1, &present_bg, &[]);
                rp.draw(0..3, 0..1);
                // Drop RP so HUD can open its own pass targeting the same swapchain view
                drop(rp);
                // Draw HUD directly to the swapchain unless RA_NO_HUD=1
                let no_hud = std::env::var("RA_NO_HUD")
                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);
                if !no_hud {
                    ctx.renderer
                        .hud
                        .queue(&ctx.renderer.device, &ctx.renderer.queue);
                    ctx.renderer.hud.draw(ctx.encoder, &swap_view);
                }
                // Defer present until after submission; store the frame on the renderer
                ctx.renderer.set_pending_frame(frame);
                log::debug!("present: drew fullscreen and set pending frame");
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
    pub fn declare(builder: &mut GraphBuilder, hdr: Handle<Img>, msaa: Option<Handle<Img>>) {
        let _ = builder
            .pass("Particles", move |ctx: &mut ExecCtx| {
                // Allow disabling particles entirely for isolation
                let disable_particles = std::env::var("RA_DISABLE_PARTICLES")
                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);
                if disable_particles {
                    return;
                }
                let dc0 = ctx.renderer.draw_calls;
                let h0 = ctx.renderer.bg_cache.hits;
                let m0 = ctx.renderer.bg_cache.misses;
                let t0 = std::time::Instant::now();
                if ctx.renderer.fx_count > 0 {
                    let (view, resolve_to) = if let Some(msaa_h) = msaa {
                        (
                            ctx.view_color(msaa_h).clone(),
                            Some(ctx.view_color(hdr).clone()),
                        )
                    } else {
                        (ctx.view_color(hdr).clone(), None)
                    };
                    let mut rp = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("particles-pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: resolve_to.as_ref(),
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
            .writes(hdr)
            .writes(msaa.unwrap_or(hdr));
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
    pub fn declare(builder: &mut GraphBuilder, post: Handle<Img>, depth: Handle<Img>) {
        let _ = builder
            .pass("PostAO", move |ctx: &mut ExecCtx| {
                let h0 = ctx.renderer.bg_cache.hits;
                let m0 = ctx.renderer.bg_cache.misses;
                let t0 = std::time::Instant::now();
                let target = ctx.view_color(post).clone();
                // Build a depth BG from graph view via BgCache
                let depth_view = ctx.view_depth(depth);
                let key = crate::gfx::renderer::bindgroups::BgKey::new(
                    &ctx.renderer.post_ao_bgl,
                    &[
                        depth_view as *const _ as u64,
                        &ctx.renderer.point_sampler as *const _ as u64,
                    ],
                );
                let depth_owned = ctx.view_depth(depth).clone();
                let post_ao_bg = ctx.renderer.bg_cache.get_or_create(key, || {
                    ctx.renderer
                        .device
                        .create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("post-ao-bg[graph]"),
                            layout: &ctx.renderer.post_ao_bgl,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(&depth_owned),
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
            .writes(post);
    }
}

pub struct BlitHdrToPostPass;
impl BlitHdrToPostPass {
    pub fn declare(builder: &mut GraphBuilder, hdr: Handle<Img>, post: Handle<Img>) {
        let _ = builder
            .pass("BlitHdrToPost", move |ctx: &mut ExecCtx| {
                let src = ctx.view_color(hdr);
                let key = crate::gfx::renderer::bindgroups::BgKey::new(
                    &ctx.renderer.present_bgl,
                    &[
                        src as *const _ as u64,
                        &ctx.renderer.point_sampler as *const _ as u64,
                        &ctx.renderer.attachments.depth_view as *const _ as u64,
                        &ctx.renderer.point_sampler as *const _ as u64,
                    ],
                );
                let src_owned = ctx.view_color(hdr).clone();
                let blit_bg = ctx.renderer.bg_cache.get_or_create(key, || {
                    ctx.renderer
                        .device
                        .create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("blit-hdr-to-post[graph]"),
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
                let target = ctx.view_color(post).clone();
                let mut rp = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("blit-hdr-to-post-pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });
                rp.set_pipeline(&ctx.renderer.blit_scene_read_pipeline);
                rp.set_bind_group(0, &blit_bg, &[]);
                rp.draw(0..3, 0..1);
            })
            .reads(hdr)
            .writes(post);
    }
}

pub struct BlitPostToSrcPass;
impl BlitPostToSrcPass {
    pub fn declare(builder: &mut GraphBuilder, src: Handle<Img>, dst: Handle<Img>) {
        let _ = builder
            .pass("BlitPostToSrc", move |ctx: &mut ExecCtx| {
                let src_ref = ctx.view_color(src);
                let src_owned = src_ref.clone();
                let key = crate::gfx::renderer::bindgroups::BgKey::new(
                    &ctx.renderer.present_bgl,
                    &[
                        src_ref as *const _ as u64,
                        &ctx.renderer.point_sampler as *const _ as u64,
                        &ctx.renderer.attachments.depth_view as *const _ as u64,
                        &ctx.renderer.point_sampler as *const _ as u64,
                    ],
                );
                let bg = ctx.renderer.bg_cache.get_or_create(key, || {
                    ctx.renderer
                        .device
                        .create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("blit-post-to-src[graph]"),
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
                let dst_view = ctx.view_color(dst).clone();
                let mut rp = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("blit-post-to-src-pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &dst_view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });
                rp.set_pipeline(&ctx.renderer.blit_scene_read_pipeline);
                rp.set_bind_group(0, &bg, &[]);
                rp.draw(0..3, 0..1);
            })
            .reads(src)
            .writes(dst);
    }
}

pub struct HistoryCopyPass;
impl HistoryCopyPass {
    pub fn declare(builder: &mut GraphBuilder, src: Handle<Img>) {
        let _ = builder
            .pass("HistoryCopy", move |ctx: &mut ExecCtx| {
                let src_ref = ctx.view_color(src);
                let src_owned = src_ref.clone();
                let key = crate::gfx::renderer::bindgroups::BgKey::new(
                    &ctx.renderer.present_bgl,
                    &[
                        src_ref as *const _ as u64,
                        &ctx.renderer.point_sampler as *const _ as u64,
                        &ctx.renderer.attachments.depth_view as *const _ as u64,
                        &ctx.renderer.point_sampler as *const _ as u64,
                    ],
                );
                let bg = ctx.renderer.bg_cache.get_or_create(key, || {
                    ctx.renderer
                        .device
                        .create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("history-copy-bg"),
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
                let mut rp = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("history-copy-pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &ctx.renderer.attachments.history_view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });
                rp.set_pipeline(&ctx.renderer.blit_scene_read_pipeline);
                rp.set_bind_group(0, &bg, &[]);
                rp.draw(0..3, 0..1);
            })
            .reads(src);
    }
}

pub struct SsgiPass;
impl SsgiPass {
    pub fn declare(
        builder: &mut GraphBuilder,
        post: Handle<Img>,
        hdr: Handle<Img>,
        depth: Handle<Img>,
    ) {
        let _ = builder
            .pass("SSGI", move |ctx: &mut ExecCtx| {
                let h0 = ctx.renderer.bg_cache.hits;
                let m0 = ctx.renderer.bg_cache.misses;
                let t0 = std::time::Instant::now();
                let target = ctx.view_color(post).clone();
                // Depth BG
                let depth_view = ctx.view_depth(depth);
                let key_d = crate::gfx::renderer::bindgroups::BgKey::new(
                    &ctx.renderer.ssgi_depth_bgl,
                    &[
                        depth_view as *const _ as u64,
                        &ctx.renderer.point_sampler as *const _ as u64,
                    ],
                );
                let depth_owned = ctx.view_depth(depth).clone();
                let ssgi_depth_bg = ctx.renderer.bg_cache.get_or_create(key_d, || {
                    ctx.renderer
                        .device
                        .create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("ssgi-depth-bg[graph]"),
                            layout: &ctx.renderer.ssgi_depth_bgl,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(&depth_owned),
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
                let scene_view = ctx.view_color(hdr);
                let key_s = crate::gfx::renderer::bindgroups::BgKey::new(
                    &ctx.renderer.ssgi_scene_bgl,
                    &[
                        scene_view as *const _ as u64,
                        &ctx.renderer._post_sampler as *const _ as u64,
                    ],
                );
                let hdr_owned = ctx.view_color(hdr).clone();
                let ssgi_scene_bg = ctx.renderer.bg_cache.get_or_create(key_s, || {
                    ctx.renderer
                        .device
                        .create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("ssgi-scene-bg[graph]"),
                            layout: &ctx.renderer.ssgi_scene_bgl,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(&hdr_owned),
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
            .reads(hdr)
            .reads(depth)
            .writes(post);
    }
}

pub struct SsrPass;
impl SsrPass {
    pub fn declare(
        builder: &mut GraphBuilder,
        post: Handle<Img>,
        hdr: Handle<Img>,
        depth: Handle<Img>,
    ) {
        let _ = builder
            .pass("SSR", move |ctx: &mut ExecCtx| {
                let h0 = ctx.renderer.bg_cache.hits;
                let m0 = ctx.renderer.bg_cache.misses;
                let t0 = std::time::Instant::now();
                let target = ctx.view_color(post).clone();
                let scene_view = ctx.view_color(hdr);
                let key_s = crate::gfx::renderer::bindgroups::BgKey::new(
                    &ctx.renderer.ssr_scene_bgl,
                    &[
                        scene_view as *const _ as u64,
                        &ctx.renderer._post_sampler as *const _ as u64,
                    ],
                );
                let hdr_owned = ctx.view_color(hdr).clone();
                let ssr_scene_bg = ctx.renderer.bg_cache.get_or_create(key_s, || {
                    ctx.renderer
                        .device
                        .create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("ssr-scene-bg[graph]"),
                            layout: &ctx.renderer.ssr_scene_bgl,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(&hdr_owned),
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
                // Depth BG (graph-owned)
                let depth_view = ctx.view_depth(depth);
                let key_d = crate::gfx::renderer::bindgroups::BgKey::new(
                    &ctx.renderer.ssr_depth_bgl,
                    &[
                        depth_view as *const _ as u64,
                        &ctx.renderer.point_sampler as *const _ as u64,
                    ],
                );
                let depth_owned = ctx.view_depth(depth).clone();
                let ssr_depth_bg = ctx.renderer.bg_cache.get_or_create(key_d, || {
                    ctx.renderer
                        .device
                        .create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("ssr-depth-bg[graph]"),
                            layout: &ctx.renderer.ssr_depth_bgl,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(&depth_owned),
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
                rp.set_bind_group(0, &ssr_depth_bg, &[]);
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
            .reads(hdr)
            .reads(depth)
            .writes(post);
    }
}

pub struct BloomPass;
impl BloomPass {
    pub fn declare(builder: &mut GraphBuilder, post: Handle<Img>, src: Handle<Img>) {
        let _ = builder
            .pass("Bloom", move |ctx: &mut ExecCtx| {
                let h0 = ctx.renderer.bg_cache.hits;
                let m0 = ctx.renderer.bg_cache.misses;
                let t0 = std::time::Instant::now();
                // Build bloom BG before opening the pass to avoid borrow overlap
                let src_ptr = ctx.view_color(src) as *const _ as u64;
                let key = crate::gfx::renderer::bindgroups::BgKey::new(
                    &ctx.renderer.bloom_bgl,
                    &[src_ptr, &ctx.renderer._post_sampler as *const _ as u64],
                );
                let src_owned = ctx.view_color(src).clone();
                let bloom_bg = ctx.renderer.bg_cache.get_or_create(key, || {
                    ctx.renderer
                        .device
                        .create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("bloom-bg[graph]"),
                            layout: &ctx.renderer.bloom_bgl,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(&src_owned),
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
                let target = ctx.view_color(post).clone();
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
                rp.set_bind_group(0, &bloom_bg, &[]);
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
            .reads(src)
            .writes(post);
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
