//! Render path moved out of `gfx/mod.rs`.

use wgpu::SurfaceError;

// Bring parent gfx modules/types into scope for the moved body.
#[cfg(target_arch = "wasm32")]
use crate::gfx::types::Globals;
use crate::gfx::{camera_sys, terrain, types::Model};
use net_core::snapshot::SnapshotEncode;

/// Full render implementation (moved from gfx/mod.rs).
pub fn render_impl(r: &mut crate::gfx::Renderer) -> Result<(), SurfaceError> {
    let frame = r.surface.get_current_texture()?;
    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    // Optional tracing left to RA_TRACE (no default info spam)

    // WASM debug path: draw SKY + TERRAIN into offscreen, then present to swapchain.
    // This isolates pipeline/render-graph step-by-step. Disabled by default now that
    // the full render path is stable on web; re-enable locally by changing the
    // `enable_wasm_debug` flag below if you need to bisect a regression.
    #[cfg(target_arch = "wasm32")]
    if false {
        let mut encoder = r
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("wasm-debug-sky"),
            });
        // Update a minimal Globals UBO so terrain renders with a sane camera
        let aspect = r.config.width.max(1) as f32 / r.config.height.max(1) as f32;
        let eye = glam::vec3(0.0, 6.0, -10.0);
        let look = glam::vec3(0.0, 0.5, 0.0);
        let up = glam::Vec3::Y;
        let view_m = glam::Mat4::look_at_rh(eye, look, up);
        let fov_y = 60f32.to_radians();
        let proj = glam::Mat4::perspective_rh(fov_y, aspect, 0.1, 1000.0);
        let vp = proj * view_m;
        let mut g = Globals {
            view_proj: vp.to_cols_array_2d(),
            cam_right_time: [1.0, 0.0, 0.0, 0.0],
            cam_up_pad: [0.0, 1.0, 0.0, (fov_y * 0.5).tan()],
            sun_dir_time: [
                r.sky.sun_dir.x,
                r.sky.sun_dir.y,
                r.sky.sun_dir.z,
                r.sky.day_frac,
            ],
            sh_coeffs: [[0.0; 4]; 9],
            fog_params: [0.6, 0.7, 0.8, 0.0035],
            clip_params: [0.1, 1000.0, 1.0, 0.0],
        };
        if r.sky.sun_dir.y <= 0.0 {
            g.fog_params = [0.01, 0.015, 0.02, 0.018];
        }
        r.queue
            .write_buffer(&r.globals_buf, 0, bytemuck::bytes_of(&g));

        // 1) Sky into offscreen
        let mut sky = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("wasm-debug-sky-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &r.attachments.scene_view,
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
        sky.set_pipeline(&r.sky_pipeline);
        sky.set_bind_group(0, &r.globals_bg, &[]);
        sky.set_bind_group(1, &r.sky_bg, &[]);
        sky.draw(0..3, 0..1);
        drop(sky);
        // 2) Main terrain into offscreen with depth
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("wasm-debug-main-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &r.attachments.scene_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &r.attachments.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            rp.set_pipeline(&r.pipeline);
            rp.set_bind_group(0, &r.globals_bg, &[]);
            rp.set_bind_group(1, &r.terrain_model_bg, &[]);
            rp.set_vertex_buffer(0, r.terrain_vb.slice(..));
            rp.set_index_buffer(r.terrain_ib.slice(..), wgpu::IndexFormat::Uint16);
            rp.draw_indexed(0..r.terrain_index_count, 0, 0..1);
            drop(rp);
        }
        // 3) Present offscreen -> swapchain
        let mut present = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("wasm-debug-present-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
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
        present.set_pipeline(&r.present_pipeline);
        present.set_bind_group(0, &r.globals_bg, &[]);
        present.set_bind_group(1, &r.present_bg, &[]);
        present.draw(0..3, 0..1);
        drop(present);
        r.queue.submit(Some(encoder.finish()));
        frame.present();
        return Ok(());
    }

    // Time and dt
    let t = r.start.elapsed().as_secs_f32();
    let aspect = r.config.width as f32 / r.config.height as f32;
    let dt = (t - r.last_time).max(0.0);
    r.last_time = t;
    // Replication: drain any incoming deltas and upload chunk meshes (local loop)
    if let Some(rx) = &r.repl_rx {
        let msgs = rx.drain();
        if !msgs.is_empty() {
            for b in &msgs {
                let _ = r.repl_buf.apply_message(b);
            }
            let updates = r.repl_buf.drain_mesh_updates();
            use client_core::upload::MeshUpload;
            for (did, chunk, entry) in updates {
                r.upload_chunk_mesh(did, chunk, &entry);
            }
        }
    }
    // Reset per-frame stats
    r.draw_calls = 0;

    // If screenshot mode is active, auto-animate a smooth orbit for 5 seconds
    if let Some(ts) = r.screenshot_start {
        let elapsed = (t - ts).max(0.0);
        if elapsed <= 5.0 {
            let speed = 0.6; // rad/s
            r.cam_orbit_yaw = crate::gfx::Renderer::wrap_angle(r.cam_orbit_yaw + dt * speed);
        } else {
            r.screenshot_start = None;
        }
    }

    // Update player transform (controls + collision) via external scene inputs
    {
        let cam_fwd = r.cam_follow.current_look - r.cam_follow.current_pos;
        r.scene_inputs.apply_input(&r.input);
        r.scene_inputs.update(dt, cam_fwd, r.static_index.as_ref());
        r.player.pos = r.scene_inputs.pos();
        r.player.yaw = r.scene_inputs.yaw();
        r.apply_pc_transform();
    }
    // Simple AI (legacy/demo only): rotate non-PC wizards
    #[cfg(feature = "legacy_client_ai")]
    {
        r.update_wizard_ai(dt);
    }
    // Compute local orbit offsets (relative to PC orientation)
    let near_d = 1.6f32;
    let far_d = 25.0f32;
    let zoom_t = ((r.cam_distance - near_d) / (far_d - near_d)).clamp(0.0, 1.0);
    let near_lift = 0.5f32; // meters above anchor when fully zoomed-in
    let near_look = 0.5f32; // aim point above anchor when fully zoomed-in
    let eff_lift = near_lift * (1.0 - zoom_t) + r.cam_lift * zoom_t;
    let eff_look = near_look * (1.0 - zoom_t) + r.cam_look_height * zoom_t;
    let (off_local, look_local) = camera_sys::compute_local_orbit_offsets(
        r.cam_distance,
        r.cam_orbit_yaw,
        r.cam_orbit_pitch,
        eff_lift,
        eff_look,
    );
    #[allow(unused_assignments)]
    let pc_anchor = if r.pc_alive {
        if r.pc_index < r.wizard_models.len() {
            let m = r.wizard_models[r.pc_index];
            (m * glam::Vec4::new(0.0, 1.2, 0.0, 1.0)).truncate()
        } else {
            r.player.pos + glam::vec3(0.0, 1.2, 0.0)
        }
    } else {
        r.player.pos + glam::vec3(0.0, 1.2, 0.0)
    };

    // While RMB is held, snap follow (no lag); otherwise use smoothed dt
    let follow_dt = if r.rmb_down { 1.0 } else { dt };
    let _ = camera_sys::third_person_follow(
        &mut r.cam_follow,
        pc_anchor,
        glam::Quat::IDENTITY,
        off_local,
        look_local,
        aspect,
        follow_dt,
    );
    // Keep camera above terrain: clamp eye/target Y to terrain height + clearance
    let clearance_eye = 0.2f32;
    let clearance_look = 0.05f32;
    let eye = r.cam_follow.current_pos;
    let (hy, _n) = terrain::height_at(&r.terrain_cpu, eye.x, eye.z);
    if r.cam_follow.current_pos.y < hy + clearance_eye {
        r.cam_follow.current_pos.y = hy + clearance_eye;
    }
    let look = r.cam_follow.current_look;
    let (hy2, _n2) = terrain::height_at(&r.terrain_cpu, look.x, look.z);
    if r.cam_follow.current_look.y < hy2 + clearance_look {
        r.cam_follow.current_look.y = hy2 + clearance_look;
    }
    // Recompute camera/globals without smoothing after clamping
    let (_cam2, mut globals) = camera_sys::third_person_follow(
        &mut r.cam_follow,
        pc_anchor,
        glam::Quat::IDENTITY,
        off_local,
        look_local,
        aspect,
        0.0,
    );
    // Advance sky & lighting
    r.sky.update(dt);
    globals.sun_dir_time = [
        r.sky.sun_dir.x,
        r.sky.sun_dir.y,
        r.sky.sun_dir.z,
        r.sky.day_frac,
    ];
    for i in 0..9 {
        globals.sh_coeffs[i] = [
            r.sky.sh9_rgb[i][0],
            r.sky.sh9_rgb[i][1],
            r.sky.sh9_rgb[i][2],
            0.0,
        ];
    }
    if r.sky.sun_dir.y <= 0.0 {
        globals.fog_params = [0.01, 0.015, 0.02, 0.018];
    } else {
        globals.fog_params = [0.6, 0.7, 0.8, 0.0035];
    }
    r.queue
        .write_buffer(&r.globals_buf, 0, bytemuck::bytes_of(&globals));
    r.queue
        .write_buffer(&r.sky_buf, 0, bytemuck::bytes_of(&r.sky.sky_uniform));

    // Keep model base identity to avoid moving instances globally
    let shard_mtx = glam::Mat4::IDENTITY;
    let shard_model = Model {
        model: shard_mtx.to_cols_array_2d(),
        color: [0.85, 0.15, 0.15],
        emissive: 0.05,
        _pad: [0.0; 4],
    };
    r.queue
        .write_buffer(&r.shard_model_buf, 0, bytemuck::bytes_of(&shard_model));

    // Handle queued PC cast and update animation state
    r.process_pc_cast(t);
    // Update wizard skinning palettes on CPU then upload
    r.update_wizard_palettes(t);
    // Update PC (UBC) palette if separate rig is active
    r.update_pc_palette(t);
    // Zombie AI/movement on server; then update local transforms and palettes
    {
        let mut wiz_pos: Vec<glam::Vec3> = Vec::with_capacity(r.wizard_count as usize);
        for (i, m) in r.wizard_models.iter().enumerate() {
            if !r.pc_alive && i == r.pc_index {
                wiz_pos.push(glam::vec3(1.0e6, 0.0, 1.0e6));
            } else {
                let c = m.to_cols_array();
                wiz_pos.push(glam::vec3(c[12], c[13], c[14]));
            }
        }
        #[cfg(feature = "legacy_client_ai")]
        for (widx, dmg) in r.server.step_npc_ai(dt, &wiz_pos) {
            if let Some(hp) = r.wizard_hp.get_mut(widx) {
                let before = *hp;
                *hp = (*hp - dmg).max(0);
                let fatal = *hp == 0;
                log::debug!(
                    "wizard melee hit: idx={} hp {} -> {} (dmg {}), fatal={}",
                    widx,
                    before,
                    *hp,
                    dmg,
                    fatal
                );
                if widx < r.wizard_models.len() {
                    let head = r.wizard_models[widx] * glam::Vec4::new(0.0, 1.7, 0.0, 1.0);
                    r.damage.spawn(head.truncate(), dmg);
                }
                if fatal {
                    if widx == r.pc_index {
                        r.kill_pc();
                    } else {
                        r.remove_wizard_at(widx);
                    }
                }
            }
        }
        #[cfg(feature = "legacy_client_ai")]
        {
            r.update_zombies_from_server();
        }
        r.update_zombie_palettes(t);
        #[cfg(feature = "legacy_client_ai")]
        {
            r.update_deathknight_from_server();
        }
        // Move sorceress client-side toward the wizards (slow walk)
        r.update_sorceress_motion(dt);
    }
    // Death Knight palettes (single instance)
    r.update_deathknight_palettes(t);
    // Sorceress palettes (walk/idle based on motion)
    r.update_sorceress_palettes(t);
    // FX update (projectiles/particles)
    r.update_fx(t, dt);
    // Debris cubes update and upload instances
    r.update_debris(dt);
    // Update dynamic lights from active projectiles (up to 16)
    {
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct LightsRaw {
            count: u32,
            _pad: [f32; 3],
            pos_radius: [[f32; 4]; 16],
            color: [[f32; 4]; 16],
        }
        let mut raw = LightsRaw {
            count: 0,
            _pad: [0.0; 3],
            pos_radius: [[0.0; 4]; 16],
            color: [[0.0; 4]; 16],
        };
        let mut n = 0usize;
        let maxr = 10.0f32;
        for p in &r.projectiles {
            if n >= 16 {
                break;
            }
            raw.pos_radius[n] = [p.pos.x, p.pos.y, p.pos.z, maxr];
            // Tint dynamic light by projectile color so different spells light correctly
            // Fire Bolt remains warm; Magic Missile emits a purple light.
            // Slightly reduced intensity for a more subtle look
            let s = 0.9f32;
            raw.color[n] = [p.color[0] * s, p.color[1] * s, p.color[2] * s, 0.0];
            n += 1;
        }
        raw.count = n as u32;
        r.queue
            .write_buffer(&r.lights_buf, 0, bytemuck::bytes_of(&raw));
    }

    // Validate frame-graph invariants for this frame
    {
        let g = super::graph::graph_for(
            r.enable_ssgi,
            r.enable_ssr,
            r.enable_bloom,
            r.direct_present,
        );
        g.validate();
    }

    // Begin commands
    #[cfg(not(target_arch = "wasm32"))]
    r.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut encoder = r
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("encoder"),
        });
    let present_only = std::env::var("RA_PRESENT_ONLY")
        .map(|v| v == "1")
        .unwrap_or(false);
    let render_view: &wgpu::TextureView = if r.direct_present {
        &view
    } else {
        &r.attachments.scene_view
    };
    // Sky-only pass
    log::debug!("pass: sky");
    if !present_only {
        let mut sky = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sky-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: render_view,
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
        sky.set_pipeline(&r.sky_pipeline);
        sky.set_bind_group(0, &r.globals_bg, &[]);
        sky.set_bind_group(1, &r.sky_bg, &[]);
        sky.draw(0..3, 0..1);
        r.draw_calls += 1;
    }
    // Main pass with depth
    log::debug!("pass: main");
    if !present_only {
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("main-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: render_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &r.attachments.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        // Terrain
        let trace = std::env::var("RA_TRACE").map(|v| v == "1").unwrap_or(false);
        if std::env::var("RA_DRAW_TERRAIN")
            .map(|v| v == "0")
            .unwrap_or(false)
        {
            log::debug!("draw: terrain skipped (RA_DRAW_TERRAIN=0)");
        } else {
            log::debug!("draw: terrain");
            if trace {
                #[cfg(not(target_arch = "wasm32"))]
                r.device.push_error_scope(wgpu::ErrorFilter::Validation);
            }
            rp.set_pipeline(&r.pipeline);
            rp.set_bind_group(0, &r.globals_bg, &[]);
            rp.set_bind_group(1, &r.terrain_model_bg, &[]);
            rp.set_vertex_buffer(0, r.terrain_vb.slice(..));
            rp.set_index_buffer(r.terrain_ib.slice(..), wgpu::IndexFormat::Uint16);
            rp.draw_indexed(0..r.terrain_index_count, 0, 0..1);
            r.draw_calls += 1;
            #[cfg(not(target_arch = "wasm32"))]
            if trace && let Some(e) = pollster::block_on(r.device.pop_error_scope()) {
                log::error!("validation after terrain: {:?}", e);
            }
        }
        // Trees
        if r.trees_count > 0 && !r.is_vox_onepath() {
            log::debug!("draw: trees x{}", r.trees_count);
            if trace {
                #[cfg(not(target_arch = "wasm32"))]
                r.device.push_error_scope(wgpu::ErrorFilter::Validation);
            }
            let inst_pipe = if r.wire_enabled {
                r.wire_pipeline.as_ref().unwrap_or(&r.inst_pipeline)
            } else {
                &r.inst_pipeline
            };
            rp.set_pipeline(inst_pipe);
            rp.set_bind_group(0, &r.globals_bg, &[]);
            rp.set_bind_group(1, &r.shard_model_bg, &[]);
            rp.set_vertex_buffer(0, r.trees_vb.slice(..));
            rp.set_vertex_buffer(1, r.trees_instances.slice(..));
            rp.set_index_buffer(r.trees_ib.slice(..), wgpu::IndexFormat::Uint16);
            rp.draw_indexed(0..r.trees_index_count, 0, 0..r.trees_count);
            r.draw_calls += 1;
            #[cfg(not(target_arch = "wasm32"))]
            if trace && let Some(e) = pollster::block_on(r.device.pop_error_scope()) {
                log::error!("validation after trees: {:?}", e);
            }
        }
        // Rocks
        if r.rocks_count > 0 && !r.is_vox_onepath() {
            log::debug!("draw: rocks x{}", r.rocks_count);
            if trace {
                #[cfg(not(target_arch = "wasm32"))]
                r.device.push_error_scope(wgpu::ErrorFilter::Validation);
            }
            let inst_pipe = if r.wire_enabled {
                r.wire_pipeline.as_ref().unwrap_or(&r.inst_pipeline)
            } else {
                &r.inst_pipeline
            };
            rp.set_pipeline(inst_pipe);
            rp.set_bind_group(0, &r.globals_bg, &[]);
            rp.set_bind_group(1, &r.shard_model_bg, &[]);
            rp.set_vertex_buffer(0, r.rocks_vb.slice(..));
            rp.set_vertex_buffer(1, r.rocks_instances.slice(..));
            rp.set_index_buffer(r.rocks_ib.slice(..), wgpu::IndexFormat::Uint16);
            rp.draw_indexed(0..r.rocks_index_count, 0, 0..r.rocks_count);
            r.draw_calls += 1;
            #[cfg(not(target_arch = "wasm32"))]
            if trace && let Some(e) = pollster::block_on(r.device.pop_error_scope()) {
                log::error!("validation after rocks: {:?}", e);
            }
        }
        // Debris cubes (instanced)
        if r.debris_count > 0 {
            let inst_pipe = if r.wire_enabled {
                r.wire_pipeline.as_ref().unwrap_or(&r.inst_pipeline)
            } else {
                &r.inst_pipeline
            };
            rp.set_pipeline(inst_pipe);
            rp.set_bind_group(0, &r.globals_bg, &[]);
            rp.set_bind_group(1, &r.debris_model_bg, &[]);
            rp.set_vertex_buffer(0, r.debris_vb.slice(..));
            rp.set_vertex_buffer(1, r.debris_instances.slice(..));
            rp.set_index_buffer(r.debris_ib.slice(..), wgpu::IndexFormat::Uint16);
            rp.draw_indexed(0..r.debris_index_count, 0, 0..r.debris_count);
            r.draw_calls += 1;
        }
        // Ruins
        if r.ruins_count > 0 && !r.is_vox_onepath() {
            log::debug!("draw: ruins x{}", r.ruins_count);
            if trace {
                r.device.push_error_scope(wgpu::ErrorFilter::Validation);
            }
            let inst_pipe = if r.wire_enabled {
                r.wire_pipeline.as_ref().unwrap_or(&r.inst_pipeline)
            } else {
                &r.inst_pipeline
            };
            rp.set_pipeline(inst_pipe);
            rp.set_bind_group(0, &r.globals_bg, &[]);
            rp.set_bind_group(1, &r.shard_model_bg, &[]);
            rp.set_vertex_buffer(0, r.ruins_vb.slice(..));
            rp.set_vertex_buffer(1, r.ruins_instances.slice(..));
            rp.set_index_buffer(r.ruins_ib.slice(..), wgpu::IndexFormat::Uint16);
            rp.draw_indexed(0..r.ruins_index_count, 0, 0..r.ruins_count);
            r.draw_calls += 1;
            if trace && let Some(e) = pollster::block_on(r.device.pop_error_scope()) {
                log::error!("validation after ruins: {:?}", e);
            }
        }
        // Voxel chunk meshes (if any)
        if !r.voxel_meshes.is_empty() {
            log::debug!("[draw] voxel meshes: {} chunks", r.voxel_meshes.len());
            let trace = std::env::var("RA_TRACE").map(|v| v == "1").unwrap_or(false);
            if trace {
                #[cfg(not(target_arch = "wasm32"))]
                r.device.push_error_scope(wgpu::ErrorFilter::Validation);
            }
            rp.set_pipeline(&r.pipeline);
            rp.set_bind_group(0, &r.globals_bg, &[]);
            rp.set_bind_group(1, &r.voxel_model_bg, &[]);
            for m in r.voxel_meshes.values() {
                rp.set_vertex_buffer(0, m.vb.slice(..));
                rp.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed(0..m.idx, 0, 0..1);
                r.draw_calls += 1;
            }
            #[cfg(not(target_arch = "wasm32"))]
            if trace && let Some(e) = pollster::block_on(r.device.pop_error_scope()) {
                log::error!("validation after voxel meshes: {:?}", e);
            }
        }
        // Skinned: wizards (PC always visible even if hide_wizards)
        if r.is_vox_onepath() {
            // skip wizard visuals entirely in one‑path demo
        } else {
            #[cfg(feature = "legacy_client_carve")]
            if r.destruct_cfg.hide_wizards {
                r.draw_pc_only(&mut rp);
                r.draw_calls += 1;
            } else {
                if std::env::var("RA_DRAW_WIZARDS")
                    .map(|v| v != "0")
                    .unwrap_or(true)
                {
                    log::debug!("draw: wizards x{}", r.wizard_count);
                    r.draw_wizards(&mut rp);
                    r.draw_calls += 1;
                    // If PC uses a separate rig, draw it explicitly in addition to NPC wizards
                    if r.pc_vb.is_some() {
                        r.draw_pc_only(&mut rp);
                        r.draw_calls += 1;
                    }
                } else {
                    log::debug!("draw: wizards skipped (RA_DRAW_WIZARDS=0)");
                }
            }
            #[cfg(not(feature = "legacy_client_carve"))]
            {
                if std::env::var("RA_DRAW_WIZARDS")
                    .map(|v| v != "0")
                    .unwrap_or(true)
                {
                    log::debug!("draw: wizards x{}", r.wizard_count);
                    r.draw_wizards(&mut rp);
                    r.draw_calls += 1;
                    if r.pc_vb.is_some() {
                        r.draw_pc_only(&mut rp);
                        r.draw_calls += 1;
                    }
                } else {
                    log::debug!("draw: wizards skipped (RA_DRAW_WIZARDS=0)");
                }
            }
        }
        // Skinned: Death Knight (boss)
        if r.dk_count > 0 && !r.is_vox_onepath() {
            log::debug!("draw: deathknight x{}", r.dk_count);
            r.draw_deathknight(&mut rp);
            r.draw_calls += 1;
        }
        // Skinned: Sorceress (static idle)
        if r.sorc_count > 0 && !r.is_vox_onepath() {
            log::debug!("draw: sorceress x{}", r.sorc_count);
            r.draw_sorceress(&mut rp);
            r.draw_calls += 1;
        }
        // Skinned: zombies
        if !r.is_vox_onepath()
            && std::env::var("RA_DRAW_ZOMBIES")
                .map(|v| v != "0")
                .unwrap_or(true)
        {
            log::debug!("draw: zombies x{}", r.zombie_count);
            r.draw_zombies(&mut rp);
            r.draw_calls += 1;
        } else {
            log::debug!("draw: zombies skipped (RA_DRAW_ZOMBIES=0)");
        }
        // Particles + projectiles
        r.draw_particles(&mut rp);
        if r.fx_count > 0 {
            r.draw_calls += 1;
        }
        // Copy SceneColor into SceneRead when not direct-present
        if !r.direct_present {
            drop(rp);
            let mut blit = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit-scene-read"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &r.attachments.scene_read_view,
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
            blit.set_pipeline(&r.blit_scene_read_pipeline);
            blit.set_bind_group(0, &r.present_bg, &[]);
            blit.draw(0..3, 0..1);
            r.draw_calls += 1;
        }
    }
    // Overlay: health bars, floating damage numbers, then nameplates
    // Build entries for bars and queue damage/nameplates using the current view-projection.
    let view_proj = glam::Mat4::from_cols_array_2d(&globals.view_proj);
    let overlays_disabled = std::env::var("RA_OVERLAYS")
        .map(|v| v == "0")
        .unwrap_or(false);
    if !overlays_disabled && !r.is_vox_onepath() {
        // Bars for wizards
        let mut bar_entries: Vec<(glam::Vec3, f32)> = Vec::new();
        for (i, m) in r.wizard_models.iter().enumerate() {
            #[cfg(feature = "legacy_client_carve")]
            if r.destruct_cfg.hide_wizards && i != r.pc_index {
                continue;
            }
            let head = *m * glam::Vec4::new(0.0, 1.7, 0.0, 1.0);
            let frac = (r.wizard_hp.get(i).copied().unwrap_or(r.wizard_hp_max) as f32)
                / (r.wizard_hp_max as f32);
            bar_entries.push((head.truncate(), frac));
        }
        // Bars for alive zombies
        // Prefer replicated NPC view if present; fallback to server (legacy)
        {
            use std::collections::HashMap;
            let mut npc_map: HashMap<u32, (i32, i32, bool)> = HashMap::new();
            for n in &r.repl_buf.npcs {
                npc_map.insert(n.id, (n.hp, n.max, n.alive));
            }
            #[cfg(feature = "legacy_client_ai")]
            if npc_map.is_empty() {
                for n in &r.server.npcs {
                    npc_map.insert(n.id.0, (n.hp, n.max_hp, n.alive));
                }
            }
            for (i, id) in r.zombie_ids.iter().enumerate() {
                #[cfg(feature = "legacy_client_carve")]
                if r.destruct_cfg.vox_sandbox {
                    continue;
                }
                if let Some((hp, max_hp, alive)) = npc_map.get(id).copied()
                    && alive
                {
                    let m = r
                        .zombie_models
                        .get(i)
                        .copied()
                        .unwrap_or(glam::Mat4::IDENTITY);
                    let head = m * glam::Vec4::new(0.0, 1.6, 0.0, 1.0);
                    let frac = (hp.max(0) as f32) / (max_hp.max(1) as f32);
                    bar_entries.push((head.truncate(), frac));
                }
            }
        }
        #[cfg(not(feature = "legacy_client_ai"))]
        for _ in &r.zombie_ids {
            #[cfg(feature = "legacy_client_carve")]
            if r.destruct_cfg.vox_sandbox {
                continue;
            }
            // No server data; skip zombie bars in non-legacy mode
        }
        // Death Knight health bar (use server HP; lower vertical offset)
        if r.dk_count > 0
            && {
                #[cfg(feature = "legacy_client_carve")]
                { !r.destruct_cfg.vox_sandbox }
                #[cfg(not(feature = "legacy_client_carve"))]
                { true }
            }
            && let Some(m) = r.dk_models.first().copied()
        {
            let frac = {
                #[cfg(feature = "legacy_client_ai")]
                {
                    if let Some(id) = r.dk_id
                        && let Some(n) = r.server.npcs.iter().find(|n| n.id.0 == id)
                    {
                        (n.hp.max(0) as f32) / (n.max_hp.max(1) as f32)
                    } else {
                        1.0
                    }
                }
                #[cfg(not(feature = "legacy_client_ai"))]
                {
                    1.0
                }
            };
            let head = m * glam::Vec4::new(0.0, 1.6, 0.0, 1.0);
            bar_entries.push((head.truncate(), frac));
        }
        // Queue bars vertices and draw to the active target
        r.bars.queue_entries(
            &r.device,
            &r.queue,
            r.config.width,
            r.config.height,
            view_proj,
            &bar_entries,
        );
        let bars_target = if r.direct_present {
            &view
        } else {
            &r.attachments.scene_view
        };
        r.bars.draw(&mut encoder, bars_target);
    }

    // Damage numbers: update, queue, draw (independent of RA_OVERLAYS to ensure visibility)
    if !r.is_vox_onepath() {
        r.damage.update(dt);
        r.damage.queue(
            &r.device,
            &r.queue,
            r.config.width,
            r.config.height,
            view_proj,
        );
        let damage_target = if r.direct_present {
            &view
        } else {
            &r.attachments.scene_view
        };
        r.damage.draw(&mut encoder, damage_target);
    }

    // Nameplates disabled by default. Set RA_NAMEPLATES=1 to enable.
    let draw_labels = std::env::var("RA_NAMEPLATES")
        .map(|v| v == "1")
        .unwrap_or(false);
    if draw_labels && !r.is_vox_onepath() {
        // Alive wizards only
        let mut wiz_alive: Vec<glam::Mat4> = Vec::new();
        for (i, m) in r.wizard_models.iter().enumerate() {
            if r.wizard_hp.get(i).copied().unwrap_or(0) > 0 {
                wiz_alive.push(*m);
            }
        }
        if !wiz_alive.is_empty() {
            let target_view = if r.direct_present {
                &view
            } else {
                &r.attachments.scene_view
            };
            r.nameplates.queue_labels(
                &r.device,
                &r.queue,
                r.config.width,
                r.config.height,
                view_proj,
                &wiz_alive,
            );
            r.nameplates.draw(&mut encoder, target_view);
        }
        // NPC nameplates: skip dead
        let mut npc_positions: Vec<glam::Vec3> = Vec::new();
        for (idx, m) in r.zombie_models.iter().enumerate() {
            #[cfg(feature = "legacy_client_ai")]
            if let Some(npc) = r.server.npcs.get(idx) && !npc.alive {
                continue;
            }
            let head = *m * glam::Vec4::new(0.0, 1.6, 0.0, 1.0);
            npc_positions.push(head.truncate());
        }
        if !npc_positions.is_empty() {
            let target_view = if r.direct_present {
                &view
            } else {
                &r.attachments.scene_view
            };
            r.nameplates_npc.queue_npc_labels(
                &r.device,
                &r.queue,
                r.config.width,
                r.config.height,
                view_proj,
                &npc_positions,
                "Zombie",
            );
            r.nameplates_npc.draw(&mut encoder, target_view);
        }

        // Death Knight nameplate (single instance)
        if r.dk_count > 0
            && let Some(m) = r.dk_models.first().copied()
        {
            let head = m * glam::Vec4::new(0.0, 1.6, 0.0, 1.0);
            let pos = head.truncate();
            let target_view = if r.direct_present {
                &view
            } else {
                &r.attachments.scene_view
            };
            r.nameplates_npc.queue_npc_labels(
                &r.device,
                &r.queue,
                r.config.width,
                r.config.height,
                view_proj,
                std::slice::from_ref(&pos),
                "Death Knight",
            );
            r.nameplates_npc.draw(&mut encoder, target_view);
        }
    }

    log::debug!("end: main pass");

    if std::env::var("RA_MINIMAL")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        log::debug!("submit: minimal");
        r.queue.submit(Some(encoder.finish()));
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(e) = pollster::block_on(r.device.pop_error_scope()) {
            log::error!("wgpu validation error (minimal mode): {:?}", e);
            return Ok(());
        }
        frame.present();
        return Ok(());
    }
    // Ensure SceneRead is available for bloom pass as well
    if !present_only && r.enable_bloom {
        let mut blit = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("blit-scene-to-read(bloom)"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &r.attachments.scene_read_view,
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
        blit.set_pipeline(&r.blit_scene_read_pipeline);
        blit.set_bind_group(0, &r.present_bg, &[]);
        blit.draw(0..3, 0..1);
    }
    // SSR overlay
    if !present_only && r.enable_ssr {
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ssr-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &r.attachments.scene_view,
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
        rp.set_pipeline(&r.ssr_pipeline);
        rp.set_bind_group(0, &r.ssr_depth_bg, &[]);
        rp.set_bind_group(1, &r.ssr_scene_bg, &[]);
        rp.draw(0..3, 0..1);
        r.draw_calls += 1;
    }
    // SSGI additive overlay
    if !present_only && r.enable_ssgi {
        let mut gi = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ssgi-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &r.attachments.scene_view,
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
        gi.set_pipeline(&r.ssgi_pipeline);
        gi.set_bind_group(0, &r.ssgi_globals_bg, &[]);
        gi.set_bind_group(1, &r.ssgi_depth_bg, &[]);
        gi.set_bind_group(2, &r.ssgi_scene_bg, &[]);
        gi.draw(0..3, 0..1);
        r.draw_calls += 1;
    }
    // Post AO
    if !present_only && r.enable_post_ao {
        let mut post = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("post-ao"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &r.attachments.scene_view,
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
        post.set_pipeline(&r.post_ao_pipeline);
        post.set_bind_group(0, &r.globals_bg, &[]);
        post.set_bind_group(1, &r.post_ao_bg, &[]);
        post.draw(0..3, 0..1);
        r.draw_calls += 1;
    }
    // Bloom
    if r.enable_bloom {
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("bloom-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &r.attachments.scene_view,
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
        rp.set_pipeline(&r.bloom_pipeline);
        rp.set_bind_group(0, &r.bloom_bg, &[]);
        rp.draw(0..3, 0..1);
    }
    // Present pass when using offscreen
    if !r.direct_present {
        log::debug!("pass: present");
        let mut present = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("present-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
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
        present.set_pipeline(&r.present_pipeline);
        present.set_bind_group(0, &r.globals_bg, &[]);
        present.set_bind_group(1, &r.present_bg, &[]);
        present.draw(0..3, 0..1);
        r.draw_calls += 1;
    }

    // Submit
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(e) = pollster::block_on(r.device.pop_error_scope()) {
        log::error!("wgpu validation error (skipping frame): {:?}", e);
        return Ok(());
    }

    log::debug!("submit: normal path");
    // Periodically publish BossStatus into the local replication buffer (simulated channel)
    #[cfg(feature = "legacy_client_ai")]
    if r.last_time >= r.boss_status_next_emit
        && let Some(st) = r.server.nivita_status()
    {
        let msg = net_core::snapshot::BossStatusMsg {
            name: st.name,
            ac: st.ac,
            hp: st.hp,
            max: st.max,
            pos: [st.pos.x, st.pos.y, st.pos.z],
        };
        let mut buf = Vec::new();
        msg.encode(&mut buf);
        let _ = r.repl_buf.apply_message(&buf);
        r.boss_status_next_emit = r.last_time + 1.0;
    }
    // HUD
    let pc_hp = r
        .wizard_hp
        .get(r.pc_index)
        .copied()
        .unwrap_or(r.wizard_hp_max);
    let cast_frac = if let Some(start) = r.pc_anim_start {
        if r.wizard_anim_index[r.pc_index] == 0 {
            let dur = r.pc_cast_time.max(0.0001);
            ((t - start) / dur).clamp(0.0, 1.0)
        } else {
            0.0
        }
    } else {
        0.0
    };
    // Hotbar overlays: per-slot cooldown fractions
    let gcd_frac_fb =
        r.scene_inputs
            .cooldown_frac("wiz.fire_bolt.srd521", r.last_time, r.firebolt_cd_dur);
    let gcd_frac_mm = r.scene_inputs.cooldown_frac(
        "wiz.magic_missile.srd521",
        r.last_time,
        r.magic_missile_cd_dur,
    );
    let gcd_frac_fb2 =
        r.scene_inputs
            .cooldown_frac("wiz.fireball.srd521", r.last_time, r.fireball_cd_dur);
    let cd1 = gcd_frac_fb;
    let cd2 = gcd_frac_mm;
    let cd3 = gcd_frac_fb2;
    let overlays_disabled = std::env::var("RA_OVERLAYS")
        .map(|v| v == "0")
        .unwrap_or(false);
    if !r.pc_alive {
        r.hud.reset();
        r.hud.death_overlay(
            r.size.width,
            r.size.height,
            "You died.",
            "Press R to respawn",
        );
    } else if !overlays_disabled {
        let cast_label = if !r.is_vox_onepath() && cast_frac > 0.0 {
            match r.pc_cast_kind.unwrap_or(super::super::PcCast::FireBolt) {
                super::super::PcCast::FireBolt => Some("Fire Bolt"),
                super::super::PcCast::MagicMissile => Some("Magic Missile"),
                super::super::PcCast::Fireball => Some("Fireball"),
            }
        } else {
            None
        };
        if !r.is_vox_onepath() {
            r.hud.build(
                r.size.width,
                r.size.height,
                pc_hp,
                r.wizard_hp_max,
                cast_frac,
                cd1,
                cd2,
                cd3,
                cast_label,
            );
            // Boss banner (top-center) via replicated cache or server fallback
            let boss_line = if let Some(bs) = r.repl_buf.boss_status.as_ref() {
                Some(format!(
                    "{} — HP {}/{}  AC {}",
                    bs.name, bs.hp, bs.max, bs.ac
                ))
            } else if {
                #[cfg(feature = "legacy_client_ai")]
                {
                    if let Some(st) = r.server.nivita_status() {
                        r.hud.append_center_text(
                            r.size.width,
                            r.size.height,
                            &format!("{} — HP {}/{}  AC {}", st.name, st.hp, st.max, st.ac),
                            20.0,
                            [0.95, 0.98, 1.0, 0.95],
                        );
                        true
                    } else {
                        false
                    }
                }
                #[cfg(not(feature = "legacy_client_ai"))]
                {
                    false
                }
            } {
                None
            } else {
                None
            };
            if let Some(txt) = boss_line {
                r.hud.append_center_text(
                    r.size.width,
                    r.size.height,
                    &txt,
                    20.0,
                    [0.95, 0.98, 1.0, 0.95],
                );
            }
        } else {
            r.hud.reset();
        }
        // Draw a minimal reticle when in mouselook
        // (disabled per request; keeping code for potential re‑enable)
        /*
        if r.controller_state.mode() == ecs_core::components::ControllerMode::Mouselook {
            r.hud.append_reticle(r.size.width, r.size.height);
        }
        */
        if r.hud_model.perf_enabled() {
            let ms = dt * 1000.0;
            let fps = if dt > 1e-5 { 1.0 / dt } else { 0.0 };
            let line0 = format!("{:.2} ms  {:.0} FPS  {} draws", ms, fps, r.draw_calls);
            r.hud
                .append_perf_text_line(r.size.width, r.size.height, &line0, 0);
            // Destructible overlay line
            let vox = format!(
                "vox: queue={} chunks={} skipped={} debris={} | remesh {:.2}ms coll {:.2}ms",
                r.vox_queue_len,
                r.vox_last_chunks,
                r.vox_skipped_last,
                r.vox_debris_last,
                r.vox_remesh_ms_last,
                r.vox_collider_ms_last
            );
            r.hud
                .append_perf_text_line(r.size.width, r.size.height, &vox, 1);
            if let Some((shot, carved, meshed)) = r.vox_onepath_ui {
                let check = |b: bool| if b { '✓' } else { ' ' };
                let demo = format!(
                    "VOX ONEPATH | ray {} | carve {} | mesh {} | debris: {}",
                    check(shot),
                    check(carved),
                    check(meshed),
                    r.debris.len()
                );
                r.hud
                    .append_perf_text_line(r.size.width, r.size.height, &demo, 2);
                let hint = "Space/Enter carve   R reset   S screenshot   P perf";
                r.hud
                    .append_perf_text_line(r.size.width, r.size.height, hint, 3);
            }
            // Boss status line (Nivita) — prefer replicated cache; fallback to server.
            if let Some(bs) = r.repl_buf.boss_status.as_ref() {
                let line = format!("Boss: {}  HP {}/{}  AC {}", bs.name, bs.hp, bs.max, bs.ac);
                r.hud
                    .append_perf_text_line(r.size.width, r.size.height, &line, 4);
            } else {
                #[cfg(feature = "legacy_client_ai")]
                if let Some(st) = r.server.nivita_status() {
                    let line = format!("Boss: {}  HP {}/{}  AC {}", st.name, st.hp, st.max, st.ac);
                    r.hud
                        .append_perf_text_line(r.size.width, r.size.height, &line, 4);
                }
            }
        }
        // Short demo hint for first few seconds
        if !r.is_vox_onepath() && r.last_time <= r.demo_hint_until.unwrap_or(0.0) {
            let hint = "Press F to blast (voxel destructible demo)";
            r.hud.append_perf_text(r.size.width, r.size.height, hint);
        }
    }
    r.hud.queue(&r.device, &r.queue);
    r.hud.draw(&mut encoder, &view);
    r.queue.submit(Some(encoder.finish()));
    frame.present();
    Ok(())
}
