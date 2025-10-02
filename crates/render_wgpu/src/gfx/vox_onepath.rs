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
struct Script { shot: bool, carved: bool, saved: bool }

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

        // Build a procedural voxel block grid (64x32x64), origin 6m forward.
        let dims = UVec3::new(64, 32, 64);
        let vm = renderer.destruct_cfg.voxel_size_m;
        let meta = VoxelProxyMeta {
            object_id: GlobalId(1),
            origin_m: DVec3::new(0.0, 0.0, 6.0),
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

        // Pre-mesh synchronously so the first frame shows geometry
        let saved_budget = renderer.destruct_cfg.max_chunk_remesh;
        renderer.destruct_cfg.max_chunk_remesh = 64;
        while renderer.vox_queue_len > 0 {
            renderer.process_voxel_queues();
        }
        renderer.destruct_cfg.max_chunk_remesh = saved_budget;

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
                            let p1 = p0 + (cam.target - cam.eye).normalize_or_zero() * 10.0;
                            let pre = state.vox_queue_len;
                            state.try_voxel_impact(p0, p1);
                            self.script.shot = true;
                            self.script.carved = state.vox_queue_len > pre;
                            self.script.saved = false;
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyR) => {
                        if pressed {
                            reset_to_block(state);
                            self.script = Script::default();
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyP) => {
                        if pressed { state.hud_model.toggle_perf(); }
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
    let padded = unpadded.div_ceil(align);
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
    // Procedural voxel block grid 6m ahead
    let dims = UVec3::new(64, 32, 64);
    let vm = renderer.destruct_cfg.voxel_size_m;
    let meta = VoxelProxyMeta {
        object_id: GlobalId(1),
        origin_m: DVec3::new(0.0, 0.0, 6.0),
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
    // Enqueue all chunks
    let dims = renderer.voxel_grid.as_ref().unwrap().dims();
    let csz = renderer.voxel_grid.as_ref().unwrap().meta().chunk;
    let nx = dims.x.div_ceil(csz.x);
    let ny = dims.y.div_ceil(csz.y);
    let nz = dims.z.div_ceil(csz.z);
    renderer.chunk_queue = server_core::destructible::queue::ChunkQueue::new();
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
    // Pre-mesh synchronously
    let saved_budget = renderer.destruct_cfg.max_chunk_remesh;
    renderer.destruct_cfg.max_chunk_remesh = 64;
    while renderer.vox_queue_len > 0 {
        renderer.process_voxel_queues();
    }
    renderer.destruct_cfg.max_chunk_remesh = saved_budget;
}
