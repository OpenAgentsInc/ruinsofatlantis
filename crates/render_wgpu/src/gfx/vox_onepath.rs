//! vox_onepath: self-contained, deterministic voxel demo.
//!
//! - Creates a tiny winit window + Renderer
//! - Replaces world content with a procedural voxel block 6m in front
//! - Pre-meshes all chunks synchronously on init
//! - Fires one scripted ray from camera forward to carve a 0.25m sphere
//! - Saves a screenshot to `target/vox_onepath.png` when the remesh queue drains

use crate::gfx::{Renderer, camera_sys};
use anyhow::Result;
use glam::{DVec3, UVec3};
use server_core::destructible::config::DestructibleConfig;
use std::path::PathBuf;
use voxel_proxy::{GlobalId, VoxelGrid, VoxelProxyMeta};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes},
};

// Tiny deterministic RNG: SplitMix64 + helpers (no external deps)
#[inline]
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[inline]
fn rand01(s: &mut u64) -> f32 {
    let r = splitmix64(s);
    ((r >> 40) as u32) as f32 / (1u32 << 24) as f32
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// Intersect a ray with an axis-aligned box. Returns (t_enter, t_exit) along the ray.
#[inline]
fn ray_box_intersect(
    p0: glam::Vec3,
    dir: glam::Vec3,
    bmin: glam::Vec3,
    bmax: glam::Vec3,
) -> Option<(f32, f32)> {
    let inv = glam::Vec3::new(
        if dir.x.abs() > 1e-8 {
            1.0 / dir.x
        } else {
            f32::INFINITY
        },
        if dir.y.abs() > 1e-8 {
            1.0 / dir.y
        } else {
            f32::INFINITY
        },
        if dir.z.abs() > 1e-8 {
            1.0 / dir.z
        } else {
            f32::INFINITY
        },
    );
    let t0s = (bmin - p0) * inv;
    let t1s = (bmax - p0) * inv;
    let tmin = t0s.min(t1s);
    let tmax = t0s.max(t1s);
    let t_enter = tmin.x.max(tmin.y).max(tmin.z);
    let t_exit = tmax.x.min(tmax.y).min(tmax.z);
    if t_exit >= t_enter.max(0.0) {
        Some((t_enter.max(0.0), t_exit))
    } else {
        None
    }
}

pub fn run() -> Result<()> {
    // Skip in headless environments (CI)
    if is_headless() {
        return Ok(());
    }
    let event_loop = EventLoop::new()?;
    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn is_headless() -> bool {
    if std::env::var("RA_HEADLESS")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        return true;
    }
    if std::env::var("CI")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false)
    {
        return true;
    }
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
            return true;
        }
    }
    false
}

#[derive(Default)]
struct App {
    window: Option<Window>,
    state: Option<Renderer>,
    script: Script,
}

#[derive(Default)]
struct Script {
    shot: bool,
    carved: bool,
    saved: bool,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = el
            .create_window(
                WindowAttributes::default()
                    .with_title("VOX ONEPATH")
                    .with_maximized(true),
            )
            .expect("create window");

