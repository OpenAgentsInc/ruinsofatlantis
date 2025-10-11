//! Render path moved out of `gfx/mod.rs`.

use wgpu::SurfaceError;
// Needed for create_buffer_init in default-build replication visuals
use wgpu::util::DeviceExt;

// Bring parent gfx modules/types into scope for the moved body.
#[cfg(target_arch = "wasm32")]
use crate::gfx::types::Globals;
use crate::gfx::{camera_sys, terrain, types::Model};
// legacy client AI paths removed; renderer is replication-driven only

/// Full render implementation (moved from gfx/mod.rs).
#[allow(unused_variables)]
pub fn render_impl(
    r: &mut crate::gfx::Renderer,
    window: Option<&winit::window::Window>,
) -> Result<(), SurfaceError> {
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
        // PC-isolation draw removed from debug block to keep wasm build compiling cleanly.
        sky.set_pipeline(&r.sky_pipeline);
        sky.set_bind_group(0, &r.globals_bg, &[]);
        sky.set_bind_group(1, &r.sky_bg, &[]);
        sky.draw(0..3, 0..1);
        drop(sky);
        // 2) Main terrain into offscreen with depth
        {
            let pc_debug = std::env::var("RA_PC_DEBUG")
                .map(|v| v == "1")
                .unwrap_or(false);
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
                    load: wgpu::LoadOp::Load,
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
        metrics::gauge!("replication.queue_depth").set(rx.depth() as f64);
        let msgs = rx.drain();
        if !msgs.is_empty() {
            let mut total = 0usize;
            for b in &msgs {
                total += b.len();
                let _ = r.repl_buf.apply_message(b);
            }
            metrics::counter!("net.bytes_recv_total", "dir" => "rx").increment(total as u64);
            if std::env::var("RA_LOG_REPL")
                .map(|v| v == "1")
                .unwrap_or(false)
            {
                log::info!(
                    "replication: drained {} msg(s), npcs now {}",
                    msgs.len(),
                    r.repl_buf.npcs.len()
                );
            } else {
                log::debug!(
                    "replication: drained {} msg(s), npcs now {}",
                    msgs.len(),
                    r.repl_buf.npcs.len()
                );
            }
            let updates = r.repl_buf.drain_mesh_updates();
            use client_core::upload::MeshUpload;
            for (did, chunk, entry) in updates {
                r.upload_chunk_mesh(did, chunk, &entry);
            }
            // If we have replicated NPCs and no zombie visuals, build a minimal instance set
            if r.zombie_count == 0 && !r.repl_buf.npcs.is_empty() {
                let joints = r.zombie_joints;
                let mut inst_cpu: Vec<crate::gfx::types::InstanceSkin> = Vec::new();
                let mut models: Vec<glam::Mat4> = Vec::new();
                let mut ids: Vec<u32> = Vec::new();
                for (i, n) in r.repl_buf.npcs.iter().enumerate() {
                    if !n.alive {
                        continue;
                    }
                    ids.push(n.id);
                    let (h, _n) = terrain::height_at(&r.terrain_cpu, n.pos.x, n.pos.z);
                    let pos = glam::vec3(n.pos.x, h, n.pos.z);
                    let m = glam::Mat4::from_scale_rotation_translation(
                        glam::Vec3::splat(1.0),
                        glam::Quat::IDENTITY,
                        pos,
                    );
                    models.push(m);
                    inst_cpu.push(crate::gfx::types::InstanceSkin {
                        model: m.to_cols_array_2d(),
                        color: [1.0, 1.0, 1.0],
                        selected: 0.0,
                        palette_base: (i as u32) * joints,
                        _pad_inst: [0; 3],
                    });
                }
                r.zombie_instances =
                    r.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("zombie-instances(repl)"),
                            contents: bytemuck::cast_slice(&inst_cpu),
                            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        });
                r.zombie_instances_cpu = inst_cpu;
                r.zombie_models = models;
                r.zombie_ids = ids;
                r.zombie_count = r.zombie_ids.len() as u32;
                log::info!(
                    "replication: built zombie visuals from {} NPCs (joints={})",
                    r.zombie_count,
                    r.zombie_joints
                );
                // Initialize tracking arrays to correct lengths
                r.zombie_prev_pos = r
                    .zombie_models
                    .iter()
                    .map(|m| {
                        let c = m.to_cols_array();
                        glam::vec3(c[12], c[13], c[14])
                    })
                    .collect();
                r.zombie_time_offset = (0..r.zombie_count as usize)
                    .map(|i| i as f32 * 0.35)
                    .collect();
                r.zombie_forward_offsets = vec![
                    crate::gfx::zombies::forward_offset(&r.zombie_cpu);
                    r.zombie_count as usize
                ];
                // Resize palette buffer (min 64 bytes)
                let total = (r.zombie_count as usize * r.zombie_joints as usize).max(1) * 64;
                r.zombie_palettes_buf = r.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("zombie-palettes"),
                    size: total as u64,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                r.zombie_palettes_bg = r.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("zombie-palettes-bg"),
                    layout: &r.palettes_bgl,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: r.zombie_palettes_buf.as_entire_binding(),
                    }],
                });
            }
        }
        // Projectiles: renderer is presentation-only. Prefer replicated projectiles, but
        // keep locally spawned PC projectiles alive briefly to avoid the "eaten immediately"
        // artifact when a cast is accepted locally and replication arrives a frame later.
        // Also detect fireballs that disappeared this frame and spawn an explosion VFX at their last position.
        //
        // Strategy:
        // - Start from replicated list for authoritative visuals
        // - Retain any existing local (owner_wizard.is_some()) projectiles whose lifetime
        //   hasn't expired and that do not have a nearby replicated counterpart this frame.
        //   This gives a small grace so local visuals aren't wiped before replication catches up.
        let mut next_map: std::collections::HashMap<u32, (u8, glam::Vec3)> =
            std::collections::HashMap::new();
        // Collect existing local (PC-owned) projectiles before we rebuild the list
        let mut locals: Vec<crate::gfx::fx::Projectile> = Vec::new();
        if !r.projectiles.is_empty() {
            let mut prev = Vec::new();
            std::mem::swap(&mut prev, &mut r.projectiles);
            for pr in prev {
                // Keep only locally spawned ones that are still alive
                if pr.owner_wizard.is_some() && r.last_time < pr.t_die {
                    locals.push(pr);
                }
            }
        }
        let mut rebuilt: Vec<crate::gfx::fx::Projectile> = Vec::new();
        for p in &r.repl_buf.projectiles {
            rebuilt.push(crate::gfx::fx::Projectile {
                pos: p.pos,
                vel: p.vel,
                t_die: t + 1.0, // visual grace period only
                owner_wizard: None,
                color: match p.kind {
                    1 => [2.2, 0.9, 0.3],
                    2 => [0.6, 0.7, 2.2],
                    _ => [2.6, 0.7, 0.18],
                },
                kind: match p.kind {
                    1 => crate::gfx::fx::ProjectileKind::Fireball {
                        radius: 6.0,
                        damage: 28,
                    },
                    2 => crate::gfx::fx::ProjectileKind::MagicMissile,
                    _ => crate::gfx::fx::ProjectileKind::Normal,
                },
            });
            next_map.insert(p.id, (p.kind, p.pos));
        }
        // Merge unmatched locals back in (no nearby replicated counterpart)
        if !locals.is_empty() {
            const DEDUP_R: f32 = 0.6; // meters
            for lp in locals {
                let mut matched = false;
                for rp in &rebuilt {
                    let same_kind = matches!(
                        (rp.kind, lp.kind),
                        (
                            crate::gfx::fx::ProjectileKind::Fireball { .. },
                            crate::gfx::fx::ProjectileKind::Fireball { .. }
                        ) | (
                            crate::gfx::fx::ProjectileKind::MagicMissile,
                            crate::gfx::fx::ProjectileKind::MagicMissile
                        ) | (
                            crate::gfx::fx::ProjectileKind::Normal,
                            crate::gfx::fx::ProjectileKind::Normal
                        )
                    );
                    if same_kind && rp.pos.distance(lp.pos) <= DEDUP_R {
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    rebuilt.push(lp);
                }
            }
        }
        r.projectiles = rebuilt;
        // Fireball disappear → VFX
        if !r.last_repl_projectiles.is_empty() {
            // Collect disappear events first to avoid aliasing mutable borrow
            let mut to_explode: Vec<glam::Vec3> = Vec::new();
            for (id, (kind, pos)) in r.last_repl_projectiles.iter() {
                if !next_map.contains_key(id) && *kind == 1 {
                    to_explode.push(*pos);
                }
            }
            for pos in to_explode {
                r.explode_fireball_at(None, pos, 6.0, 28);
            }
        }
        r.last_repl_projectiles = next_map;
        // Direct-hit sparks from replicated HitFx events (server-authoritative). One short burst per hit.
        if !r.repl_buf.hits.is_empty() {
            let mut hits = std::mem::take(&mut r.repl_buf.hits);
            if hits.len() > 32 {
                hits.truncate(32);
            }
            for h in hits {
                let pos = glam::vec3(h.pos[0], h.pos[1], h.pos[2]);
                // Clamp slightly above terrain to ensure visibility
                let pos = {
                    let (hgt, _n) = terrain::height_at(&r.terrain_cpu, pos.x, pos.z);
                    let y = pos.y.max(hgt + 0.05);
                    glam::vec3(pos.x, y, pos.z)
                };
                // Bright core flash
                r.particles.push(crate::gfx::fx::Particle {
                    pos,
                    vel: glam::Vec3::new(0.0, 0.6, 0.0),
                    age: 0.0,
                    life: 0.12,
                    size: 0.06,
                    color: [1.8, 1.2, 0.4],
                });
                // Small radial burst (deterministic, 8 spokes)
                let spokes = 8;
                for i in 0..spokes {
                    let a = (i as f32) / (spokes as f32) * std::f32::consts::TAU;
                    let rvel = 3.2f32;
                    r.particles.push(crate::gfx::fx::Particle {
                        pos,
                        vel: glam::vec3(a.cos() * rvel, 1.2, a.sin() * rvel),
                        age: 0.0,
                        life: 0.12,
                        size: 0.015,
                        color: [1.6, 0.9, 0.3],
                    });
                }
            }
        }
        // Fallback: if we already have a non-empty replicated NPC cache but haven't
        // built visuals yet (e.g., drain happened earlier), build now.
        if r.zombie_count == 0 && !r.repl_buf.npcs.is_empty() {
            let joints = r.zombie_joints;
            let mut inst_cpu: Vec<crate::gfx::types::InstanceSkin> = Vec::new();
            let mut models: Vec<glam::Mat4> = Vec::new();
            let mut ids: Vec<u32> = Vec::new();
            for (i, n) in r.repl_buf.npcs.iter().enumerate() {
                if !n.alive {
                    continue;
                }
                ids.push(n.id);
                let (h, _n) = terrain::height_at(&r.terrain_cpu, n.pos.x, n.pos.z);
                let pos = glam::vec3(n.pos.x, h, n.pos.z);
                let m = glam::Mat4::from_scale_rotation_translation(
                    glam::Vec3::splat(1.0),
                    glam::Quat::IDENTITY,
                    pos,
                );
                models.push(m);
                inst_cpu.push(crate::gfx::types::InstanceSkin {
                    model: m.to_cols_array_2d(),
                    color: [1.0, 1.0, 1.0],
                    selected: 0.0,
                    palette_base: (i as u32) * joints,
                    _pad_inst: [0; 3],
                });
            }
            if !inst_cpu.is_empty() {
                r.zombie_instances =
                    r.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("zombie-instances(repl-fallback)"),
                            contents: bytemuck::cast_slice(&inst_cpu),
                            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        });
                r.zombie_instances_cpu = inst_cpu;
                r.zombie_models = models;
                r.zombie_ids = ids;
                r.zombie_count = r.zombie_ids.len() as u32;
                // Initialize tracking arrays to correct lengths
                r.zombie_prev_pos = r
                    .zombie_models
                    .iter()
                    .map(|m| {
                        let c = m.to_cols_array();
                        glam::vec3(c[12], c[13], c[14])
                    })
                    .collect();
                r.zombie_time_offset = (0..r.zombie_count as usize)
                    .map(|i| i as f32 * 0.35)
                    .collect();
                r.zombie_forward_offsets = vec![
                    crate::gfx::zombies::forward_offset(&r.zombie_cpu);
                    r.zombie_count as usize
                ];
                // Resize palette buffer (min 64 bytes)
                let total = (r.zombie_count as usize * r.zombie_joints as usize).max(1) * 64;
                r.zombie_palettes_buf = r.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("zombie-palettes"),
                    size: total as u64,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                r.zombie_palettes_bg = r.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("zombie-palettes-bg"),
                    layout: &r.palettes_bgl,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: r.zombie_palettes_buf.as_entire_binding(),
                    }],
                });
                log::info!(
                    "replication: built zombie visuals (fallback) from {} NPCs",
                    r.zombie_count
                );
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
            r.scene_inputs.rig_add_yaw(dt * speed);
        } else {
            r.screenshot_start = None;
        }
    }

    // Update player transform (controls + collision) via external scene inputs
    {
        // Compute camera forward from rig yaw (authoritative for input), not smoothed follow
        let rig_yaw = r.scene_inputs.rig_yaw();
        let cam_fwd = glam::vec3(rig_yaw.sin(), 0.0, rig_yaw.cos());
        // Clear latched inputs on RMB edge (pointer-lock transitions can eat key-ups)
        if r.prev_rmb_down != r.rmb_down {
            r.a_down = false;
            r.d_down = false;
            r.q_down = false;
            r.e_down = false;
            r.input.turn_left = false;
            r.input.turn_right = false;
            r.input.strafe_left = false;
            r.input.strafe_right = false;
        }
        r.prev_rmb_down = r.rmb_down;
        // Derive per-frame controller flags that depend on mouse buttons
        r.input.mouse_look = r.rmb_down;
        r.input.click_move_forward = r.lmb_down && r.rmb_down;
        // Resolve raw buttons → intents and camera swing via client_core helper
        let raw = client_core::systems::pc_controller::RawButtons {
            w: r.input.forward,
            s: r.input.backward,
            a: r.a_down,
            d: r.d_down,
            q: r.q_down,
            e: r.e_down,
            lmb: r.lmb_down,
            rmb: r.rmb_down,
            shift: r.shift_down,
        };
        let out = client_core::systems::pc_controller::resolve(
            raw,
            client_core::systems::pc_controller::ResolveParams {
                dt,
                turn_speed_rad_per_s: 180.0f32.to_radians(),
            },
        );
        r.input.turn_left = false;
        r.input.turn_right = false;
        r.input.strafe_left = out.intents.strafe_left;
        r.input.strafe_right = out.intents.strafe_right;
        r.input.forward = out.intents.forward;
        r.input.backward = out.intents.backward;
        r.input.click_move_forward = out.intents.click_move_forward;
        r.input.run = out.intents.run;
        if out.cam_yaw_delta != 0.0 {
            r.scene_inputs.rig_add_yaw(out.cam_yaw_delta);
            client_core::systems::auto_face::register_cam_change(
                &mut r.cam_yaw_prev,
                &mut r.cam_yaw_changed_at,
                r.scene_inputs.rig_yaw(),
                r.last_time,
            );
        }
        // WoW: while RMB is held and moving forward (not back), snap facing to camera yaw
        let moving_forward = r.rmb_down && r.input.forward && !r.input.backward;
        if moving_forward {
            r.scene_inputs.set_yaw(r.scene_inputs.rig_yaw());
        }
        r.scene_inputs.apply_input(&r.input);
        // One-shot: clear jump so holding Space does not repeat
        r.input.jump_pressed = false;
        // Use camera basis (rig yaw) only when RMB is down; otherwise use character facing (yaw).
        let move_fwd = if r.rmb_down {
            cam_fwd
        } else {
            glam::vec3(r.player.yaw.sin(), 0.0, r.player.yaw.cos())
        };
        r.scene_inputs.update(dt, move_fwd, r.static_index.as_ref());
        // Auto-face camera yaw after a short delay if camera rotated
        {
            let cam_yaw = r.scene_inputs.rig_yaw(); // use rig yaw, not smoothed follow
            // Note: anchor updates occur only on explicit user input (mouse orbit / A/D swing).
            let cur_yaw = r.scene_inputs.yaw();
            // Compute panic vs normal and update face reset when exiting panic
            let mut d = cam_yaw - cur_yaw;
            while d > std::f32::consts::PI {
                d -= std::f32::consts::TAU;
            }
            while d < -std::f32::consts::PI {
                d += std::f32::consts::TAU;
            }
            let in_panic = d.abs() > std::f32::consts::FRAC_PI_2;
            if r.cam_prev_panic && !in_panic {
                r.cam_face_reset_at = r.last_time;
            }
            r.cam_prev_panic = in_panic;
            let last_anchor = if in_panic {
                r.cam_yaw_changed_at
            } else {
                r.cam_yaw_changed_at.max(r.cam_face_reset_at)
            };
            let delay = if r.rmb_down { 0.125 } else { 0.25 };
            let new_yaw = client_core::systems::auto_face::auto_face_step(
                cur_yaw,
                cam_yaw,
                client_core::systems::auto_face::AutoFaceParams {
                    last_change_at: last_anchor,
                    now: r.last_time,
                    delay_s: delay,
                    turning: false,
                    turn_speed_rad_per_s: 180.0f32.to_radians(),
                    dt,
                    panic_threshold_rad: std::f32::consts::FRAC_PI_2,
                    trail_by_threshold: true,
                    hysteresis_rad: 0.15, // ~8.6° inside threshold to guarantee exit
                },
            );
            if (new_yaw - cur_yaw).abs() > 1e-6 {
                r.scene_inputs.set_yaw(new_yaw);
            }
        }
        r.player.pos = r.scene_inputs.pos();
        r.player.yaw = r.scene_inputs.yaw();
        r.apply_pc_transform();
    }
    // No client-side AI; server authoritative yaw provided via replication
    // Compute local orbit offsets (relative to PC orientation)
    let near_d = 1.6f32;
    let far_d = 25.0f32;
    let (_, _, dist, lift_base, look_base) = r.scene_inputs.rig_values();
    let zoom_t = ((dist - near_d) / (far_d - near_d)).clamp(0.0, 1.0);
    let near_lift = 0.5f32; // meters above anchor when fully zoomed-in
    let near_look = 0.5f32; // aim point above anchor when fully zoomed-in
    let eff_lift = near_lift * (1.0 - zoom_t) + lift_base * zoom_t;
    let eff_look = near_look * (1.0 - zoom_t) + look_base * zoom_t;
    let (off_local, look_local) = camera_sys::compute_local_orbit_offsets(
        dist,
        r.scene_inputs.rig_yaw(),
        r.scene_inputs.rig_pitch(),
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

    // Send authoritative Move/Aim intents each frame when command TX is present
    if let Some(tx) = &r.cmd_tx {
        // Compute movement basis (XZ) like WoW:
        // - RMB held    -> camera forward/right basis
        // - RMB not held -> character yaw basis
        let basis_fwd_xz = if r.rmb_down {
            let cam_fwd =
                (r.cam_follow.current_look - r.cam_follow.current_pos).normalize_or_zero();
            glam::vec2(cam_fwd.x, cam_fwd.z).normalize_or_zero()
        } else {
            glam::vec2(r.player.yaw.sin(), r.player.yaw.cos())
        };
        let basis_right_xz = glam::vec2(basis_fwd_xz.y, -basis_fwd_xz.x);
        let mut mx = 0.0f32;
        let mut mz = 0.0f32;
        if r.input.strafe_right {
            mx += 1.0;
        }
        if r.input.strafe_left {
            mx -= 1.0;
        }
        if r.input.forward {
            mz += 1.0;
        }
        if r.input.backward {
            mz -= 1.0;
        }
        let mut v = basis_right_xz * mx + basis_fwd_xz * mz;
        if v.length_squared() > 1.0 {
            v = v.normalize();
        }
        // Net intent expects LEFT positive for dx, FORWARD positive for dz
        let dx = -v.x;
        let dz = v.y;
        // Move intent
        {
            let cmd = net_core::command::ClientCmd::Move {
                dx,
                dz,
                run: u8::from(r.input.run),
            };
            let mut payload = Vec::new();
            cmd.encode(&mut payload);
            let mut framed = Vec::with_capacity(payload.len() + 8);
            net_core::frame::write_msg(&mut framed, &payload);
            let _ = tx.try_send(framed);
        }
        // Aim intent (use current player yaw)
        {
            let cmd = net_core::command::ClientCmd::Aim { yaw: r.player.yaw };
            let mut payload = Vec::new();
            cmd.encode(&mut payload);
            let mut framed = Vec::with_capacity(payload.len() + 8);
            net_core::frame::write_msg(&mut framed, &payload);
            let _ = tx.try_send(framed);
        }
    }

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

    // Handle queued PC cast and update animation state (skip in Picker)
    if !r.is_picker_batches() {
        r.process_pc_cast(t);
    }
    // Sync wizard transforms from replicated positions so overlays/bars and projectile origins
    // align with visuals. Map PC to `pc_index`, then map NPC wizards to remaining instances
    // in order of appearance. Preserve current yaw when available, otherwise use replicated yaw.
    if !r.repl_buf.wizards.is_empty() && !r.wizard_models.is_empty() {
        use std::collections::HashSet;
        // 1) Build current id set
        let mut current_ids: HashSet<u32> = HashSet::new();
        for w in &r.repl_buf.wizards {
            current_ids.insert(w.id);
        }
        // 2) Free slots for ids that disappeared
        let mut to_remove: Vec<u32> = r
            .wizard_slot_map
            .keys()
            .copied()
            .filter(|id| !current_ids.contains(id))
            .collect();
        for id in to_remove.drain(..) {
            if let Some(slot) = r.wizard_slot_map.remove(&id)
                && slot < r.wizard_slot_id.len()
            {
                r.wizard_slot_id[slot] = None;
                r.wizard_free_slots.push(slot);
            }
        }
        // 3) Ensure PC id is pinned to pc_index; if occupied, move that id to a free slot
        if let Some(pcw) = r.repl_buf.wizards.iter().find(|w| w.is_pc) {
            let pc_id = pcw.id;
            if r.pc_rep_id_last != Some(pc_id) {
                log::info!("client: local PC replicated id={}", pc_id);
                r.pc_rep_id_last = Some(pc_id);
            }
            let mapped = r.wizard_slot_map.get(&pc_id).copied();
            match mapped {
                Some(slot) if slot == r.pc_index => {}
                Some(slot_other) => {
                    // Free pc_index if held by someone else
                    if let Some(other_id) = r.wizard_slot_id.get(r.pc_index).copied().flatten() {
                        // Move other_id to the former pc slot
                        r.wizard_slot_map.insert(other_id, slot_other);
                        r.wizard_slot_id[slot_other] = Some(other_id);
                    } else {
                        r.wizard_free_slots.retain(|&s| s != slot_other);
                        r.wizard_free_slots.push(slot_other);
                    }
                    r.wizard_slot_map.insert(pc_id, r.pc_index);
                    r.wizard_slot_id[r.pc_index] = Some(pc_id);
                }
                None => {
                    // Assign to pc_index; if someone occupies it, move them to a free slot
                    if let Some(other_id) = r.wizard_slot_id.get(r.pc_index).copied().flatten() {
                        let new_slot = if let Some(s) = r.wizard_free_slots.pop() {
                            s
                        } else {
                            r.pc_index
                        };
                        if new_slot != r.pc_index {
                            r.wizard_slot_map.insert(other_id, new_slot);
                            r.wizard_slot_id[new_slot] = Some(other_id);
                        }
                    }
                    r.wizard_slot_map.insert(pc_id, r.pc_index);
                    r.wizard_slot_id[r.pc_index] = Some(pc_id);
                }
            }
        }
        // 4) Map remaining ids to stable slots
        for w in &r.repl_buf.wizards {
            if r.wizard_slot_map.contains_key(&w.id) {
                continue;
            }
            // Find a free slot (skip pc_index)
            let mut slot = None;
            while let Some(s) = r.wizard_free_slots.pop() {
                if s != r.pc_index {
                    slot = Some(s);
                    break;
                }
            }
            if let Some(s) = slot {
                r.wizard_slot_map.insert(w.id, s);
                r.wizard_slot_id[s] = Some(w.id);
            } else {
                // Fallback: scan for any None slot
                if let Some((idx, _)) = r
                    .wizard_slot_id
                    .iter()
                    .enumerate()
                    .find(|(i, v)| v.is_none() && *i != r.pc_index)
                {
                    r.wizard_slot_map.insert(w.id, idx);
                    r.wizard_slot_id[idx] = Some(w.id);
                }
            }
        }
        // 5) Upload transforms per id→slot
        for w in &r.repl_buf.wizards {
            if let Some(&slot) = r.wizard_slot_map.get(&w.id) {
                if slot >= r.wizard_models.len() {
                    continue;
                }
                let cur = r.wizard_models[slot];
                let (_s, rot, _t) = cur.to_scale_rotation_translation();
                let pos = w.pos;
                let yaw = if w.yaw.is_finite() { w.yaw } else { 0.0 };
                let use_yaw = yaw != 0.0 || rot.length_squared() < 0.9;
                let new_m = if use_yaw {
                    glam::Mat4::from_scale_rotation_translation(
                        glam::Vec3::splat(1.0),
                        glam::Quat::from_rotation_y(yaw),
                        pos,
                    )
                } else {
                    glam::Mat4::from_scale_rotation_translation(glam::Vec3::splat(1.0), rot, pos)
                };
                if new_m.to_cols_array() != cur.to_cols_array() {
                    r.wizard_models[slot] = new_m;
                    let mut inst = r.wizard_instances_cpu[slot];
                    inst.model = new_m.to_cols_array_2d();
                    r.wizard_instances_cpu[slot] = inst;
                    let offset =
                        (slot * std::mem::size_of::<crate::gfx::types::InstanceSkin>()) as u64;
                    r.queue
                        .write_buffer(&r.wizard_instances, offset, bytemuck::bytes_of(&inst));
                }
            }
        }
        // Align draw count with replication (cap to capacity)
        r.wizard_count = r.repl_buf.wizards.len().min(r.wizard_models.len()) as u32;
    }
    // Update wizard skinning palettes on CPU then upload (skip in Picker)
    if !r.is_picker_batches() {
        r.update_wizard_palettes(t);
    }
    // Default build: rotate NPC wizards to face targets (player or nearest replicated NPC)
    {
        let yaw_rate = 2.5 * dt;
        let mut targets: Vec<glam::Vec3> = Vec::new();
        for n in &r.repl_buf.npcs {
            if n.alive {
                targets.push(n.pos);
            }
        }
        for i in 0..(r.wizard_count as usize) {
            if i == r.pc_index {
                continue;
            }
            let m = r.wizard_models[i];
            let pos = (m * glam::Vec4::new(0.0, 0.0, 0.0, 1.0)).truncate();
            let tgt = if !targets.is_empty() {
                let mut best = pos;
                let mut best_d2 = f32::INFINITY;
                for tpos in &targets {
                    let dx = tpos.x - pos.x;
                    let dz = tpos.z - pos.z;
                    let d2 = dx * dx + dz * dz;
                    if d2 < best_d2 {
                        best_d2 = d2;
                        best = *tpos;
                    }
                }
                best
            } else {
                pos
            };
            let desired = (tgt.x - pos.x).atan2(tgt.z - pos.z);
            let cur = {
                let c = m.to_cols_array();
                let (_, rquat, _) = glam::Mat4::from_cols_array(&c).to_scale_rotation_translation();
                let fwd = rquat * glam::Vec3::Z;
                fwd.x.atan2(fwd.z)
            };
            let delta = (desired - cur + std::f32::consts::PI)
                .rem_euclid(2.0 * std::f32::consts::PI)
                - std::f32::consts::PI;
            let new_yaw = if delta.abs() <= yaw_rate {
                desired
            } else {
                cur + yaw_rate * delta.signum()
            };
            if (new_yaw - cur).abs() > 1e-4 {
                let new_m = glam::Mat4::from_scale_rotation_translation(
                    glam::Vec3::splat(1.0),
                    glam::Quat::from_rotation_y(new_yaw),
                    pos,
                );
                r.wizard_models[i] = new_m;
                let mut inst = r.wizard_instances_cpu[i];
                inst.model = new_m.to_cols_array_2d();
                r.wizard_instances_cpu[i] = inst;
                let offset = (i * std::mem::size_of::<crate::gfx::types::InstanceSkin>()) as u64;
                r.queue
                    .write_buffer(&r.wizard_instances, offset, bytemuck::bytes_of(&inst));
            }
        }
    }
    // Update PC (UBC) palette if separate rig is active (skip in Picker)
    if !r.is_picker_batches() {
        r.update_pc_palette(t);
    }
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
        #[cfg(any())]
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
                let _ = fatal;
            }
        }
        r.update_zombies_from_replication();
        r.update_zombie_palettes(t);
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
        let pc_debug = std::env::var("RA_PC_DEBUG")
            .map(|v| v == "1")
            .unwrap_or(false);
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
    // Capture validation across the entire main pass to surface concrete errors
    #[cfg(not(target_arch = "wasm32"))]
    r.device.push_error_scope(wgpu::ErrorFilter::Validation);
    if !present_only {
        let pc_debug = std::env::var("RA_PC_DEBUG")
            .map(|v| v == "1")
            .unwrap_or(false);
        // Depth is required for the normal scene. In debug-isolate we only
        // omit depth when the picker is active and we're not drawing the PC.
        // Otherwise, keep depth so pipelines that expect it are compatible.
        let want_depth = if pc_debug {
            // In debug: use depth unless the picker overlay is active.
            !r.is_picker_batches()
        } else {
            // In normal runs: always use depth for the main pass.
            true
        };
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
            depth_stencil_attachment: if want_depth {
                Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &r.attachments.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                })
            } else {
                None
            },
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        if pc_debug {
            // If the picker overlay is active, do NOT draw the PC in debug mode.
            if r.is_picker_batches() {
                // Keep the pass open but do not draw the PC while the Zone Picker is active.
            } else {
                // Proactively ensure PC rig assets exist in debug isolate
                r.ensure_pc_assets();
                let shard_m = crate::gfx::types::Model {
                    model: glam::Mat4::IDENTITY.to_cols_array_2d(),
                    color: [1.0, 1.0, 1.0],
                    emissive: 0.0,
                    _pad: [0.0; 4],
                };
                r.queue
                    .write_buffer(&r.shard_model_buf, 0, bytemuck::bytes_of(&shard_m));
                r.draw_pc_only(&mut rp);
                drop(rp);
                // Present immediately for the isolate path (no HUD perf text)
                r.hud.queue(&r.device, &r.queue);
                r.hud.draw(&mut encoder, &view);
                r.queue.submit(Some(encoder.finish()));
                frame.present();
                #[cfg(not(target_arch = "wasm32"))]
                {
                    if let Some(e) = pollster::block_on(r.device.pop_error_scope()) {
                        log::error!("validation (main pass): {:?}", e);
                    }
                }
                return Ok(());
            }
        }
        if !pc_debug {
            // Terrain
            let trace = std::env::var("RA_TRACE").map(|v| v == "1").unwrap_or(false);
            if std::env::var("RA_DRAW_TERRAIN")
                .map(|v| v == "0")
                .unwrap_or(false)
            {
                log::debug!("draw: terrain skipped (RA_DRAW_TERRAIN=0)");
            } else if !r.is_picker_batches() {
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
            // Ghost preview (worldsmithing): draw a single cube instance if present
            if r.ghost_present {
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
                rp.set_vertex_buffer(0, r.ghost_vb.slice(..));
                rp.set_vertex_buffer(1, r.ghost_inst.slice(..));
                rp.set_index_buffer(r.ghost_ib.slice(..), wgpu::IndexFormat::Uint16);
                rp.draw_indexed(0..r.ghost_index_count, 0, 0..1);
                r.draw_calls += 1;
                #[cfg(not(target_arch = "wasm32"))]
                if trace && let Some(e) = pollster::block_on(r.device.pop_error_scope()) {
                    log::error!("validation after ghost: {:?}", e);
                }
            }
            // Trees
            // Show vegetation when not in Picker. Previously this was suppressed when
            // zone_batches existed; until zone-baked draws land, allow draws here too.
            if !r.is_vox_onepath() && !r.is_picker_batches() && !pc_debug {
                // Prefer drawing per-kind groups when present; otherwise fall back to single batch
                if !r.trees_groups.is_empty() {
                    let total: u32 = r.trees_groups.iter().map(|g| g.count).sum();
                    log::debug!(
                        "draw: trees groups x{} (total {})",
                        r.trees_groups.len(),
                        total
                    );
                    if trace {
                        #[cfg(not(target_arch = "wasm32"))]
                        r.device.push_error_scope(wgpu::ErrorFilter::Validation);
                    }
                    let inst_pipe = &r.inst_tex_pipeline;
                    rp.set_pipeline(inst_pipe);
                    rp.set_bind_group(0, &r.globals_bg, &[]);
                    rp.set_bind_group(1, &r.shard_model_bg, &[]);
                    // Textured instanced pipeline layout expects palettes at group(2) even if unused.
                    rp.set_bind_group(2, &r.palettes_bg, &[]);
                    for g in &r.trees_groups {
                        if g.count == 0 {
                            continue;
                        }
                        // Always bind a material BG; fall back to default if group has none
                        let mat_bg = g.material_bg.as_ref().unwrap_or(&r.default_material_bg);
                        // Material lives at group(3) for textured instanced pipeline
                        rp.set_bind_group(3, mat_bg, &[]);
                        rp.set_vertex_buffer(0, g.vb.slice(..));
                        rp.set_vertex_buffer(1, g.instances.slice(..));
                        rp.set_index_buffer(g.ib.slice(..), wgpu::IndexFormat::Uint16);
                        rp.draw_indexed(0..g.index_count, 0, 0..g.count);
                        r.draw_calls += 1;
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    if trace && let Some(e) = pollster::block_on(r.device.pop_error_scope()) {
                        log::error!("validation after trees: {:?}", e);
                    }
                } else if r.trees_count > 0 {
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
            }
            // Rocks
            if r.rocks_count > 0 && !r.is_vox_onepath() && !r.is_picker_batches() && !pc_debug {
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
            if r.ruins_count > 0 && !r.is_vox_onepath() && !r.is_picker_batches() && !pc_debug {
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
                #[cfg(not(target_arch = "wasm32"))]
                if trace && let Some(e) = pollster::block_on(r.device.pop_error_scope()) {
                    log::error!("validation after ruins: {:?}", e);
                }
            }
            // TEMP: debug cube to prove camera & pass are visible
            if std::env::var("RA_PC_DEBUG").as_deref() == Ok("1")
                && r.has_zone_batches()
                && !r.is_picker_batches()
            {
                let m = glam::Mat4::from_translation(glam::vec3(0.0, 1.6, 0.0));
                r.draw_debug_cube(&mut rp, m);
            }
            // Voxel chunk meshes (if any)
            if !r.voxel_meshes.is_empty() && !pc_debug {
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
        }
        // Skinned: wizards (PC always visible even if hide_wizards)
        if r.is_vox_onepath() {
            // skip wizard visuals entirely in one‑path demo
        } else if r.has_zone_batches() && !r.is_picker_batches() {
            // Draw only the PC rig when a real zone is active (not in Picker)
            let pc_ready = r.pc_vb.is_some()
                && r.pc_ib.is_some()
                && r.pc_instances.is_some()
                && r.pc_mat_bg.is_some()
                && r.pc_palettes_bg.is_some()
                && r.pc_index_count > 0;
            if pc_ready {
                // Always validate the PC draw pass so encoder errors are surfaced with context.
                #[cfg(not(target_arch = "wasm32"))]
                r.device.push_error_scope(wgpu::ErrorFilter::Validation);

                // Ensure the per-draw model UBO (group=1) is valid; identity is fine because
                // the PC's per-instance matrix carries the actual transform.
                let shard_m = crate::gfx::types::Model {
                    model: glam::Mat4::IDENTITY.to_cols_array_2d(),
                    color: [1.0, 1.0, 1.0],
                    emissive: 0.0,
                    _pad: [0.0; 4],
                };
                r.queue
                    .write_buffer(&r.shard_model_buf, 0, bytemuck::bytes_of(&shard_m));

                // Extra visibility: record resource readiness
                log::debug!(
                    "pc_draw(start): vb={} ib={} inst={} mat={} pal={} idx={}",
                    r.pc_vb.is_some(),
                    r.pc_ib.is_some(),
                    r.pc_instances.is_some(),
                    r.pc_mat_bg.is_some(),
                    r.pc_palettes_bg.is_some(),
                    r.pc_index_count
                );
                r.draw_pc_only(&mut rp);
                r.draw_calls += 1;
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(e) = pollster::block_on(r.device.pop_error_scope()) {
                    log::error!("validation after PC draw: {:?}", e);
                }
                // No HUD marker in normal builds; keep logs only
            } else {
                log::debug!(
                    "pc_draw: skipped (policy requires actors), ready? vb={} ib={} inst={} mat={} pal={} idx={}",
                    r.pc_vb.is_some(),
                    r.pc_ib.is_some(),
                    r.pc_instances.is_some(),
                    r.pc_mat_bg.is_some(),
                    r.pc_palettes_bg.is_some(),
                    r.pc_index_count
                );
            }
        } else if !r.has_zone_batches()
            && !pc_debug
            && std::env::var("RA_DRAW_WIZARDS")
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
        // Skinned: Death Knight (boss)
        if r.dk_count > 0
            && !r.is_vox_onepath()
            && !r.has_zone_batches()
            && r.repl_buf.boss_status.is_some()
        {
            log::debug!("draw: deathknight x{}", r.dk_count);
            r.draw_deathknight(&mut rp);
            r.draw_calls += 1;
        }
        // Skinned: Sorceress (static idle)
        if r.sorc_count > 0 && !r.is_vox_onepath() && !r.has_zone_batches() {
            log::debug!("draw: sorceress x{}", r.sorc_count);
            r.draw_sorceress(&mut rp);
            r.draw_calls += 1;
        }
        // Skinned: zombies
        if !r.is_vox_onepath()
            && !r.has_zone_batches()
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
    if !overlays_disabled && !r.is_vox_onepath() && !r.has_zone_batches() {
        // Bars for wizards from replicated views (positions + HP), with distance cull for NPCs.
        let mut bar_entries: Vec<(glam::Vec3, f32)> = Vec::new();
        let pc_pos = if let Some(pcw) = r.repl_buf.wizards.iter().find(|w| w.is_pc) {
            pcw.pos
        } else {
            let m = r
                .wizard_models
                .get(r.pc_index)
                .copied()
                .unwrap_or(glam::Mat4::IDENTITY);
            let c = m.to_cols_array();
            glam::vec3(c[12], c[13], c[14])
        };
        let npc_bar_radius: f32 = std::env::var("RA_NPC_BAR_RADIUS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(25.0);
        for w in &r.repl_buf.wizards {
            if w.hp <= 0 {
                continue;
            }
            let is_pc = w.is_pc;
            if !is_pc {
                let d2 = (w.pos.x - pc_pos.x).powi(2) + (w.pos.z - pc_pos.z).powi(2);
                if d2 > npc_bar_radius * npc_bar_radius {
                    continue;
                }
            }
            let head = glam::vec3(w.pos.x, w.pos.y + 1.7, w.pos.z);
            let frac = (w.hp.max(0) as f32) / (w.max.max(1) as f32);
            bar_entries.push((head, frac));
        }
        // Bars for alive zombies (replication only)
        {
            use std::collections::HashMap;
            let mut npc_map: HashMap<u32, (i32, i32, bool)> = HashMap::new();
            for n in &r.repl_buf.npcs {
                npc_map.insert(n.id, (n.hp, n.max, n.alive));
            }
            for (i, id) in r.zombie_ids.iter().enumerate() {
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
        // Death Knight health bar (use server HP; lower vertical offset)
        if r.dk_count > 0
            && let Some(m) = r.dk_models.first().copied()
        {
            // Prefer replication; if not present, only fallback under legacy feature.
            if let Some(bs) = r.repl_buf.boss_status.as_ref() {
                let frac = (bs.hp.max(0) as f32) / (bs.max.max(1) as f32);
                let head = m * glam::Vec4::new(0.0, 1.6, 0.0, 1.0);
                bar_entries.push((head.truncate(), frac));
            }
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
    if !r.is_vox_onepath() && !r.has_zone_batches() {
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

    #[cfg(test)]
    {
        // Sanity: wizard positions used for overlays should be distinct for different instances
        // (guards against accidental collapse to a single transform)
        if r.wizard_models.len() >= 2 {
            let a = r.wizard_models[0].to_cols_array();
            let b = r.wizard_models[1].to_cols_array();
            let pa = glam::vec3(a[12], a[13], a[14]);
            let pb = glam::vec3(b[12], b[13], b[14]);
            debug_assert!((pa - pb).length_squared() > 1e-6);
        }
        // Bars should cull by distance for NPC wizards (default radius 25m)
        {
            let old_models = r.wizard_models.clone();
            let old_hp = r.wizard_hp.clone();
            // Two wizards: PC at origin, NPC at 100m
            r.wizard_models = vec![
                glam::Mat4::from_translation(glam::vec3(0.0, 0.6, 0.0)),
                glam::Mat4::from_translation(glam::vec3(100.0, 0.6, 0.0)),
            ];
            r.wizard_hp = vec![100, 100];
            r.pc_index = 0;
            let view_proj = glam::Mat4::IDENTITY.to_cols_array_2d();
            let _vp = glam::Mat4::from_cols_array_2d(&view_proj);
            let mut entries: Vec<(glam::Vec3, f32)> = Vec::new();
            // Re-run the same culling logic locally
            let pc_pos = glam::vec3(0.0, 0.6, 0.0);
            let npc_bar_radius: f32 = 25.0;
            for (i, m) in r.wizard_models.iter().enumerate() {
                let hp = r.wizard_hp.get(i).copied().unwrap_or(0);
                if hp <= 0 {
                    continue;
                }
                if i != r.pc_index {
                    let c = m.to_cols_array();
                    let pos = glam::vec3(c[12], c[13], c[14]);
                    let d2 = (pos.x - pc_pos.x).powi(2) + (pos.z - pc_pos.z).powi(2);
                    if d2 > npc_bar_radius * npc_bar_radius {
                        continue;
                    }
                }
                let head = *m * glam::Vec4::new(0.0, 1.7, 0.0, 1.0);
                entries.push((head.truncate(), 1.0));
            }
            // Only the PC should remain within radius
            assert_eq!(entries.len(), 1);
            // Restore
            r.wizard_models = old_models;
            r.wizard_hp = old_hp;
        }
    }

    // Nameplates disabled by default. Set RA_NAMEPLATES=1 to enable.
    let draw_labels = std::env::var("RA_NAMEPLATES")
        .map(|v| v == "1")
        .unwrap_or(false);
    if draw_labels && !r.is_vox_onepath() && !r.has_zone_batches() {
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
        // NPC nameplates: prefer replication to filter out dead
        let mut npc_positions: Vec<glam::Vec3> = Vec::new();
        use std::collections::HashSet;
        let mut alive: HashSet<u32> = HashSet::new();
        for n in &r.repl_buf.npcs {
            if n.alive {
                alive.insert(n.id);
            }
        }
        for (idx, id) in r.zombie_ids.iter().enumerate() {
            if !alive.is_empty() && !alive.contains(id) {
                continue;
            }
            if let Some(m) = r.zombie_models.get(idx).copied() {
                let head = m * glam::Vec4::new(0.0, 1.6, 0.0, 1.0);
                npc_positions.push(head.truncate());
            }
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
                    load: wgpu::LoadOp::Load,
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

    // Zone Picker overlay: drawn by platform via Renderer::draw_picker_overlay()
    // Submit (defer error-scope pop until AFTER submit to catch pass/encoder errors)
    log::debug!("submit: normal path");
    // legacy BossStatus emission removed; HUD is replication-driven
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
    // If casting is not allowed in this zone, suppress any cast progress overlay
    let cast_frac = if r.zone_policy.allow_casting {
        cast_frac
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
    } else if !overlays_disabled && !r.is_picker_batches() {
        // Zone policy controls whether to show HUD (including hotbar)
        let show_hud = r.zone_policy.show_player_hud;
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
            // Compute seconds remaining for numeric cooldown labels
            let cd1_secs = cd1 * r.firebolt_cd_dur;
            let cd2_secs = cd2 * r.magic_missile_cd_dur;
            let cd3_secs = cd3 * r.fireball_cd_dur;
            if show_hud {
                r.hud.build(
                    r.size.width,
                    r.size.height,
                    pc_hp,
                    r.wizard_hp_max,
                    r.repl_buf.hud.mana as i32,
                    r.repl_buf.hud.mana_max as i32,
                    cast_frac,
                    cd1,
                    cd2,
                    cd3,
                    cd1_secs,
                    cd2_secs,
                    cd3_secs,
                    cast_label,
                );
            } else {
                r.hud.reset();
            }
            // Boss banner (top-center) via replicated cache or server fallback
            let mut boss_line: Option<String> = None;
            if let Some(bs) = r.repl_buf.boss_status.as_ref() {
                boss_line = Some(format!(
                    "{} — HP {}/{}  AC {}",
                    bs.name, bs.hp, bs.max, bs.ac
                ));
            } else {
                #[cfg(any())]
                if let Some(st) = r.server.nivita_status() {
                    boss_line = Some(format!(
                        "{} — HP {}/{}  AC {}",
                        st.name, st.hp, st.max, st.ac
                    ));
                }
            }
            if let Some(txt) = boss_line {
                r.hud.append_center_text(
                    r.size.width,
                    r.size.height,
                    &txt,
                    20.0,
                    [0.95, 0.98, 1.0, 0.95],
                );
                // Also update DK model position from replication (snap to terrain)
                if r.dk_count > 0
                    && let Some(bs) = r.repl_buf.boss_status.as_ref()
                {
                    let (h, _n) = terrain::height_at(&r.terrain_cpu, bs.pos[0], bs.pos[2]);
                    let pos = glam::vec3(bs.pos[0], h, bs.pos[2]);
                    if let Some(m) = r.dk_models.get_mut(0) {
                        let (_, rq, _) = m.to_scale_rotation_translation();
                        *m = glam::Mat4::from_scale_rotation_translation(
                            glam::Vec3::splat(2.5),
                            rq,
                            pos,
                        );
                        if let Some(inst) = r.dk_instances_cpu.get_mut(0) {
                            inst.model = m.to_cols_array_2d();
                            r.queue
                                .write_buffer(&r.dk_instances, 0, bytemuck::bytes_of(inst));
                        }
                    }
                }
            }
            // HUD toasts: show transient messages for this frame
            if !r.repl_buf.toasts.is_empty() {
                for code in std::mem::take(&mut r.repl_buf.toasts) {
                    if code == 1 {
                        r.hud.append_center_text(
                            r.size.width,
                            r.size.height,
                            "Not enough mana",
                            18.0,
                            [1.0, 0.2, 0.2, 1.0],
                        );
                    }
                }
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
                #[cfg(any())]
                if let Some(st) = r.server.nivita_status() {
                    let line = format!("Boss: {}  HP {}/{}  AC {}", st.name, st.hp, st.max, st.ac);
                    r.hud
                        .append_perf_text_line(r.size.width, r.size.height, &line, 4);
                }
            }
        }
        // Hint overlay removed for CC demo and general scenes.
    }
    r.hud.queue(&r.device, &r.queue);
    r.hud.draw(&mut encoder, &view);
    r.queue.submit(Some(encoder.finish()));
    frame.present();
    // Pop the validation scope after submit; this captures any errors raised
    // during encoder.finish() or queue.submit().
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(e) = pollster::block_on(r.device.pop_error_scope()) {
        log::error!("validation (main pass): {:?}", e);
        return Ok(());
    }
    Ok(())
}

// Unit tests for WoW-style controller input mapping live in a separate file
// to keep this file focused on rendering logic.
#[cfg(test)]
mod render_input_tests;