        // Initialize the default renderer (full path), then surgically convert to our demo.
        let mut renderer = match pollster::block_on(Renderer::new(&window)) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Renderer init failed: {e}");
                el.exit();
                return;
            }
        };

        // Force HUD perf overlay on so we can append a one-line checklist.
        renderer.hud_model.toggle_perf();

        // Replace destructible config with a fixed, deterministic setup.
        renderer.destruct_cfg = DestructibleConfig {
            voxel_size_m: core_units::Length::meters(0.10),
            chunk: UVec3::new(32, 32, 32),
            material: core_materials::find_material_id("stone").unwrap(),
            max_debris: 250,
            max_chunk_remesh: 3,
            close_surfaces: false,
            profile: false,
            seed: 12345,
            debris_vs_world: false,
            demo_grid: false,
            replay_log: None,
            replay: None,
            voxel_model: None,
            vox_tiles_per_meter: Some(0.25),
            max_carve_chunks: Some(16),
            vox_sandbox: true,
            hide_wizards: true,
            vox_offset: None,
        };

        // Build a procedural voxel block grid (64x32x64), origin 1m forward.
        let dims = UVec3::new(64, 32, 64);
        let vm = renderer.destruct_cfg.voxel_size_m;
        let meta = VoxelProxyMeta {
            object_id: GlobalId(1),
            origin_m: DVec3::new(0.0, 0.0, 1.0),
            voxel_m: vm,
            dims,
            chunk: renderer.destruct_cfg.chunk,
            material: renderer.destruct_cfg.material,
        };
        let mut grid = VoxelGrid::new(meta);
        for z in 16..48 {
            for y in 0..20 {
                for x in 16..48 {
                    grid.set(x, y, z, true);
                }
            }
        }
        // Install grid and enqueue all chunks once
        let dims = grid.dims();
        let csz = grid.meta().chunk;
        let nx = dims.x.div_ceil(csz.x);
        let ny = dims.y.div_ceil(csz.y);
        let nz = dims.z.div_ceil(csz.z);
        renderer.voxel_grid = Some(grid.clone());
        renderer.voxel_grid_initial = Some(grid);
        // Mark as vox_onepath so we use naive mesher + burst remesh from the first premesh
        renderer.vox_onepath_ui = Some((false, false, false));
        for cz in 0..nz {
            for cy in 0..ny {
                for cx in 0..nx {
                    renderer
                        .chunk_queue
                        .enqueue_many([glam::UVec3::new(cx, cy, cz)]);
                }
            }
        }
        renderer.impact_id = 0;
        renderer.vox_queue_len = renderer.chunk_queue.len();

        // For the demo, force-rebuild all chunk meshes immediately so the first frame shows geometry
        force_remesh_all(&mut renderer);

        // Hide NPCs/wizards completely for a clean demo
        renderer.server.npcs.clear();
        renderer.zombie_count = 0;
        renderer.dk_count = 0;
        renderer.dk_id = None;
        renderer.rocks_count = 0;
        renderer.trees_count = 0;
        renderer.ruins_count = 0;
        if renderer.destruct_cfg.hide_wizards {
            renderer.wizard_count = 0;
        }

        self.window = Some(window);
        self.state = Some(renderer);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: winit::window::WindowId, e: WindowEvent) {
        let (Some(window), Some(state)) = (&self.window, &mut self.state) else {
            return;
        };
        if window.id() != id {
            return;
        }
        match e {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => state.resize(size),
            WindowEvent::RedrawRequested => {
                // Update UI checklist
                let meshed = state.vox_queue_len == 0 && state.vox_last_chunks == 0;
                state.vox_onepath_ui = Some((self.script.shot, self.script.carved, meshed));

                // Render the frame
                if let Err(err) = state.render() {
                    match err {
                        wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated => {
                            state.resize(window.inner_size())
                        }
                        wgpu::SurfaceError::OutOfMemory => el.exit(),
                        e => eprintln!("render error: {e:?}"),
                    }
                }

                // No auto-carve: wait for explicit input so debris only spawns on hit

                // When remesh queue drains, save a screenshot once.
                if self.script.shot && self.script.carved && !self.script.saved && meshed {
                    let path = PathBuf::from("target/vox_onepath.png");
                    if let Err(e) = save_screenshot(state, &path) {
                        log::error!("screenshot failed: {e}");
                    } else {
                        log::info!("saved screenshot: {:?}", path);
                    }
                    self.script.saved = true;
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                use winit::keyboard::{KeyCode, PhysicalKey};
                let pressed = event.state.is_pressed();
                log::info!("[onepath] key {:?} pressed={}", event.physical_key, pressed);
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Space) | PhysicalKey::Code(KeyCode::Enter) => {
                        if pressed {
                            // Aim from camera forward and carve once
                            let aspect = state.size.width as f32 / state.size.height.max(1) as f32;
                            let (off, look) = camera_sys::compute_local_orbit_offsets(
                                state.cam_distance,
                                state.cam_orbit_yaw,
                                state.cam_orbit_pitch,
                                state.cam_lift,
                                state.cam_look_height,
                            );
                            let (cam, _g) = camera_sys::third_person_follow(
                                &mut state.cam_follow,
                                state.scene_inputs.pos(),
                                glam::Quat::IDENTITY,
                                off,
                                look,
                                aspect,
                                0.0,
                            );
                            let p0 = cam.eye;
                            // Aim at the center of the voxel grid to guarantee a hit
                            let p1 = if let Some(ref grid) = state.voxel_grid {
                                let vm = grid.voxel_m().0 as f32;
                                let dims = grid.dims();
                                let origin = grid.origin_m();
                                let center = glam::vec3(
                                    (origin.x as f32) + (dims.x as f32 * vm * 0.5),
                                    (origin.y as f32) + (dims.y as f32 * vm * 0.5),
                                    (origin.z as f32) + (dims.z as f32 * vm * 0.5),
                                );
                                let dir = (center - p0).normalize_or_zero();
                                p0 + dir * 10.0
                            } else {
                                p0 + (cam.target - cam.eye).normalize_or_zero() * 10.0
                            };
                            let pre = state.vox_queue_len;
                            let _pre_debris = state.debris.len();
                            log::info!("[onepath] carve attempt from p0={:?} -> p1={:?}", p0, p1);
                            state.try_voxel_impact(p0, p1);
                            // Fallback: if nothing enqueued, raycast into the current grid along the camera ray
                            if state.vox_queue_len == pre
                                && let Some(ref mut grid) = state.voxel_grid
                            {
                                let dir = (p1 - p0).normalize_or_zero();
                                if dir.length_squared() > 1e-6 {
                                    // Move the ray origin to the entry point of the grid AABB to avoid DDA starting outside
                                    let o = grid.origin_m();
                                    let vmf = grid.voxel_m().0 as f32;
                                    let dims = grid.dims();
                                    let gmin = glam::vec3(o.x as f32, o.y as f32, o.z as f32);
                                    let gmax = gmin
                                        + glam::vec3(
                                            dims.x as f32 * vmf,
                                            dims.y as f32 * vmf,
                                            dims.z as f32 * vmf,
                                        );
                                    if let Some((t_enter, _t_exit)) =
                                        ray_box_intersect(p0, dir, gmin, gmax)
                                    {
                                        let eps = vmf * 1e-3;
                                        let p_entry = p0 + dir * (t_enter + eps);
                                        // Use the shared DDA in server_core to find the first solid along the ray
                                        let origin = DVec3::new(
                                            p_entry.x as f64,
                                            p_entry.y as f64,
                                            p_entry.z as f64,
                                        );
                                        let dir_m =
                                            DVec3::new(dir.x as f64, dir.y as f64, dir.z as f64);
                                        // Max length: diagonal of the grid AABB
                                        let vm = grid.voxel_m().0 as f32;
                                        let d = grid.dims();
                                        let ext = glam::vec3(
                                            d.x as f32 * vm,
                                            d.y as f32 * vm,
                                            d.z as f32 * vm,
                                        );
                                        let max_len = ext.length() as f64;
                                        let max_len_m = core_units::Length::meters(max_len);
                                        if let Some(hit) = server_core::destructible::raycast_voxels(
                                            grid, origin, dir_m, max_len_m,
                                        ) {
                                            let o = grid.origin_m();
                                            let vm = grid.voxel_m().0;
                                            let vc = DVec3::new(
                                                hit.voxel.x as f64 + 0.5,
                                                hit.voxel.y as f64 + 0.5,
                                                hit.voxel.z as f64 + 0.5,
                                            );
                                            let impact = o + vc * vm;
                                            // Per‑press jitter for variety
                                            let mut rng = state.destruct_cfg.seed
                                                ^ state
                                                    .impact_id
                                                    .wrapping_mul(0xD2B7_4407_B1CE_6E93);
                                            let radius_m = (0.26 + 0.22 * rand01(&mut rng)) as f64;
                                            let seed = splitmix64(&mut rng);
                                            // First impact at current surface
                                            let mut total_debris = 0usize;
                                            let out =
                                                server_core::destructible::carve_and_spawn_debris(
                                                    grid,
                                                    impact,
                                                    core_units::Length::meters(radius_m),
                                                    seed,
                                                    state.impact_id,
                                                    state.destruct_cfg.max_debris,
                                                );
                                            state.impact_id = state.impact_id.wrapping_add(1);
                                            total_debris += out.positions_m.len();
                                            let mut start = glam::vec3(
                                                impact.x as f32,
                                                impact.y as f32,
                                                impact.z as f32,
                                            ) + dir * (radius_m as f32 * 0.9);
                                            // Optional: drill a few steps deeper along the same ray this press
                                            let drill_steps = 4usize;
                                            for _ in 0..drill_steps {
                                                if let Some(next) =
                                                    server_core::destructible::raycast_voxels(
                                                        grid,
                                                        DVec3::new(
                                                            start.x as f64,
                                                            start.y as f64,
                                                            start.z as f64,
                                                        ),
                                                        dir_m,
                                                        max_len_m,
                                                    )
                                                {
                                                    let vc2 = DVec3::new(
                                                        next.voxel.x as f64 + 0.5,
                                                        next.voxel.y as f64 + 0.5,
                                                        next.voxel.z as f64 + 0.5,
                                                    );
                                                    let impact2 = o + vc2 * vm;
                                                    let r2 =
                                                        (0.24 + 0.20 * rand01(&mut rng)) as f64;
                                                    let seed2 = splitmix64(&mut rng);
                                                    let out2 = server_core::destructible::carve_and_spawn_debris(
                                                    grid,
                                                    impact2,
                                                    core_units::Length::meters(r2),
                                                    seed2,
                                                    state.impact_id,
                                                    state.destruct_cfg.max_debris,
                                                );
                                                    state.impact_id =
                                                        state.impact_id.wrapping_add(1);
                                                    total_debris += out2.positions_m.len();
                                                    start = glam::vec3(
                                                        impact2.x as f32,
                                                        impact2.y as f32,
                                                        impact2.z as f32,
                                                    ) + dir * (r2 as f32 * 1.25);
                                                } else {
                                                    break;
                                                }
                                            }
                                            // enqueue & show immediately
                                            let enq = grid.pop_dirty_chunks(usize::MAX);
                                            state.chunk_queue.enqueue_many(enq);
                                            state.vox_queue_len = state.chunk_queue.len();
                                            force_remesh_all(state);
                                            // debris instances
                                            for (i, p) in out.positions_m.iter().enumerate() {
                                                if (state.debris.len() as u32)
                                                    < state.debris_capacity
                                                {
                                                    let pos = glam::vec3(
                                                        p.x as f32, p.y as f32, p.z as f32,
                                                    );
                                                    let vel = out
                                                        .velocities_mps
                                                        .get(i)
                                                        .map(|v| {
                                                            glam::vec3(
                                                                v.x as f32, v.y as f32, v.z as f32,
                                                            )
                                                        })
                                                        .unwrap_or(glam::Vec3::Y * 2.5);
                                                    state.debris.push(crate::gfx::Debris {
                                                        pos,
                                                        vel,
                                                        age: 0.0,
                                                        life: 2.5,
                                                    });
                                                }
                                            }
                                            log::info!(
                                                "[onepath] raycast fallback hit r={:.2} debris+{}",
                                                radius_m,
                                                total_debris
                                            );
                                        } else {
                                            // As a last resort, scatter on the front face so we still remove material
                                            let o = grid.origin_m();
                                            let vm_f = grid.voxel_m().0 as f32;
                                            let _dims = grid.dims();
                                            let bmin = glam::vec3(
                                                o.x as f32 + 16.0 * vm_f,
                                                o.y as f32 + 0.0 * vm_f,
                                                o.z as f32 + 16.0 * vm_f,
                                            );
                                            let bmax = glam::vec3(
                                                o.x as f32 + 48.0 * vm_f,
                                                o.y as f32 + 20.0 * vm_f,
                                                o.z as f32 + 48.0 * vm_f,
                                            );
                                            let mut rng = state.destruct_cfg.seed ^ state.impact_id;
                                            let u = rand01(&mut rng);
                                            let v = rand01(&mut rng);
                                            let px = lerp(bmin.x + vm_f, bmax.x - vm_f, u);
                                            let py = lerp(bmin.y + vm_f, bmax.y - vm_f, v);
                                            // Distribute depth across thickness for coverage
                                            let z_layers = ((bmax.z - bmin.z) / vm_f).floor().max(1.0) as u32;
                                            let layer = (splitmix64(&mut rng) as u32) % z_layers;
                                            let pz = bmin.z + (layer as f32 + 0.5) * vm_f;
                                            let center =
                                                DVec3::new(px as f64, py as f64, pz as f64);
                                            let radius_m = (0.26 + 0.22 * rand01(&mut rng)) as f64;
                                            let seed = splitmix64(&mut rng);
                                            let out =
                                                server_core::destructible::carve_and_spawn_debris(
                                                    grid,
                                                    center,
                                                    core_units::Length::meters(radius_m),
                                                    seed,
                                                    state.impact_id,
                                                    state.destruct_cfg.max_debris,
                                                );
                                            // Spawn debris instances for scatter too
                                            for (i, p) in out.positions_m.iter().enumerate() {
                                                if (state.debris.len() as u32) < state.debris_capacity {
                                                    let pos = glam::vec3(p.x as f32, p.y as f32, p.z as f32);
                                                    let vel = out
                                                        .velocities_mps
                                                        .get(i)
                                                        .map(|v| glam::vec3(v.x as f32, v.y as f32, v.z as f32))
                                                        .unwrap_or(glam::Vec3::Y * 2.5);
                                                    state.debris.push(crate::gfx::Debris { pos, vel, age: 0.0, life: 2.5 });
                                                }
                                            }
                                            state.impact_id = state.impact_id.wrapping_add(1);
                                            let enq = grid.pop_dirty_chunks(usize::MAX);
                                            state.chunk_queue.enqueue_many(enq);
                                            state.vox_queue_len = state.chunk_queue.len();
                                            force_remesh_all(state);
                                            log::info!("[onepath] scatter fallback applied");
                                        }
                                    }
                                }
                            }
                            // end fallback
                            // Rebuild meshes regardless of whether the raycast hit or we used fallback
                            force_remesh_all(state);
                            self.script.shot = true;
                            self.script.carved = state.vox_queue_len > pre;
                            self.script.saved = false;
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyR) => {
                        if pressed {
                            reset_to_block(state);
                            self.script = Script::default();
                            log::info!("[onepath] reset block");
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyC) => {
                        // force carve fallback path
                        if pressed {
                            let pre = state.vox_queue_len;
                            let pre_debris = state.debris.len();
                            if let Some(ref mut grid) = state.voxel_grid {
                                // Carve a small sphere at grid center
                                let vm = grid.voxel_m().0;
                                let d = grid.dims();
                                let o = grid.origin_m();
                                let center = DVec3::new(
                                    o.x + vm * (d.x as f64 * 0.5),
                                    o.y + vm * (d.y as f64 * 0.5),
                                    o.z + vm * (d.z as f64 * 0.5),
                                );
                                // Per-impact randomization
                                let mut rng = state.destruct_cfg.seed
                                    ^ state.impact_id.wrapping_mul(0x9E37_79B9_7F4A_7C15);
                                let radius_m = lerp(0.22, 0.45, rand01(&mut rng)) as f64;
                                let debris_scale = lerp(0.60, 1.40, rand01(&mut rng));
                                let max_debris_hit =
                                    ((state.destruct_cfg.max_debris as f32 * debris_scale).round()
                                        as u32)
                                        .max(8);
                                let seed = splitmix64(&mut rng);
                                let out = server_core::destructible::carve_and_spawn_debris(
                                    grid,
                                    center,
                                    core_units::Length::meters(radius_m),
                                    seed,
                                    state.impact_id,
                                    max_debris_hit as usize,
                                );
                                state.impact_id = state.impact_id.wrapping_add(1);
                                let enq = grid.pop_dirty_chunks(usize::MAX);
                                state.chunk_queue.enqueue_many(enq);
                                state.vox_queue_len = state.chunk_queue.len();
                                for (i, p) in out.positions_m.iter().enumerate() {
                                    if (state.debris.len() as u32) < state.debris_capacity {
                                        let pos = glam::vec3(p.x as f32, p.y as f32, p.z as f32);
                                        let vel = out
                                            .velocities_mps
                                            .get(i)
                                            .map(|v| glam::vec3(v.x as f32, v.y as f32, v.z as f32))
                                            .unwrap_or(glam::Vec3::Y * 2.5);
                                        state.debris.push(crate::gfx::Debris {
                                            pos,
                                            vel,
                                            age: 0.0,
                                            life: 2.5,
                                        });
                                    }
                                }
                            }
                            // Demo-only: rebuild all chunk meshes immediately
                            force_remesh_all(state);
                            self.script.shot = true;
                            self.script.carved = state.vox_queue_len > pre;
                            self.script.saved = false;
                            log::info!(
                                "[onepath] forced center carve enq={} debris+{}",
                                state.vox_queue_len - pre,
                                state.debris.len().saturating_sub(pre_debris)
                            );
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyB) => {
                        // Burst demo mode: perform several raycast-guided hits along jittered camera rays
                        if pressed {
                            let hits = 5u32;
                            let mut enq_total = 0usize;
                            let start_debris = state.debris.len();
                            if let Some(ref mut grid) = state.voxel_grid {
                                // Camera ray base toward grid center
                                let aspect =
                                    state.size.width as f32 / state.size.height.max(1) as f32;
                                let (off, look) = camera_sys::compute_local_orbit_offsets(
                                    state.cam_distance,
                                    state.cam_orbit_yaw,
                                    state.cam_orbit_pitch,
                                    state.cam_lift,
                                    state.cam_look_height,
                                );
                                let (cam, _g) = camera_sys::third_person_follow(
                                    &mut state.cam_follow,
                                    state.scene_inputs.pos(),
                                    glam::Quat::IDENTITY,
                                    off,
                                    look,
                                    aspect,
                                    0.0,
                                );
                                let p0 = cam.eye;
                                let vm = grid.voxel_m().0 as f32;
                                let dims = grid.dims();
                                let origin = grid.origin_m();
                                let center = glam::vec3(
                                    (origin.x as f32) + (dims.x as f32 * vm * 0.5),
                                    (origin.y as f32) + (dims.y as f32 * vm * 0.5),
                                    (origin.z as f32) + (dims.z as f32 * vm * 0.5),
                                );
                                let base_dir = (center - p0).normalize_or_zero();
                                let ext = glam::vec3(
                                    dims.x as f32 * vm,
                                    dims.y as f32 * vm,
                                    dims.z as f32 * vm,
                                );
                                let max_len_m = core_units::Length::meters(ext.length() as f64);
                                let gmin =
                                    glam::vec3(origin.x as f32, origin.y as f32, origin.z as f32);
                                let gmax = gmin + ext;
                                let mut base_rng = state.destruct_cfg.seed ^ state.impact_id;
                                for _ in 0..hits {
                                    let mut r = splitmix64(&mut base_rng);
                                    // Small angular jitter
                                    let jitter = glam::vec3(
                                        (rand01(&mut r) - 0.5) * 0.15,
                                        (rand01(&mut r) - 0.5) * 0.15,
                                        0.0,
                                    );
                                    let dir = (base_dir + jitter).normalize_or_zero();
                                    if let Some((t_enter, _)) =
                                        ray_box_intersect(p0, dir, gmin, gmax)
                                    {
                                        let p_entry = p0 + dir * (t_enter + vm * 1e-3);
                                        let origin_m = DVec3::new(
                                            p_entry.x as f64,
                                            p_entry.y as f64,
                                            p_entry.z as f64,
                                        );
                                        let dir_m =
                                            DVec3::new(dir.x as f64, dir.y as f64, dir.z as f64);
                                        if let Some(hit) = server_core::destructible::raycast_voxels(
                                            grid, origin_m, dir_m, max_len_m,
                                        ) {
                                            let o = grid.origin_m();
                                            let vm = grid.voxel_m().0;
                                            let vc = DVec3::new(
                                                hit.voxel.x as f64 + 0.5,
                                                hit.voxel.y as f64 + 0.5,
                                                hit.voxel.z as f64 + 0.5,
                                            );
                                            let impact = o + vc * vm;
                                            let radius_m = (0.24 + 0.20 * rand01(&mut r)) as f64;
                                            let seed = splitmix64(&mut r);
                                            let out =
                                                server_core::destructible::carve_and_spawn_debris(
                                                    grid,
                                                    impact,
                                                    core_units::Length::meters(radius_m),
                                                    seed,
                                                    state.impact_id,
                                                    state.destruct_cfg.max_debris,
                                                );
                                            state.impact_id = state.impact_id.wrapping_add(1);
                                            let enq = grid.pop_dirty_chunks(usize::MAX);
                                            enq_total += enq.len();
                                            state.chunk_queue.enqueue_many(enq);
                                            for (i, p) in out.positions_m.iter().enumerate() {
                                                if (state.debris.len() as u32)
                                                    < state.debris_capacity
                                                {
                                                    let pos = glam::vec3(
                                                        p.x as f32, p.y as f32, p.z as f32,
                                                    );
                                                    let vel = out
                                                        .velocities_mps
                                                        .get(i)
                                                        .map(|v| {
                                                            glam::vec3(
                                                                v.x as f32, v.y as f32, v.z as f32,
                                                            )
                                                        })
                                                        .unwrap_or(glam::Vec3::Y * 2.5);
                                                    state.debris.push(crate::gfx::Debris {
                                                        pos,
                                                        vel,
                                                        age: 0.0,
                                                        life: 2.5,
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // Single immediate rebuild so all cuts appear together
                            force_remesh_all(state);
                            log::info!(
                                "[onepath] burst hits={} enq_total={} debris+{}",
                                hits,
                                enq_total,
                                state.debris.len().saturating_sub(start_debris)
                            );
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyP) => {
                        if pressed {
                            state.hud_model.toggle_perf();
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyS) => {
                        if pressed {
                            let path = PathBuf::from("target/vox_onepath.png");
                            let _ = save_screenshot(state, &path);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        if let Some(win) = &self.window {
            win.request_redraw();
        }
    }
}

fn save_screenshot(r: &mut Renderer, path: &PathBuf) -> Result<()> {
    // Read back the HDR scene color (Rgba16Float) and convert to RGBA8.
    let w = r.attachments.width;
    let h = r.attachments.height;
    let bytes_per_pixel = 8u32; // RGBA16F
    let unpadded = w * bytes_per_pixel;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded = unpadded.div_ceil(align) * align;
    let buf_size = (padded * h) as u64;
    let readback = r.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vox_onepath-readback"),
        size: buf_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = r
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("vox_onepath-enc"),
        });
    enc.copy_texture_to_buffer(
        r.attachments.scene_color.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    r.queue.submit([enc.finish()]);
    // Map and convert to RGBA8
    let slice = readback.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    // Kick the callback dispatch
    r.queue.submit(std::iter::empty());
    let _ = rx.recv();
    let data = slice.get_mapped_range();
    let mut out = vec![0u8; (w * h * 4) as usize];
    let mut idx8 = 0usize;
    for row in 0..h as usize {
        let start = row * padded as usize;
        let row_bytes = &data[start..start + unpadded as usize];
        // Each pixel: 4 * f16
        for px in 0..w as usize {
            let off = px * 8;
            let r16 = u16::from_le_bytes([row_bytes[off], row_bytes[off + 1]]);
            let g16 = u16::from_le_bytes([row_bytes[off + 2], row_bytes[off + 3]]);
            let b16 = u16::from_le_bytes([row_bytes[off + 4], row_bytes[off + 5]]);
            let a16 = u16::from_le_bytes([row_bytes[off + 6], row_bytes[off + 7]]);
            let rf = half::f16::from_bits(r16).to_f32().clamp(0.0, 1.0);
            let gf = half::f16::from_bits(g16).to_f32().clamp(0.0, 1.0);
            let bf = half::f16::from_bits(b16).to_f32().clamp(0.0, 1.0);
            let af = half::f16::from_bits(a16).to_f32().clamp(0.0, 1.0);
            out[idx8] = (rf * 255.0) as u8;
            out[idx8 + 1] = (gf * 255.0) as u8;
            out[idx8 + 2] = (bf * 255.0) as u8;
            out[idx8 + 3] = (af * 255.0) as u8;
            idx8 += 4;
        }
    }
    drop(data);
    readback.unmap();
    // Encode PNG
    std::fs::create_dir_all("target").ok();
    let file = std::fs::File::create(path)?;
    let mut enc = png::Encoder::new(file, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut wrt = enc.write_header()?;
    wrt.write_image_data(&out)?;
    Ok(())
}

fn reset_to_block(renderer: &mut Renderer) {
    // Procedural voxel block grid 1m ahead
    let dims = UVec3::new(64, 32, 64);
    let vm = renderer.destruct_cfg.voxel_size_m;
    let meta = VoxelProxyMeta {
        object_id: GlobalId(1),
        origin_m: DVec3::new(0.0, 0.0, 1.0),
        voxel_m: vm,
        dims,
        chunk: renderer.destruct_cfg.chunk,
        material: renderer.destruct_cfg.material,
    };
    let mut grid = VoxelGrid::new(meta);
    for z in 16..48 {
        for y in 0..20 {
            for x in 16..48 {
                grid.set(x, y, z, true);
            }
        }
    }
    renderer.voxel_meshes.clear();
    renderer.voxel_hashes.clear();
    renderer.debris.clear();
    renderer.debris_count = 0;
    renderer.voxel_grid = Some(grid.clone());
    renderer.voxel_grid_initial = Some(grid);
    renderer.vox_onepath_ui = Some((false, false, false));
    // Demo-only: rebuild all chunk meshes immediately
    force_remesh_all(renderer);
}

// Demo-only: rebuild all chunk meshes from the CPU grid immediately and upload to GPU
fn force_remesh_all(r: &mut Renderer) {
    use wgpu::util::DeviceExt as _;
    let Some(grid) = r.voxel_grid.as_ref() else {
        return;
    };

    // Clear existing GPU meshes and hashes so we don't draw stale buffers
    r.voxel_meshes.clear();
    r.voxel_hashes.clear();

    // Iterate every chunk and build a fresh mesh with the simple (naive) mesher
    let dims = grid.dims();
    let csz = grid.meta().chunk;
    let nx = dims.x.div_ceil(csz.x);
    let ny = dims.y.div_ceil(csz.y);
    let nz = dims.z.div_ceil(csz.z);

    for cz in 0..nz {
        for cy in 0..ny {
            for cx in 0..nx {
                let c = glam::UVec3::new(cx, cy, cz);
                let mb = voxel_mesh::naive_mesh_chunk(grid, c);

                // Interleave to match gfx::types::Vertex { pos, nrm }
                let mut verts: Vec<crate::gfx::types::Vertex> =
                    Vec::with_capacity(mb.positions.len());
                for (i, p) in mb.positions.iter().enumerate() {
                    let n = mb.normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
                    verts.push(crate::gfx::types::Vertex { pos: *p, nrm: n });
                }

                let vb = r
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("vox_onepath-chunk-vb"),
                        contents: bytemuck::cast_slice(&verts),
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    });
                let ib = r
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("vox_onepath-chunk-ib"),
                        contents: bytemuck::cast_slice(&mb.indices),
                        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    });

                r.voxel_meshes.insert(
                    (cx, cy, cz),
                    crate::gfx::VoxelChunkMesh {
                        vb,
                        ib,
                        idx: mb.indices.len() as u32,
                    },
                );
            }
        }
    }

    // Overlay counters: no queued work when we rebuild synchronously
    r.vox_last_chunks = 0;
    r.vox_queue_len = 0;
}
