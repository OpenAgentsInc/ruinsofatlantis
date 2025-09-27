//! gfx: minimal rendering module for the prototype client
//!
//! This module wraps winit/wgpu initialization and draws a very simple scene:
//! - A large ground plane
//! - An instanced grid of small "shards" (cubes)
//!
//! It is deliberately split into focused files so the structure resembles a
//! real codebase you could extend into an MMORPG client.
//!
//! Files
//! - camera.rs: Camera type and view/projection helpers
//! - types.rs: POD buffer structs and vertex layouts (Globals/Model/Vertex/Instance)
//! - mesh.rs: CPU-side mesh helpers (cube + plane)
//! - pipeline.rs: Pipeline and bind-group creation + shader module (WGSL stored in shader.wgsl)
//! - util.rs: small helpers (clamp surface size while preserving aspect)

mod camera;
mod mesh;
mod pipeline;
mod types;
pub use types::Vertex;
mod anim;
mod camera_sys;
mod draw;
pub mod fx;
mod material;
mod scene;
mod sky;
pub mod terrain;
mod ui;
mod util;

use crate::assets::skinning::merge_gltf_animations;
use crate::assets::{
    AnimClip, SkinnedMeshCPU, TrackQuat, TrackVec3, load_gltf_mesh, load_gltf_skinned,
    load_obj_mesh,
};
use crate::core::data::{
    loader as data_loader,
    spell::SpellSpec,
    zone::{ZoneManifest, load_zone_manifest},
};
// (scene building now encapsulated; ECS types unused here)
use anyhow::Context;
use types::{Globals, InstanceSkin, Model, ParticleInstance, VertexSkinned};
use util::scale_to_max;

use std::time::Instant;

use wgpu::{
    SurfaceError, SurfaceTargetUnsafe, rwh::HasDisplayHandle, rwh::HasWindowHandle, util::DeviceExt,
};
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

fn asset_path(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

use fx::{Particle, Projectile};

/// Renderer owns the GPU state and per‑scene resources.
///
/// The intent is that a higher‑level game loop owns a `Renderer` and calls
/// `resize` and `render` based on window events.
pub struct Renderer {
    // --- GPU & Surface ---
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    max_dim: u32,
    depth: wgpu::TextureView,

    // --- Pipelines & BGLs ---
    pipeline: wgpu::RenderPipeline,
    inst_pipeline: wgpu::RenderPipeline,
    wire_pipeline: Option<wgpu::RenderPipeline>,
    particle_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    globals_bg: wgpu::BindGroup,
    sky_bg: wgpu::BindGroup,
    terrain_model_bg: wgpu::BindGroup,
    shard_model_bg: wgpu::BindGroup,

    // --- Scene Buffers ---
    globals_buf: wgpu::Buffer,
    sky_buf: wgpu::Buffer,
    _plane_model_buf: wgpu::Buffer,
    shard_model_buf: wgpu::Buffer,

    // Geometry (terrain)
    terrain_vb: wgpu::Buffer,
    terrain_ib: wgpu::Buffer,
    terrain_index_count: u32,

    // GLTF geometry (wizard + ruins)
    wizard_vb: wgpu::Buffer,
    wizard_ib: wgpu::Buffer,
    wizard_index_count: u32,
    // Zombie skinned geometry
    zombie_vb: wgpu::Buffer,
    zombie_ib: wgpu::Buffer,
    zombie_index_count: u32,
    ruins_vb: wgpu::Buffer,
    ruins_ib: wgpu::Buffer,
    ruins_index_count: u32,

    // NPC cubes
    npc_vb: wgpu::Buffer,
    npc_ib: wgpu::Buffer,
    npc_index_count: u32,
    npc_instances: wgpu::Buffer,
    npc_count: u32,
    #[allow(dead_code)]
    npc_instances_cpu: Vec<types::Instance>,
    #[allow(dead_code)]
    npc_models: Vec<glam::Mat4>,

    // Vegetation (trees) — instanced cubes for now
    trees_instances: wgpu::Buffer,
    trees_count: u32,
    trees_vb: wgpu::Buffer,
    trees_ib: wgpu::Buffer,
    trees_index_count: u32,

    // Instancing buffers
    wizard_instances: wgpu::Buffer,
    wizard_count: u32,
    zombie_instances: wgpu::Buffer,
    zombie_count: u32,
    zombie_instances_cpu: Vec<InstanceSkin>,
    ruins_instances: wgpu::Buffer,
    ruins_count: u32,

    // FX buffers
    fx_instances: wgpu::Buffer,
    _fx_capacity: u32,
    fx_count: u32,
    _fx_model_bg: wgpu::BindGroup,
    quad_vb: wgpu::Buffer,

    // Wizard skinning palettes
    palettes_buf: wgpu::Buffer,
    palettes_bg: wgpu::BindGroup,
    joints_per_wizard: u32,
    wizard_models: Vec<glam::Mat4>,
    wizard_instances_cpu: Vec<InstanceSkin>,
    // Zombies
    zombie_palettes_buf: wgpu::Buffer,
    zombie_palettes_bg: wgpu::BindGroup,
    zombie_joints: u32,
    #[allow(dead_code)]
    zombie_models: Vec<glam::Mat4>,
    zombie_cpu: SkinnedMeshCPU,
    zombie_time_offset: Vec<f32>,
    zombie_ids: Vec<crate::server::NpcId>,
    zombie_prev_pos: Vec<glam::Vec3>,
    // Some skinned assets have a different authoring "forward" axis. We detect the
    // root-bone yaw at import and keep a correction so top-level yaw aligns with
    // world +Z forward when turning toward velocity.
    zombie_forward_offset: f32,

    // Wizard pipelines
    wizard_pipeline: wgpu::RenderPipeline,

    wizard_mat_bg: wgpu::BindGroup,
    _wizard_mat_buf: wgpu::Buffer,
    _wizard_tex_view: wgpu::TextureView,
    _wizard_sampler: wgpu::Sampler,
    zombie_mat_bg: wgpu::BindGroup,
    _zombie_mat_buf: wgpu::Buffer,
    _zombie_tex_view: wgpu::TextureView,
    _zombie_sampler: wgpu::Sampler,

    // Flags
    wire_enabled: bool,

    // Sky/time-of-day state
    sky: sky::SkyStateCPU,

    // Terrain sampler (CPU)
    terrain_cpu: terrain::TerrainCPU,

    // Time base for animation
    start: Instant,
    last_time: f32,

    // Wizard animation selection and time offsets
    wizard_anim_index: Vec<usize>,
    wizard_time_offset: Vec<f32>,

    // CPU-side skinned mesh data
    skinned_cpu: SkinnedMeshCPU,

    // Animation-driven VFX
    wizard_last_phase: Vec<f32>,
    hand_right_node: Option<usize>,
    #[allow(dead_code)]
    root_node: Option<usize>,

    // Projectile + particle pools
    projectiles: Vec<Projectile>,
    particles: Vec<Particle>,

    // Data-driven spec
    fire_bolt: Option<SpellSpec>,

    // Camera focus (we orbit around a close wizard)

    // UI overlay
    nameplates: ui::Nameplates,
    nameplates_npc: ui::Nameplates,
    bars: ui::HealthBars,
    damage: ui::DamageFloaters,

    // --- Player/Camera ---
    pc_index: usize,
    player: crate::client::controller::PlayerController,
    input: crate::client::input::InputState,
    cam_follow: camera_sys::FollowState,
    pc_cast_queued: bool,
    pc_anim_start: Option<f32>,
    // Orbit params
    cam_orbit_yaw: f32,
    cam_orbit_pitch: f32,
    cam_distance: f32,
    cam_lift: f32,
    cam_look_height: f32,
    rmb_down: bool,
    last_cursor_pos: Option<(f64, f64)>,

    // Server state (NPCs/health)
    server: crate::server::ServerState,

    // Wizard health (including PC at pc_index)
    wizard_hp: Vec<i32>,
    wizard_hp_max: i32,
    pc_alive: bool,
}

impl Renderer {
    fn any_zombies_alive(&self) -> bool {
        self.server.npcs.iter().any(|n| n.alive)
    }
    /// Handle player character death: hide visuals, disable input/casting,
    /// and keep camera in a spectator orbit around the last position.
    fn kill_pc(&mut self) {
        if !self.pc_alive {
            return;
        }
        self.pc_alive = false;
        if let Some(hp) = self.wizard_hp.get_mut(self.pc_index) {
            *hp = 0;
        }
        self.pc_cast_queued = false;
        self.input.clear();
        // Move PC far off-screen to avoid AI targeting and hide the model by scaling it down.
        // Keep instance slot to avoid reindexing other wizards; UI bars already omit 0 HP.
        if self.pc_index < self.wizard_models.len() {
            let hide_pos = glam::vec3(1.0e6, -1.0e6, 1.0e6);
            let m = glam::Mat4::from_scale_rotation_translation(
                glam::Vec3::splat(0.0001),
                glam::Quat::IDENTITY,
                hide_pos,
            );
            self.wizard_models[self.pc_index] = m;
            if self.pc_index < self.wizard_instances_cpu.len() {
                let mut inst = self.wizard_instances_cpu[self.pc_index];
                inst.model = m.to_cols_array_2d();
                self.wizard_instances_cpu[self.pc_index] = inst;
                let offset = (self.pc_index * std::mem::size_of::<InstanceSkin>()) as u64;
                self.queue
                    .write_buffer(&self.wizard_instances, offset, bytemuck::bytes_of(&inst));
            }
        }
        log::info!("PC died; spectator camera engaged");
    }
    fn remove_wizard_at(&mut self, idx: usize) {
        if idx >= self.wizard_count as usize {
            return;
        }
        // Keep PC for now; skip removal if it's the player character to avoid breaking input/camera
        if idx == self.pc_index {
            return;
        }
        self.wizard_models.swap_remove(idx);
        self.wizard_instances_cpu.swap_remove(idx);
        self.wizard_anim_index.swap_remove(idx);
        self.wizard_time_offset.swap_remove(idx);
        self.wizard_last_phase.swap_remove(idx);
        self.wizard_hp.swap_remove(idx);
        // If swap removed moved the old PC index, adjust pc_index
        if self.pc_index == self.wizard_count as usize - 1 && idx < self.pc_index {
            // if PC was last element and we removed a lower index, PC index shifts down by 1
            self.pc_index -= 1;
        } else if idx < self.pc_index {
            self.pc_index -= 1;
        }
        // Recompute palette_base for contiguous layout
        for (i, inst) in self.wizard_instances_cpu.iter_mut().enumerate() {
            inst.palette_base = (i as u32) * self.joints_per_wizard;
        }
        self.wizard_count = self.wizard_instances_cpu.len() as u32;
        // Upload full instances buffer
        let bytes: &[u8] = bytemuck::cast_slice(&self.wizard_instances_cpu);
        self.queue.write_buffer(&self.wizard_instances, 0, bytes);
    }
    /// Create a renderer bound to a window surface.
    pub async fn new(window: &Window) -> anyhow::Result<Self> {
        // --- Instance + Surface + Adapter (with backend fallback) ---
        fn backend_from_env() -> Option<wgpu::Backends> {
            match std::env::var("RA_BACKEND").ok().as_deref() {
                Some("vulkan" | "VULKAN" | "vk") => Some(wgpu::Backends::VULKAN),
                Some("gl" | "GL" | "opengl") => Some(wgpu::Backends::GL),
                Some("primary" | "PRIMARY" | "all") => Some(wgpu::Backends::PRIMARY),
                _ => None,
            }
        }
        let candidates: &[wgpu::Backends] = if let Some(b) = backend_from_env() {
            if b == wgpu::Backends::PRIMARY {
                &[wgpu::Backends::PRIMARY]
            } else {
                &[b, wgpu::Backends::PRIMARY]
            }
        } else if cfg!(target_os = "linux") {
            &[
                wgpu::Backends::VULKAN,
                wgpu::Backends::GL,
                wgpu::Backends::PRIMARY,
            ]
        } else {
            &[wgpu::Backends::PRIMARY]
        };

        // Create a surface per candidate instance and try to get an adapter
        let raw_display = window.display_handle()?.as_raw();
        let raw_window = window.window_handle()?.as_raw();
        let (_instance, surface, adapter) = {
            let mut picked: Option<(wgpu::Instance, wgpu::Surface<'static>, wgpu::Adapter)> = None;
            for &bmask in candidates {
                let inst = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                    backends: bmask,
                    flags: wgpu::InstanceFlags::empty(),
                    ..Default::default()
                });
                let surf = unsafe {
                    inst.create_surface_unsafe(SurfaceTargetUnsafe::RawHandle {
                        raw_display_handle: raw_display,
                        raw_window_handle: raw_window,
                    })
                }
                .context("create wgpu surface (unsafe)")?;
                match inst
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        compatible_surface: Some(&surf),
                        power_preference: wgpu::PowerPreference::HighPerformance,
                        force_fallback_adapter: false,
                    })
                    .await
                {
                    Ok(a) => {
                        picked = Some((inst, surf, a));
                        break;
                    }
                    Err(_) => {
                        // try next backend mask
                    }
                }
            }
            picked.ok_or_else(|| {
                anyhow::anyhow!("no suitable GPU adapter across backends {:?}", candidates)
            })?
        };

        let mut req_features = wgpu::Features::empty();
        if adapter
            .features()
            .contains(wgpu::Features::POLYGON_MODE_LINE)
        {
            req_features |= wgpu::Features::POLYGON_MODE_LINE;
        }
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("wgpu-device"),
                required_features: req_features,
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::default(),
            })
            .await
            .context("request device")?;

        // --- Surface configuration (with clamping to device limits) ---
        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let present_mode = caps
            .present_modes
            .iter()
            .copied()
            .find(|m| *m == wgpu::PresentMode::Mailbox)
            .unwrap_or(wgpu::PresentMode::Fifo);
        let alpha_mode = caps.alpha_modes[0];
        let max_dim = device.limits().max_texture_dimension_2d.clamp(1, 2048);
        let (w, h) = scale_to_max((size.width, size.height), max_dim);
        if (w, h) != (size.width, size.height) {
            log::warn!(
                "Clamping surface from {}x{} to {}x{} (max_dim={})",
                size.width,
                size.height,
                w,
                h,
                max_dim
            );
        }
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: w,
            height: h,
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        let depth = util::create_depth_view(&device, config.width, config.height, config.format);

        // --- Pipelines + BGLs ---
        let shader = pipeline::create_shader(&device);
        let (globals_bgl, model_bgl) = pipeline::create_bind_group_layouts(&device);
        let palettes_bgl = pipeline::create_palettes_bgl(&device);
        let material_bgl = pipeline::create_material_bgl(&device);
        let (pipeline, inst_pipeline, wire_pipeline) =
            pipeline::create_pipelines(&device, &shader, &globals_bgl, &model_bgl, config.format);
        // Sky background
        let sky_bgl = pipeline::create_sky_bgl(&device);
        let sky_pipeline =
            pipeline::create_sky_pipeline(&device, &globals_bgl, &sky_bgl, config.format);
        let (wizard_pipeline, _wizard_wire_pipeline_unused) = pipeline::create_wizard_pipelines(
            &device,
            &shader,
            &globals_bgl,
            &model_bgl,
            &palettes_bgl,
            &material_bgl,
            config.format,
        );
        let particle_pipeline =
            pipeline::create_particle_pipeline(&device, &shader, &globals_bgl, config.format);

        // UI: nameplates + health bars
        let nameplates = ui::Nameplates::new(&device, config.format)?;
        let nameplates_npc = ui::Nameplates::new(&device, config.format)?;
        let bars = ui::HealthBars::new(&device, config.format)?;
        let damage = ui::DamageFloaters::new(&device, config.format)?;

        // --- Buffers & bind groups ---
        // Globals
        let globals_init = Globals {
            view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
            cam_right_time: [1.0, 0.0, 0.0, 0.0],
            cam_up_pad: [0.0, 1.0, 0.0, 0.0],
            sun_dir_time: [0.0, 1.0, 0.0, 0.0],
            sh_coeffs: [[0.0, 0.0, 0.0, 0.0]; 9],
            fog_params: [0.0, 0.0, 0.0, 0.0],
        };
        let globals_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globals"),
            contents: bytemuck::bytes_of(&globals_init),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals-bg"),
            layout: &globals_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buf.as_entire_binding(),
            }],
        });

        // Sky uniforms
        // Load Zone manifest for the wizard demo scene
        let zone: ZoneManifest =
            load_zone_manifest("wizard_woods").context("load zone manifest: wizard_woods")?;
        log::info!(
            "Zone '{}' (id={}, plane={:?})",
            zone.display_name,
            zone.zone_id,
            zone.plane
        );
        // Sky/time-of-day state with zone weather defaults
        let mut sky_state = sky::SkyStateCPU::new();
        if let Some(w) = zone.weather {
            sky_state.weather = crate::gfx::sky::Weather {
                turbidity: w.turbidity,
                ground_albedo: w.ground_albedo,
            };
            sky_state.recompute();
        }
        let sky_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sky-uniform"),
            contents: bytemuck::bytes_of(&sky_state.sky_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let sky_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sky-bg"),
            layout: &sky_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: sky_buf.as_entire_binding(),
            }],
        });

        // Per-draw Model buffers (plane and shard base)
        // Nudge the plane slightly downward to avoid z-fighting/overlap with wizard feet.
        let plane_model_init = Model {
            model: glam::Mat4::from_translation(glam::vec3(0.0, -0.05, 0.0)).to_cols_array_2d(),
            color: [0.10, 0.55, 0.25],
            emissive: 0.0,
            _pad: [0.0; 4],
        };
        let plane_model_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain-model"),
            contents: bytemuck::bytes_of(&plane_model_init),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let plane_model_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain-model-bg"),
            layout: &model_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: plane_model_buf.as_entire_binding(),
            }],
        });

        let shard_model_init = Model {
            model: glam::Mat4::IDENTITY.to_cols_array_2d(),
            color: [0.85, 0.15, 0.15],
            emissive: 0.15,
            _pad: [0.0; 4],
        };
        let shard_model_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("shard-model"),
            contents: bytemuck::bytes_of(&shard_model_init),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let shard_model_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shard-model-bg"),
            layout: &model_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: shard_model_buf.as_entire_binding(),
            }],
        });

        // Terrain (replaces single ground plane) — prefer baked snapshot, else generate from Zone
        let terrain_extent = zone.terrain.extent;
        let terrain_size = zone.terrain.size as usize; // e.g., 129 → 128x128 quads
        let (terrain_cpu, terrain_bufs) =
            if let Some(cpu) = terrain::load_terrain_snapshot("wizard_woods") {
                let bufs = terrain::upload_from_cpu(&device, &cpu);
                (cpu, bufs)
            } else {
                terrain::create_terrain(&device, terrain_size, terrain_extent, zone.terrain.seed)
            };
        let terrain_vb = terrain_bufs.vb;
        let terrain_ib = terrain_bufs.ib;
        let terrain_index_count = terrain_bufs.index_count;

        // --- Load GLTF assets into CPU meshes, then upload to GPU buffers ---
        let skinned_cpu = load_gltf_skinned(&asset_path("assets/models/wizard.gltf"))
            .context("load skinned wizard.gltf")?;
        // Load the original project zombie model
        let zombie_model_path = "assets/models/zombie.glb";
        let mut zombie_cpu = load_gltf_skinned(&asset_path(zombie_model_path))
            .with_context(|| format!("load skinned {}", zombie_model_path))?;
        {
            let count = zombie_cpu.animations.len();
            let mut names: Vec<&str> = zombie_cpu.animations.keys().map(|s| s.as_str()).collect();
            names.sort_unstable();
            log::info!("{} animations: {} -> {:?}", zombie_model_path, count, names);
        }
        // Optional external clips in assets/models/zombie_clips/*.glb
        for (_alias, file) in [
            ("Idle", "idle.glb"),
            ("Walk", "walk.glb"),
            ("Run", "run.glb"),
            ("Attack", "attack.glb"),
        ] {
            let p = asset_path(&format!("assets/models/zombie_clips/{}", file));
            if p.exists() {
                let before = zombie_cpu.animations.len();
                if let Ok(n) = merge_gltf_animations(&mut zombie_cpu, &p) {
                    let after = zombie_cpu.animations.len();
                    log::info!(
                        "merged {} clips from {} ({} -> {})",
                        n,
                        p.display(),
                        before,
                        after
                    );
                }
            }
        }
        let ruins_cpu_res = load_gltf_mesh(&asset_path("assets/models/ruins.gltf"));
        // Determine a base offset so the lowest vertex sits on ground, with a small embed.
        let (ruins_base_offset, ruins_radius): (f32, f32) = match &ruins_cpu_res {
            Ok(cpu) => {
                let mut min_y = f32::INFINITY;
                let mut min_x = f32::INFINITY;
                let mut max_x = f32::NEG_INFINITY;
                let mut min_z = f32::INFINITY;
                let mut max_z = f32::NEG_INFINITY;
                for v in &cpu.vertices {
                    min_y = min_y.min(v.pos[1]);
                    min_x = min_x.min(v.pos[0]);
                    max_x = max_x.max(v.pos[0]);
                    min_z = min_z.min(v.pos[2]);
                    max_z = max_z.max(v.pos[2]);
                }
                let sx = (max_x - min_x).abs();
                let sz = (max_z - min_z).abs();
                let radius = 0.5 * sx.max(sz);
                // Embed slightly to avoid hovering
                ((-min_y) - 0.05, radius)
            }
            Err(_) => (0.6, 6.0),
        };

        // For robustness, pull UVs from a straightforward glTF read (same primitive as viewer)
        // and override the UVs we got from the skinned loader if the counts match. This
        // sidesteps any subtle attribute mismatches that can lead to banding.
        let viewer_uv: Option<Vec<[f32; 2]>> = (|| {
            let (doc, buffers, _images) =
                gltf::import(asset_path("assets/models/wizard.gltf")).ok()?;
            let mesh = doc.meshes().next()?;
            let prim = mesh.primitives().next()?;
            let reader = prim.reader(|b| buffers.get(b.index()).map(|bb| bb.0.as_slice()));
            let uv_set = prim
                .material()
                .pbr_metallic_roughness()
                .base_color_texture()
                .map(|ti| ti.tex_coord())
                .unwrap_or(0);
            let uv = reader
                .read_tex_coords(uv_set)?
                .into_f32()
                .collect::<Vec<[f32; 2]>>();
            Some(uv)
        })();

        let wiz_vertices: Vec<VertexSkinned> = skinned_cpu
            .vertices
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let uv = if let Some(ref uvs) = viewer_uv {
                    uvs.get(i).copied().unwrap_or(v.uv)
                } else {
                    v.uv
                };
                VertexSkinned {
                    pos: v.pos,
                    nrm: v.nrm,
                    joints: v.joints,
                    weights: v.weights,
                    uv,
                }
            })
            .collect();

        let wizard_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("wizard-vb"),
            contents: bytemuck::cast_slice(&wiz_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let wizard_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("wizard-ib"),
            contents: bytemuck::cast_slice(&skinned_cpu.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let wizard_index_count = skinned_cpu.indices.len() as u32;

        // Zombie vertex buffer (use same VertexSkinned layout)
        let zom_vertices: Vec<VertexSkinned> = zombie_cpu
            .vertices
            .iter()
            .map(|v| VertexSkinned {
                pos: v.pos,
                nrm: v.nrm,
                joints: v.joints,
                weights: v.weights,
                uv: v.uv,
            })
            .collect();
        let zombie_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("zombie-vb"),
            contents: bytemuck::cast_slice(&zom_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let zombie_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("zombie-ib"),
            contents: bytemuck::cast_slice(&zombie_cpu.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let zombie_index_count = zombie_cpu.indices.len() as u32;

        let (ruins_vb, ruins_ib, ruins_index_count) = match ruins_cpu_res {
            Ok(ruins_cpu) => {
                let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("ruins-vb"),
                    contents: bytemuck::cast_slice(&ruins_cpu.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("ruins-ib"),
                    contents: bytemuck::cast_slice(&ruins_cpu.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
                (vb, ib, ruins_cpu.indices.len() as u32)
            }
            Err(e) => return Err(anyhow::anyhow!("failed to load ruins model: {e}")),
        };

        // Build scene instance buffers and camera target
        let scene_build = scene::build_demo_scene(
            &device,
            &skinned_cpu,
            terrain_extent,
            Some(&terrain_cpu),
            ruins_base_offset,
            ruins_radius,
        );

        // Snap initial wizard ring to terrain height
        let mut wizard_models = scene_build.wizard_models.clone();
        for m in &mut wizard_models {
            let c = m.to_cols_array();
            let x = c[12];
            let z = c[14];
            let (h, _n) = terrain::height_at(&terrain_cpu, x, z);
            let pos = glam::vec3(x, h, z);
            let (s, r, _t) = glam::Mat4::from_cols_array(&c).to_scale_rotation_translation();
            *m = glam::Mat4::from_scale_rotation_translation(s, r, pos);
        }
        let mut wizard_instances_cpu = scene_build.wizard_instances_cpu.clone();
        for (i, inst) in wizard_instances_cpu.iter_mut().enumerate() {
            let m = wizard_models[i].to_cols_array_2d();
            inst.model = m;
        }
        let wizard_instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("wizard-instances"),
            contents: bytemuck::cast_slice(&wizard_instances_cpu),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        // Precompute PC initial position from the soon-to-be-moved vector
        let pc_initial_pos = {
            let m = scene_build.wizard_models[scene_build.pc_index];
            let c = m.to_cols_array();
            glam::vec3(c[12], c[13], c[14])
        };
        // Upload text atlases once now that we have a queue
        nameplates.upload_atlas(&queue);
        nameplates_npc.upload_atlas(&queue);
        damage.upload_atlas(&queue);
        // FX resources
        let fx_res = fx::create_fx_resources(&device, &model_bgl);
        let fx_instances = fx_res.instances;
        let fx_model_bg = fx_res.model_bg;
        let quad_vb = fx_res.quad_vb;
        let fx_capacity = fx_res.capacity;
        let fx_count: u32 = 0;
        // Load Fire Bolt spec (optional)
        let fire_bolt = data_loader::load_spell_spec("spells/fire_bolt.json").ok();
        // Precompute strike times (or leave empty to use periodic fallback)
        let hand_right_node = skinned_cpu.hand_right_node;
        let root_node = skinned_cpu.root_node;
        let _strikes_tmp =
            anim::compute_portalopen_strikes(&skinned_cpu, hand_right_node, root_node);
        // Camera target: follow the PC (center wizard)

        // Allocate storage for skinning palettes: one palette per wizard
        let total_mats = scene_build.wizard_count as usize * scene_build.joints_per_wizard as usize;
        let palettes_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("palettes"),
            size: (total_mats * 64) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let palettes_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("palettes-bg"),
            layout: &palettes_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &palettes_buf,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        // Zombie joints count (palettes allocated after instances are built)
        let zombie_joints = zombie_cpu.joints_nodes.len() as u32;

        let material_res =
            material::create_wizard_material(&device, &queue, &material_bgl, &skinned_cpu);
        let wizard_mat_bg = material_res.bind_group;
        let _wizard_mat_buf = material_res.uniform_buf;
        let _wizard_tex_view = material_res.texture_view;
        let _wizard_sampler = material_res.sampler;

        // Zombie material
        let zmat = material::create_wizard_material(&device, &queue, &material_bgl, &zombie_cpu);
        let zombie_mat_bg = zmat.bind_group;
        let _zombie_mat_buf = zmat.uniform_buf;
        let _zombie_tex_view = zmat.texture_view;
        let _zombie_sampler = zmat.sampler;

        // NPCs: simple cubes as targets on multiple rings
        let (npc_vb, npc_ib, npc_index_count) = mesh::create_cube(&device);
        let mut server = crate::server::ServerState::new();
        // Configure ring distances and counts (keep existing ones, add more)
        // Reduce zombies ~25% overall by lowering ring counts
        let near_count = 8usize; // was 10
        let near_radius = 15.0f32;
        let mid1_count = 12usize; // was 16
        let mid1_radius = 30.0f32;
        let mid2_count = 15usize; // was 20
        let mid2_radius = 45.0f32;
        let mid3_count = 18usize; // was 24
        let mid3_radius = 60.0f32;
        let far_count = 9usize; // was 12
        let far_radius = terrain_extent * 0.7;
        // Spawn rings (hp scales mildly with distance)
        server.ring_spawn(near_count, near_radius, 20);
        server.ring_spawn(mid1_count, mid1_radius, 25);
        server.ring_spawn(mid2_count, mid2_radius, 30);
        server.ring_spawn(mid3_count, mid3_radius, 35);
        server.ring_spawn(far_count, far_radius, 30);
        let mut npc_instances_cpu: Vec<types::Instance> = Vec::new();
        let mut npc_models: Vec<glam::Mat4> = Vec::new();
        for npc in &server.npcs {
            let m = glam::Mat4::from_scale_rotation_translation(
                glam::Vec3::splat(1.2),
                glam::Quat::IDENTITY,
                npc.pos,
            );
            npc_models.push(m);
            npc_instances_cpu.push(types::Instance {
                model: m.to_cols_array_2d(),
                color: [0.75, 0.2, 0.2],
                selected: 0.0,
            });
        }
        let npc_instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("npc-instances"),
            contents: bytemuck::cast_slice(&npc_instances_cpu),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        // Trees: prefer baked instances if available, else scatter using manifest vegetation params
        let trees_models_opt = terrain::load_trees_snapshot("wizard_woods");
        let trees_instances_cpu: Vec<types::Instance> = if let Some(models) = &trees_models_opt {
            terrain::instances_from_models(models)
        } else {
            let (tree_count, tree_seed) = zone
                .vegetation
                .as_ref()
                .map(|v| (v.tree_count as usize, v.tree_seed))
                .unwrap_or((350usize, 20250926u32));
            terrain::place_trees(&terrain_cpu, tree_count, tree_seed)
        };
        let trees_count = trees_instances_cpu.len() as u32;
        let trees_instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("trees-instances"),
            contents: bytemuck::cast_slice(&trees_instances_cpu),
            usage: wgpu::BufferUsages::VERTEX,
        });
        // Load a static tree mesh (OBJ) and upload
        let tree_mesh_path = asset_path("assets/models/trees/OBJ/Tree_3.obj");
        let tree_mesh_cpu = if tree_mesh_path.exists() {
            // If OBJ not vendored yet, fall back to cube
            load_obj_mesh(&tree_mesh_path).context("load OBJ tree mesh")?
        } else {
            log::warn!("tree OBJ not found; falling back to cube mesh for trees");
            // Build a CpuMesh from the cube VB/IB? Simpler: reuse cube buffers
            // We'll just keep using cube buffers when OBJ is missing.
            // Placeholder buffers (will be overwritten by npc_vb/ib below if missing)
            crate::assets::CpuMesh {
                vertices: vec![],
                indices: vec![],
            }
        };
        let (trees_vb, trees_ib, trees_index_count) = if !tree_mesh_cpu.vertices.is_empty() {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("trees-vb"),
                contents: bytemuck::cast_slice(&tree_mesh_cpu.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("trees-ib"),
                contents: bytemuck::cast_slice(&tree_mesh_cpu.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            (vb, ib, tree_mesh_cpu.indices.len() as u32)
        } else {
            // Fallback: reuse the cube geometry
            (npc_vb.clone(), npc_ib.clone(), npc_index_count)
        };
        // If meta exists, verify fingerprints
        if let Some(models) = &trees_models_opt
            && let Some(ok) =
                terrain::verify_snapshot_fingerprints("wizard_woods", &terrain_cpu, Some(models))
        {
            log::info!(
                "zone snapshot meta verification: {}",
                if ok { "ok" } else { "MISMATCH" }
            );
        }

        log::info!(
            "spawned {} NPCs across rings: near={}, mid1={}, mid2={}, mid3={}, far={}",
            server.npcs.len(),
            near_count,
            mid1_count,
            mid2_count,
            mid3_count,
            far_count
        );
        // Build zombie instances from server NPCs
        let mut zombie_instances_cpu: Vec<InstanceSkin> = Vec::new();
        let mut zombie_models: Vec<glam::Mat4> = Vec::new();
        let mut zombie_ids: Vec<crate::server::NpcId> = Vec::new();
        for (idx, npc) in server.npcs.iter().enumerate() {
            // Snap initial zombie spawn to terrain height
            let (h, _n) = terrain::height_at(&terrain_cpu, npc.pos.x, npc.pos.z);
            let pos = glam::vec3(npc.pos.x, h, npc.pos.z);
            let m = glam::Mat4::from_scale_rotation_translation(
                glam::Vec3::splat(1.0),
                glam::Quat::IDENTITY,
                pos,
            );
            zombie_models.push(m);
            zombie_ids.push(npc.id);
            zombie_instances_cpu.push(InstanceSkin {
                model: m.to_cols_array_2d(),
                color: [1.0, 1.0, 1.0],
                selected: 0.0,
                palette_base: (idx as u32) * zombie_joints,
                _pad_inst: [0; 3],
            });
        }
        let zombie_instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("zombie-instances"),
            contents: bytemuck::cast_slice(&zombie_instances_cpu),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        // Zombie palettes storage sized to instance count
        let zombie_count = zombie_instances_cpu.len() as u32;
        let total_z_mats = zombie_count as usize * zombie_joints as usize;
        let zombie_palettes_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("zombie-palettes"),
            size: (total_z_mats * 64) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let zombie_palettes_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("zombie-palettes-bg"),
            layout: &palettes_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: zombie_palettes_buf.as_entire_binding(),
            }],
        });

        // Determine asset forward offset from the zombie root node (if present).
        let zombie_forward_offset = if let Some(root_ix) = zombie_cpu.root_node {
            let r = zombie_cpu
                .base_r
                .get(root_ix)
                .copied()
                .unwrap_or(glam::Quat::IDENTITY);
            let f = r * glam::Vec3::Z; // authoring forward in model space
            f32::atan2(f.x, f.z)
        } else {
            0.0
        };

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size: PhysicalSize::new(w, h),
            max_dim,
            depth,

            pipeline,
            inst_pipeline,
            wire_pipeline,
            particle_pipeline,
            sky_pipeline,
            globals_bg,
            sky_bg,
            terrain_model_bg: plane_model_bg,
            shard_model_bg,

            globals_buf,
            sky_buf,
            _plane_model_buf: plane_model_buf,
            shard_model_buf,

            terrain_vb,
            terrain_ib,
            terrain_index_count,
            wizard_vb,
            wizard_ib,
            wizard_index_count,
            zombie_vb,
            zombie_ib,
            zombie_index_count,
            ruins_vb,
            ruins_ib,
            ruins_index_count,
            wizard_instances,
            wizard_count: wizard_instances_cpu.len() as u32,
            zombie_instances,
            zombie_count: zombie_instances_cpu.len() as u32,
            zombie_instances_cpu,
            ruins_instances: scene_build.ruins_instances,
            ruins_count: scene_build.ruins_count,
            fx_instances,
            _fx_capacity: fx_capacity,
            fx_count,
            _fx_model_bg: fx_model_bg,
            quad_vb,
            palettes_buf,
            palettes_bg,
            joints_per_wizard: scene_build.joints_per_wizard,
            wizard_models,
            zombie_palettes_buf,
            zombie_palettes_bg,
            zombie_joints,
            zombie_models: zombie_models.clone(),
            zombie_cpu,
            zombie_time_offset: (0..zombie_count as usize)
                .map(|i| i as f32 * 0.35)
                .collect(),
            zombie_ids,
            zombie_prev_pos: zombie_models
                .iter()
                .map(|m| {
                    glam::vec3(
                        m.to_cols_array()[12],
                        m.to_cols_array()[13],
                        m.to_cols_array()[14],
                    )
                })
                .collect(),
            zombie_forward_offset,
            wizard_instances_cpu,
            wizard_pipeline,
            // debug pipelines removed
            wizard_mat_bg,
            _wizard_mat_buf,
            _wizard_tex_view,
            _wizard_sampler,
            zombie_mat_bg,
            _zombie_mat_buf,
            _zombie_tex_view,
            _zombie_sampler,
            wire_enabled: false,
            sky: sky_state,
            terrain_cpu,

            start: Instant::now(),
            last_time: 0.0,
            wizard_anim_index: scene_build.wizard_anim_index,
            wizard_time_offset: scene_build.wizard_time_offset,
            skinned_cpu,
            wizard_last_phase: vec![0.0; scene_build.wizard_count as usize],
            hand_right_node,
            root_node,
            projectiles: Vec::new(),
            particles: Vec::new(),
            fire_bolt,
            nameplates,
            nameplates_npc,
            bars,
            damage,

            // Player/camera
            pc_index: scene_build.pc_index,
            player: crate::client::controller::PlayerController::new(pc_initial_pos),
            input: Default::default(),
            cam_follow: camera_sys::FollowState {
                current_pos: glam::vec3(0.0, 5.0, -10.0),
                current_look: scene_build.cam_target,
            },
            pc_cast_queued: false,
            pc_anim_start: None,
            cam_orbit_yaw: 0.0,
            cam_orbit_pitch: 0.2,
            cam_distance: 8.5,
            cam_lift: 3.5,
            cam_look_height: 1.6,
            rmb_down: false,
            last_cursor_pos: None,
            npc_vb,
            npc_ib,
            npc_index_count,
            npc_instances,
            npc_count: 0, // cubes hidden; zombies replace them visually
            npc_instances_cpu,
            npc_models,
            trees_instances,
            trees_count,
            trees_vb,
            trees_ib,
            trees_index_count,
            server,
            wizard_hp: vec![100; scene_build.wizard_count as usize],
            wizard_hp_max: 100,
            pc_alive: true,
        })
    }

    /// Resize the swapchain while preserving aspect and device limits.
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        let (w, h) = scale_to_max((new_size.width, new_size.height), self.max_dim);
        if (w, h) != (new_size.width, new_size.height) {
            log::debug!(
                "Resized {}x{} exceeds max {}, clamped to {}x{} (aspect kept)",
                new_size.width,
                new_size.height,
                self.max_dim,
                w,
                h
            );
        }
        self.size = PhysicalSize::new(w, h);
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        self.depth = util::create_depth_view(
            &self.device,
            self.config.width,
            self.config.height,
            self.config.format,
        );
    }

    /// Render one frame.
    pub fn render(&mut self) -> Result<(), SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Time and dt
        let t = self.start.elapsed().as_secs_f32();
        let aspect = self.config.width as f32 / self.config.height as f32;
        let dt = (t - self.last_time).max(0.0);
        self.last_time = t;

        // Update player transform from input (WASD) then camera follow
        self.update_player_and_camera(dt, aspect);
        // Simple AI: rotate non-PC wizards to face nearest alive zombie so firebolts aim correctly
        self.update_wizard_ai(dt);
        // Compute local orbit offsets (relative to PC orientation)
        let (off_local, look_local) = camera_sys::compute_local_orbit_offsets(
            self.cam_distance,
            self.cam_orbit_yaw,
            self.cam_orbit_pitch,
            self.cam_lift,
            self.cam_look_height,
        );
        #[allow(unused_assignments)]
        // Anchor camera to the center of the PC model, not the feet.
        let pc_anchor = if self.pc_index < self.wizard_models.len() {
            let m = self.wizard_models[self.pc_index];
            (m * glam::Vec4::new(0.0, 1.2, 0.0, 1.0)).truncate()
        } else {
            self.player.pos + glam::vec3(0.0, 1.2, 0.0)
        };

        #[allow(unused_assignments)]
        let (_cam, mut globals) = camera_sys::third_person_follow(
            &mut self.cam_follow,
            pc_anchor,
            glam::Quat::from_rotation_y(self.player.yaw),
            off_local,
            look_local,
            aspect,
            dt,
        );
        // Keep camera above terrain: clamp eye/target Y to terrain height + clearance
        let clearance_eye = 0.2f32;
        let clearance_look = 0.05f32;
        let eye = self.cam_follow.current_pos;
        let (hy, _n) = terrain::height_at(&self.terrain_cpu, eye.x, eye.z);
        if self.cam_follow.current_pos.y < hy + clearance_eye {
            self.cam_follow.current_pos.y = hy + clearance_eye;
        }
        let look = self.cam_follow.current_look;
        let (hy2, _n2) = terrain::height_at(&self.terrain_cpu, look.x, look.z);
        if self.cam_follow.current_look.y < hy2 + clearance_look {
            self.cam_follow.current_look.y = hy2 + clearance_look;
        }
        // Recompute camera/globals without smoothing after clamping
        let (_cam2, globals2) = camera_sys::third_person_follow(
            &mut self.cam_follow,
            pc_anchor,
            glam::Quat::from_rotation_y(self.player.yaw),
            off_local,
            look_local,
            aspect,
            0.0,
        );
        globals = globals2;
        // Advance sky & lighting
        self.sky.update(dt);
        globals.sun_dir_time = [
            self.sky.sun_dir.x,
            self.sky.sun_dir.y,
            self.sky.sun_dir.z,
            self.sky.day_frac,
        ];
        for i in 0..9 {
            globals.sh_coeffs[i] = [
                self.sky.sh9_rgb[i][0],
                self.sky.sh9_rgb[i][1],
                self.sky.sh9_rgb[i][2],
                0.0,
            ];
        }
        globals.fog_params = [0.6, 0.7, 0.8, 0.0];
        self.queue
            .write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));
        // Sky raw params
        self.queue
            .write_buffer(&self.sky_buf, 0, bytemuck::bytes_of(&self.sky.sky_uniform));

        // Keep model base identity to avoid moving instances globally
        let shard_mtx = glam::Mat4::IDENTITY;
        let shard_model = Model {
            model: shard_mtx.to_cols_array_2d(),
            color: [0.85, 0.15, 0.15],
            emissive: 0.05,
            _pad: [0.0; 4],
        };
        self.queue
            .write_buffer(&self.shard_model_buf, 0, bytemuck::bytes_of(&shard_model));

        // Handle queued PC cast and update animation state
        self.process_pc_cast(t);
        // Update wizard skinning palettes on CPU then upload
        self.update_wizard_palettes(t);
        // Zombie AI/movement on server; then update local transforms and palettes
        {
            // Wizard positions for AI — keep index mapping 1:1 with wizard_models.
            // If PC is dead, push a far-away sentinel so NPCs won't target it.
            let mut wiz_pos: Vec<glam::Vec3> = Vec::with_capacity(self.wizard_count as usize);
            for (i, m) in self.wizard_models.iter().enumerate() {
                if !self.pc_alive && i == self.pc_index {
                    wiz_pos.push(glam::vec3(1.0e6, 0.0, 1.0e6));
                } else {
                    let c = m.to_cols_array();
                    wiz_pos.push(glam::vec3(c[12], c[13], c[14]));
                }
            }
            let hits = self.server.step_npc_ai(dt, &wiz_pos);
            // Apply melee hits to wizard HP
            for (widx, dmg) in hits {
                if let Some(hp) = self.wizard_hp.get_mut(widx) {
                    let before = *hp;
                    *hp = (*hp - dmg).max(0);
                    let fatal = *hp == 0;
                    log::info!(
                        "wizard melee hit: idx={} hp {} -> {} (dmg {}), fatal={}",
                        widx,
                        before,
                        *hp,
                        dmg,
                        fatal
                    );
                    // Spawn damage floater above head
                    if widx < self.wizard_models.len() {
                        let head = self.wizard_models[widx] * glam::Vec4::new(0.0, 1.7, 0.0, 1.0);
                        self.damage.spawn(head.truncate(), dmg);
                    }
                    if fatal {
                        if widx == self.pc_index {
                            self.kill_pc();
                        } else {
                            self.remove_wizard_at(widx);
                        }
                    }
                }
            }
            self.update_zombies_from_server();
            self.update_zombie_palettes(t);
        }
        // FX update (projectiles/particles)
        self.update_fx(t, dt);

        // Begin commands
        // Capture validation errors locally to avoid process-wide panic
        self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encoder"),
            });
        // Sky-only pass (no depth)
        {
            use wgpu::*;
            let mut sky = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("sky-pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.02,
                            g: 0.02,
                            b: 0.04,
                            a: 1.0,
                        }),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            sky.set_pipeline(&self.sky_pipeline);
            sky.set_bind_group(0, &self.globals_bg, &[]);
            sky.set_bind_group(1, &self.sky_bg, &[]);
            sky.draw(0..3, 0..1);
        }
        // Main pass with depth; load color from sky
        {
            use wgpu::*;
            let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("main-pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(1.0),
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // Terrain
            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(0, &self.globals_bg, &[]);
            rpass.set_bind_group(1, &self.terrain_model_bg, &[]);
            rpass.set_vertex_buffer(0, self.terrain_vb.slice(..));
            rpass.set_index_buffer(self.terrain_ib.slice(..), IndexFormat::Uint16);
            rpass.draw_indexed(0..self.terrain_index_count, 0, 0..1);

            // Trees (instanced static mesh)
            if self.trees_count > 0 {
                let inst_pipe = if self.wire_enabled {
                    self.wire_pipeline.as_ref().unwrap_or(&self.inst_pipeline)
                } else {
                    &self.inst_pipeline
                };
                rpass.set_pipeline(inst_pipe);
                rpass.set_bind_group(0, &self.globals_bg, &[]);
                rpass.set_bind_group(1, &self.shard_model_bg, &[]);
                rpass.set_vertex_buffer(0, self.trees_vb.slice(..));
                rpass.set_vertex_buffer(1, self.trees_instances.slice(..));
                rpass.set_index_buffer(self.trees_ib.slice(..), IndexFormat::Uint16);
                rpass.draw_indexed(0..self.trees_index_count, 0, 0..self.trees_count);
            }

            // Wizards
            self.draw_wizards(&mut rpass);
            // Zombies
            self.draw_zombies(&mut rpass);

            // Ruins (instanced) — only draw when we have instances
            if self.ruins_count > 0 {
                let inst_pipe = if self.wire_enabled {
                    self.wire_pipeline.as_ref().unwrap_or(&self.inst_pipeline)
                } else {
                    &self.inst_pipeline
                };
                rpass.set_pipeline(inst_pipe);
                rpass.set_bind_group(0, &self.globals_bg, &[]);
                rpass.set_bind_group(1, &self.shard_model_bg, &[]);
                rpass.set_vertex_buffer(0, self.ruins_vb.slice(..));
                rpass.set_vertex_buffer(1, self.ruins_instances.slice(..));
                rpass.set_index_buffer(self.ruins_ib.slice(..), IndexFormat::Uint16);
                rpass.draw_indexed(0..self.ruins_index_count, 0, 0..self.ruins_count);
            }

            // NPCs (instanced cubes)
            if self.npc_count > 0 {
                let inst_pipe = if self.wire_enabled {
                    self.wire_pipeline.as_ref().unwrap_or(&self.inst_pipeline)
                } else {
                    &self.inst_pipeline
                };
                rpass.set_pipeline(inst_pipe);
                rpass.set_bind_group(0, &self.globals_bg, &[]);
                rpass.set_bind_group(1, &self.shard_model_bg, &[]);
                rpass.set_vertex_buffer(0, self.npc_vb.slice(..));
                rpass.set_vertex_buffer(1, self.npc_instances.slice(..));
                rpass.set_index_buffer(self.npc_ib.slice(..), IndexFormat::Uint16);
                rpass.draw_indexed(0..self.npc_index_count, 0, 0..self.npc_count);
            }

            // FX
            self.draw_particles(&mut rpass);
        }
        // Overlay: health bars, floating damage numbers, then nameplates
        let view_proj = glam::Mat4::from_cols_array_2d(&globals.view_proj);
        // Build bar entries: wizards (including PC)
        let mut bar_entries: Vec<(glam::Vec3, f32)> = Vec::new();
        for (i, m) in self.wizard_models.iter().enumerate() {
            let head = *m * glam::Vec4::new(0.0, 1.7, 0.0, 1.0);
            let frac = (self.wizard_hp.get(i).copied().unwrap_or(self.wizard_hp_max) as f32)
                / (self.wizard_hp_max as f32);
            bar_entries.push((head.truncate(), frac));
        }
        // Zombies: anchor bars to the skinned instance head position (terrain‑aware)
        use std::collections::HashMap;
        let mut npc_map: HashMap<crate::server::NpcId, (i32, i32, bool, f32)> = HashMap::new();
        for n in &self.server.npcs {
            npc_map.insert(n.id, (n.hp, n.max_hp, n.alive, n.radius));
        }
        for (i, id) in self.zombie_ids.iter().enumerate() {
            if let Some((hp, max_hp, alive, _radius)) = npc_map.get(id).copied() {
                if !alive {
                    continue;
                }
                let m = self
                    .zombie_models
                    .get(i)
                    .copied()
                    .unwrap_or(glam::Mat4::IDENTITY);
                let head = m * glam::Vec4::new(0.0, 1.6, 0.0, 1.0);
                let frac = (hp.max(0) as f32) / (max_hp.max(1) as f32);
                bar_entries.push((head.truncate(), frac));
            }
        }
        self.bars.queue_entries(
            &self.device,
            &self.queue,
            self.config.width,
            self.config.height,
            view_proj,
            &bar_entries,
        );
        self.bars.draw(&mut encoder, &view);
        // Damage numbers
        self.damage.update(dt);
        self.damage.queue(
            &self.device,
            &self.queue,
            self.config.width,
            self.config.height,
            view_proj,
        );
        self.damage.draw(&mut encoder, &view);

        // Draw wizard nameplates first
        // Draw wizard nameplates for alive wizards only (hide dead PC/NPC labels)
        let mut wiz_alive: Vec<glam::Mat4> = Vec::new();
        for (i, m) in self.wizard_models.iter().enumerate() {
            let hp = self.wizard_hp.get(i).copied().unwrap_or(0);
            if hp > 0 {
                wiz_alive.push(*m);
            }
        }
        self.nameplates.queue_labels(
            &self.device,
            &self.queue,
            self.config.width,
            self.config.height,
            view_proj,
            &wiz_alive,
        );
        self.nameplates.draw(&mut encoder, &view);

        // Then NPC nameplates (separate atlas/vbuf instance to avoid intra-frame buffer overwrites)
        let mut npc_positions: Vec<glam::Vec3> = Vec::new();
        // Prefer model matrices for accurate label anchors (handles any future scaling/animation)
        for (idx, m) in self.zombie_models.iter().enumerate() {
            if let Some(npc) = self.server.npcs.get(idx)
                && !npc.alive
            {
                continue;
            }
            let head = *m * glam::Vec4::new(0.0, 1.6, 0.0, 1.0);
            npc_positions.push(head.truncate());
        }
        if !npc_positions.is_empty() {
            self.nameplates_npc.queue_npc_labels(
                &self.device,
                &self.queue,
                self.config.width,
                self.config.height,
                view_proj,
                &npc_positions,
                "Zombie",
            );
            self.nameplates_npc.draw(&mut encoder, &view);
        }

        // Submit only if no validation errors occurred
        if let Some(e) = pollster::block_on(self.device.pop_error_scope()) {
            // Skip submit on validation error to keep running without panicking
            log::error!("wgpu validation error (skipping frame): {:?}", e);
        } else {
            self.queue.submit(Some(encoder.finish()));
            frame.present();
        }
        Ok(())
    }
}

impl Renderer {
    fn yaw_from_model(m: &glam::Mat4) -> f32 {
        let f = *m * glam::Vec4::new(0.0, 0.0, 1.0, 0.0);
        f32::atan2(f.x, f.z)
    }

    fn turn_towards(current: f32, target: f32, max_delta: f32) -> f32 {
        let mut delta = target - current;
        while delta > std::f32::consts::PI {
            delta -= std::f32::consts::TAU;
        }
        while delta < -std::f32::consts::PI {
            delta += std::f32::consts::TAU;
        }
        if delta.abs() <= max_delta {
            target
        } else if delta > 0.0 {
            current + max_delta
        } else {
            current - max_delta
        }
    }

    fn update_wizard_ai(&mut self, dt: f32) {
        if self.wizard_count == 0 {
            return;
        }
        // Collect alive zombie positions once
        let mut targets: Vec<glam::Vec3> = Vec::new();
        for n in &self.server.npcs {
            if n.alive {
                targets.push(n.pos);
            }
        }
        if targets.is_empty() {
            return;
        }
        let yaw_rate = 2.5 * dt; // rad per frame
        for i in 0..(self.wizard_count as usize) {
            if i == self.pc_index {
                continue;
            }
            // Wizard position
            let m = self.wizard_models[i];
            let pos = glam::vec3(
                m.to_cols_array()[12],
                m.to_cols_array()[13],
                m.to_cols_array()[14],
            );
            // Find nearest target
            let mut best_d2 = f32::INFINITY;
            let mut best = None;
            for t in &targets {
                let d2 = (t.x - pos.x) * (t.x - pos.x) + (t.z - pos.z) * (t.z - pos.z);
                if d2 < best_d2 {
                    best_d2 = d2;
                    best = Some(*t);
                }
            }
            let Some(tgt) = best else {
                continue;
            };
            let desired_yaw = (tgt.x - pos.x).atan2(tgt.z - pos.z);
            let cur_yaw = Self::yaw_from_model(&m);
            let new_yaw = Self::turn_towards(cur_yaw, desired_yaw, yaw_rate);
            if (new_yaw - cur_yaw).abs() > 1e-4 {
                let new_m = glam::Mat4::from_scale_rotation_translation(
                    glam::Vec3::splat(1.0),
                    glam::Quat::from_rotation_y(new_yaw),
                    pos,
                );
                self.wizard_models[i] = new_m;
                // Update instance CPU + upload one slot
                let mut inst = self.wizard_instances_cpu[i];
                inst.model = new_m.to_cols_array_2d();
                self.wizard_instances_cpu[i] = inst;
                let offset = (i * std::mem::size_of::<InstanceSkin>()) as u64;
                self.queue
                    .write_buffer(&self.wizard_instances, offset, bytemuck::bytes_of(&inst));
            }
        }
    }
    #[allow(dead_code)]
    fn select_zombie_clip(&self) -> Option<&AnimClip> {
        // Prefer common idle names, then walk/run, otherwise any
        let keys = [
            "Idle",
            "idle",
            "IDLE",
            "ProcIdle",
            "Idle01",
            "StandingIdle",
            "Armature|mixamo.com|Layer0",
            "Walk",
            "walk",
            "Run",
            "run",
        ];
        for k in keys {
            if let Some(c) = self.zombie_cpu.animations.get(k) {
                return Some(c);
            }
        }
        self.zombie_cpu.animations.values().next()
    }

    fn ensure_proc_idle_clip(&mut self) -> String {
        if self.zombie_cpu.animations.contains_key("ProcIdle") {
            return "ProcIdle".to_string();
        }
        use std::collections::HashMap;
        let mut r_tracks: HashMap<usize, TrackQuat> = HashMap::new();
        let t_tracks: HashMap<usize, TrackVec3> = HashMap::new();
        let s_tracks: HashMap<usize, TrackVec3> = HashMap::new();
        // Helper to find node index by partial name
        let find = |name: &str, names: &Vec<String>| -> Option<usize> {
            let lname = name.to_lowercase();
            names.iter().position(|n| n.to_lowercase().contains(&lname))
        };
        let names = &self.zombie_cpu.node_names;
        // Choose nodes
        let root_idx = self
            .zombie_cpu
            .root_node
            .or(self.zombie_cpu.joints_nodes.first().copied());
        let spine_idx = find("spine", names).or(find("hips", names)).or(root_idx);
        let head_idx = find("head", names).or(find("neck", names));
        let times = vec![0.0, 1.0, 2.0];
        // Small yaw sway at spine/root
        if let Some(si) = spine_idx {
            let yaw0 = glam::Quat::from_rotation_y(0.0);
            let yaw1 = glam::Quat::from_rotation_y(3.0_f32.to_radians());
            r_tracks.insert(
                si,
                TrackQuat {
                    times: times.clone(),
                    values: vec![yaw0, yaw1, yaw0],
                },
            );
        }
        // Gentle head nod
        if let Some(hi) = head_idx {
            let p0 = glam::Quat::from_rotation_x(0.0);
            let p1 = glam::Quat::from_rotation_x((-2.5_f32).to_radians());
            r_tracks.insert(
                hi,
                TrackQuat {
                    times: times.clone(),
                    values: vec![p0, p1, p0],
                },
            );
        }
        let clip = AnimClip {
            name: "ProcIdle".to_string(),
            duration: 2.0,
            t_tracks,
            r_tracks,
            s_tracks,
        };
        self.zombie_cpu
            .animations
            .insert("ProcIdle".to_string(), clip);
        "ProcIdle".to_string()
    }
    fn update_zombie_palettes(&mut self, time_global: f32) {
        if self.zombie_count == 0 {
            return;
        }
        // Per-instance clip selection based on movement
        let joints = self.zombie_joints as usize;
        let mut mats_all: Vec<[f32; 16]> = Vec::with_capacity(self.zombie_count as usize * joints);
        // Build quick lookup for attack state and radius using server NPCs
        use std::collections::HashMap;
        let mut attack_map: HashMap<crate::server::NpcId, bool> = HashMap::new();
        let mut radius_map: HashMap<crate::server::NpcId, f32> = HashMap::new();
        for n in &self.server.npcs {
            attack_map.insert(n.id, n.attack_anim > 0.0);
            radius_map.insert(n.id, n.radius);
        }
        // Wizard positions
        let mut wiz_pos: Vec<glam::Vec3> = Vec::with_capacity(self.wizard_models.len());
        for m in &self.wizard_models {
            let c = m.to_cols_array();
            wiz_pos.push(glam::vec3(c[12], c[13], c[14]));
        }
        // Helper: fuzzy find clip by case-insensitive substring(s)
        let find_clip = |subs: &[&str], anims: &std::collections::HashMap<String, AnimClip>| -> Option<String> {
            let subsl: Vec<String> = subs.iter().map(|s| s.to_lowercase()).collect();
            for name in anims.keys() {
                let low = name.to_lowercase();
                if subsl.iter().any(|s| low.contains(s)) {
                    return Some(name.clone());
                }
            }
            None
        };
        for i in 0..(self.zombie_count as usize) {
            let c = self.zombie_models[i].to_cols_array();
            let pos = glam::vec3(c[12], c[13], c[14]);
            let prev = self.zombie_prev_pos.get(i).copied().unwrap_or(pos);
            let moving = (pos - prev).length_squared() > 1e-4;
            self.zombie_prev_pos[i] = pos;
            let has_walk = self.zombie_cpu.animations.contains_key("Walk")
                || find_clip(&["walk"], &self.zombie_cpu.animations).is_some();
            let has_run = self.zombie_cpu.animations.contains_key("Run")
                || find_clip(&["run"], &self.zombie_cpu.animations).is_some();
            let has_idle = self.zombie_cpu.animations.contains_key("Idle")
                || find_clip(&["idle", "stand"], &self.zombie_cpu.animations).is_some();
            let has_attack = self.zombie_cpu.animations.contains_key("Attack")
                || find_clip(&["attack", "punch", "hit", "swipe", "slash", "bite"], &self.zombie_cpu.animations).is_some();
            let has_proc = self.zombie_cpu.animations.contains_key("ProcIdle");
            let has_static = self.zombie_cpu.animations.contains_key("__static");
            let any_owned: String = self
                .zombie_cpu
                .animations
                .keys()
                .next()
                .cloned()
                .unwrap_or("__static".to_string());
            // Prioritize attack animation when the server reports it or if in melee contact
            let zid = *self.zombie_ids.get(i).unwrap_or(&crate::server::NpcId(0));
            let mut is_attacking = attack_map.get(&zid).copied().unwrap_or(false);
            // In-contact heuristic: nearest wizard within (z_radius + wizard_r + pad)
            let z_radius = radius_map.get(&zid).copied().unwrap_or(0.95);
            let wizard_r = 0.7f32;
            let pad = 0.10f32;
            let mut best_d2 = f32::INFINITY;
            for w in &wiz_pos {
                let dx = w.x - pos.x;
                let dz = w.z - pos.z;
                let d2 = dx * dx + dz * dz;
                if d2 < best_d2 {
                    best_d2 = d2;
                }
            }
            let contact = z_radius + wizard_r + pad;
            if best_d2 <= contact * contact {
                is_attacking = true;
            }
            let clip_name = if is_attacking && has_attack {
                if self.zombie_cpu.animations.contains_key("Attack") {
                    "Attack"
                } else if let Some(_n) = find_clip(&["attack", "punch", "hit", "swipe", "slash", "bite"], &self.zombie_cpu.animations) {
                    // Use first fuzzy match
                    // Note: we allocate here, handled below when fetching the clip
                    // by looking it up by this dynamic name
                    // We'll handle lookup via proc_name_str/lookup below
                    // Return a sentinel that will be replaced
                    // Store in a temporary variable instead
                    // We'll set proc_name_str to Some(n) and use it
                    // Use placeholder here; actual value comes from proc_name_str
                    "__attack_dynamic__"
                } else {
                    // Fallback to moving/idle below
                    "__noattack__"
                }
            } else if moving {
                if has_walk {
                    "Walk"
                } else if has_run {
                    "Run"
                } else if has_idle {
                    "Idle"
                } else if has_proc {
                    "ProcIdle"
                } else if has_static {
                    "__static"
                } else {
                    &any_owned
                }
            } else if has_idle {
                "Idle"
            } else if has_proc {
                "ProcIdle"
            } else if has_static {
                "__static"
            } else {
                &any_owned
            };

            let need_proc = clip_name == "__static" && !has_idle && !has_proc;
            // Optionally override with fuzzy-attack match name
            let mut proc_name_str = if need_proc {
                Some(self.ensure_proc_idle_clip())
            } else {
                None
            };
            if clip_name == "__attack_dynamic__"
                && let Some(n) = find_clip(&["attack", "punch", "hit", "swipe", "slash", "bite"], &self.zombie_cpu.animations)
            {
                proc_name_str = Some(n);
            }
            let t = time_global + self.zombie_time_offset.get(i).copied().unwrap_or(0.0);
            let lookup = proc_name_str.as_deref().unwrap_or(clip_name);
            let clip = self.zombie_cpu.animations.get(lookup).unwrap();
            let palette = anim::sample_palette(&self.zombie_cpu, clip, t);
            for m in palette {
                mats_all.push(m.to_cols_array());
            }
        }
        self.queue.write_buffer(
            &self.zombie_palettes_buf,
            0,
            bytemuck::cast_slice(&mats_all),
        );
    }

    fn update_zombies_from_server(&mut self) {
        // Build map from id -> pos
        use std::collections::HashMap;
        let mut pos_map: HashMap<crate::server::NpcId, glam::Vec3> = HashMap::new();
        for n in &self.server.npcs {
            pos_map.insert(n.id, n.pos);
        }
        let mut any = false;
        for (i, id) in self.zombie_ids.clone().iter().enumerate() {
            if let Some(p) = pos_map.get(id) {
                let m_old = self.zombie_models[i];
                let prev = self.zombie_prev_pos.get(i).copied().unwrap_or(*p);
                // If the zombie moved this frame, face the movement direction.
                // Apply authoring forward-axis correction so models authored with
                // +X (or -Z) forward still look where they walk.
                let delta = *p - prev;
                let yaw = if delta.length_squared() > 1e-5 {
                    delta.x.atan2(delta.z) - self.zombie_forward_offset
                } else {
                    Self::yaw_from_model(&m_old)
                };
                // Stick to terrain height
                let (h, _n) = terrain::height_at(&self.terrain_cpu, p.x, p.z);
                let pos = glam::vec3(p.x, h, p.z);
                let new_m = glam::Mat4::from_scale_rotation_translation(
                    glam::Vec3::splat(1.0),
                    glam::Quat::from_rotation_y(yaw),
                    pos,
                );
                self.zombie_models[i] = new_m;
                let mut inst = self.zombie_instances_cpu[i];
                inst.model = new_m.to_cols_array_2d();
                self.zombie_instances_cpu[i] = inst;
                any = true;
            }
        }
        if any {
            let bytes: &[u8] = bytemuck::cast_slice(&self.zombie_instances_cpu);
            self.queue.write_buffer(&self.zombie_instances, 0, bytes);
        }
    }
    /// Handle platform window events that affect input (keyboard focus/keys).
    pub fn handle_window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state.is_pressed();
                match event.physical_key {
                    // Ignore movement/casting inputs if the PC is dead
                    PhysicalKey::Code(KeyCode::KeyW) if self.pc_alive => {
                        self.input.forward = pressed
                    }
                    PhysicalKey::Code(KeyCode::KeyS) if self.pc_alive => {
                        self.input.backward = pressed
                    }
                    PhysicalKey::Code(KeyCode::KeyA) if self.pc_alive => self.input.left = pressed,
                    PhysicalKey::Code(KeyCode::KeyD) if self.pc_alive => self.input.right = pressed,
                    PhysicalKey::Code(KeyCode::ShiftLeft)
                    | PhysicalKey::Code(KeyCode::ShiftRight)
                        if self.pc_alive =>
                    {
                        self.input.run = pressed
                    }
                    PhysicalKey::Code(KeyCode::Digit1)
                    | PhysicalKey::Code(KeyCode::Numpad1)
                    | PhysicalKey::Code(KeyCode::Space)
                        if self.pc_alive =>
                    {
                        if pressed {
                            self.pc_cast_queued = true;
                            log::info!("PC cast queued: Fire Bolt");
                        }
                    }
                    // Sky controls (pause/scrub/speed)
                    PhysicalKey::Code(KeyCode::Space) => {
                        if pressed {
                            self.sky.toggle_pause();
                        }
                    }
                    PhysicalKey::Code(KeyCode::BracketLeft) => {
                        if pressed {
                            self.sky.scrub(-0.01);
                        }
                    }
                    PhysicalKey::Code(KeyCode::BracketRight) => {
                        if pressed {
                            self.sky.scrub(0.01);
                        }
                    }
                    PhysicalKey::Code(KeyCode::Minus) => {
                        if pressed {
                            self.sky.speed_mul(0.5);
                            log::info!("time_scale: {:.2}", self.sky.time_scale);
                        }
                    }
                    PhysicalKey::Code(KeyCode::Equal) => {
                        if pressed {
                            self.sky.speed_mul(2.0);
                            log::info!("time_scale: {:.2}", self.sky.time_scale);
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let mut step = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => *y,
                    winit::event::MouseScrollDelta::PixelDelta(p) => (p.y as f32) * 0.05,
                };
                if step.abs() < 1e-3 {
                    step = 0.0;
                }
                if step != 0.0 {
                    self.cam_distance = (self.cam_distance - step).clamp(3.0, 25.0);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *button == winit::event::MouseButton::Right {
                    self.rmb_down = state.is_pressed();
                    if !self.rmb_down {
                        self.last_cursor_pos = None; // reset deltas
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if self.rmb_down {
                    if let Some((lx, ly)) = self.last_cursor_pos {
                        let dx = position.x - lx;
                        let dy = position.y - ly;
                        let sens = 0.005;
                        self.cam_orbit_yaw = wrap_angle(self.cam_orbit_yaw - dx as f32 * sens);
                        // Invert pitch control (mouse up pitches camera down, and vice versa)
                        self.cam_orbit_pitch =
                            (self.cam_orbit_pitch + dy as f32 * sens).clamp(-0.6, 1.2);
                    }
                    self.last_cursor_pos = Some((position.x, position.y));
                }
            }
            WindowEvent::Focused(false) => {
                // Clear sticky keys when window loses focus
                self.input.clear();
            }
            _ => {}
        }
    }

    /// Apply a basic WASD character controller to the PC and update its instance data.
    fn update_player_and_camera(&mut self, dt: f32, _aspect: f32) {
        if self.wizard_count == 0 || !self.pc_alive || self.pc_index >= self.wizard_count as usize {
            return;
        }
        let cam_fwd = self.cam_follow.current_look - self.cam_follow.current_pos;
        self.player.update(&self.input, dt, cam_fwd);
        self.apply_pc_transform();
    }

    fn apply_pc_transform(&mut self) {
        if !self.pc_alive || self.pc_index >= self.wizard_count as usize {
            return;
        }
        // Update CPU model matrix and upload only the PC instance
        let rot = glam::Quat::from_rotation_y(self.player.yaw);
        // Project player onto terrain height
        let (h, _n) = terrain::height_at(&self.terrain_cpu, self.player.pos.x, self.player.pos.z);
        let pos = glam::vec3(self.player.pos.x, h, self.player.pos.z);
        let m = glam::Mat4::from_scale_rotation_translation(glam::Vec3::splat(1.0), rot, pos);
        self.wizard_models[self.pc_index] = m;
        let mut inst = self.wizard_instances_cpu[self.pc_index];
        inst.model = m.to_cols_array_2d();
        self.wizard_instances_cpu[self.pc_index] = inst;
        let offset = (self.pc_index * std::mem::size_of::<InstanceSkin>()) as u64;
        self.queue
            .write_buffer(&self.wizard_instances, offset, bytemuck::bytes_of(&inst));
    }
    fn update_wizard_palettes(&mut self, time_global: f32) {
        // Build palettes for each wizard with its animation + offset.
        if self.wizard_count == 0 {
            return;
        }
        let joints = self.joints_per_wizard as usize;
        let mut mats: Vec<glam::Mat4> = Vec::with_capacity(self.wizard_count as usize * joints);
        for i in 0..(self.wizard_count as usize) {
            let clip = self.select_clip(self.wizard_anim_index[i]);
            let palette = if self.pc_alive
                && i == self.pc_index
                && self.pc_index < self.wizard_count as usize
            {
                if let Some(start) = self.pc_anim_start {
                    let lt = (time_global - start).clamp(0.0, clip.duration.max(0.0));
                    anim::sample_palette(&self.skinned_cpu, clip, lt)
                } else {
                    anim::sample_palette(&self.skinned_cpu, clip, time_global)
                }
            } else {
                let t = time_global + self.wizard_time_offset[i];
                anim::sample_palette(&self.skinned_cpu, clip, t)
            };
            mats.extend(palette);
        }
        // Upload as raw f32x16

        let mut raw: Vec<[f32; 16]> = Vec::with_capacity(mats.len());
        for m in mats {
            raw.push(m.to_cols_array());
        }
        self.queue
            .write_buffer(&self.palettes_buf, 0, bytemuck::cast_slice(&raw));
    }

    fn select_clip(&self, idx: usize) -> &AnimClip {
        // Honor the requested clip first; fallback only if missing.
        let requested = match idx {
            0 => "PortalOpen",
            1 => "Still",
            _ => "Waiting",
        };
        if let Some(c) = self.skinned_cpu.animations.get(requested) {
            return c;
        }
        // Fallback preference order
        for name in ["Waiting", "Still", "PortalOpen"] {
            if let Some(c) = self.skinned_cpu.animations.get(name) {
                return c;
            }
        }
        // Last resort: any available clip
        self.skinned_cpu
            .animations
            .values()
            .next()
            .expect("at least one animation clip present")
    }

    fn process_pc_cast(&mut self, t: f32) {
        if !self.pc_alive || self.pc_index >= self.wizard_count as usize {
            return;
        }
        if self.pc_cast_queued {
            self.pc_cast_queued = false;
            if self.wizard_anim_index[self.pc_index] != 0 && self.pc_anim_start.is_none() {
                // Start PortalOpen now
                self.wizard_anim_index[self.pc_index] = 0;
                self.wizard_time_offset[self.pc_index] = -t; // phase=0 at start
                self.wizard_last_phase[self.pc_index] = 0.0;
                self.pc_anim_start = Some(t);
            }
        }
        if let Some(start) = self.pc_anim_start {
            if self.wizard_anim_index[self.pc_index] == 0 {
                let clip = self.select_clip(0);
                if t - start >= clip.duration.max(0.0) {
                    // Return to Still
                    self.wizard_anim_index[self.pc_index] = 1;
                    self.pc_anim_start = None;
                }
            } else {
                self.pc_anim_start = None;
            }
        }
    }

    // Update and render-side state for projectiles/particles
    fn update_fx(&mut self, t: f32, dt: f32) {
        // 1) Spawn firebolts for PortalOpen phase crossing.
        // PC is always allowed to cast; NPC wizards only cast while zombies remain.
        if self.wizard_count > 0 {
            let zombies_alive = self.any_zombies_alive();
            let cycle = 5.0f32; // synthetic cycle period
            let bolt_offset = 1.5f32; // trigger point in the cycle
            for i in 0..(self.wizard_count as usize) {
                if self.wizard_anim_index[i] != 0 {
                    continue;
                } // only PortalOpen
                let prev = self.wizard_last_phase[i];
                let phase = (t + self.wizard_time_offset[i]) % cycle;
                let crossed = (prev <= bolt_offset && phase >= bolt_offset)
                    || (prev > phase && (prev <= bolt_offset || phase >= bolt_offset));
                let allowed = i == self.pc_index || zombies_alive;
                if allowed && crossed {
                    let clip = self.select_clip(self.wizard_anim_index[i]);
                    let clip_time = if clip.duration > 0.0 {
                        phase.min(clip.duration)
                    } else {
                        0.0
                    };
                    if let Some(origin_local) = self.right_hand_world(clip, clip_time) {
                        let inst = self
                            .wizard_models
                            .get(i)
                            .copied()
                            .unwrap_or(glam::Mat4::IDENTITY);
                        let origin_w = inst
                            * glam::Vec4::new(origin_local.x, origin_local.y, origin_local.z, 1.0);
                        // Use instance forward in world-space to ensure truly straight shots.
                        let dir_w = (inst * glam::Vec4::new(0.0, 0.0, 1.0, 0.0))
                            .truncate()
                            .normalize_or_zero();
                        let right_w = (inst * glam::Vec4::new(1.0, 0.0, 0.0, 0.0))
                            .truncate()
                            .normalize_or_zero();
                        let lateral = 0.20; // meters to shift toward center
                        let spawn = origin_w.truncate() + dir_w * 0.3 - right_w * lateral;
                        if i == self.pc_index {
                            log::info!("PC Fire Bolt fired at t={:.2}", t);
                        }
                        self.spawn_firebolt(spawn, dir_w, t, Some(i));
                    }
                }
                self.wizard_last_phase[i] = phase;
            }
        }

        // 2) Integrate projectiles
        for p in &mut self.projectiles {
            p.pos += p.vel * dt;
        }
        // 2.5) Server-side collision vs NPCs
        if !self.projectiles.is_empty() && !self.server.npcs.is_empty() {
            let damage = 10; // TODO: integrate with spell spec dice
            let hits = self
                .server
                .collide_and_damage(&mut self.projectiles, dt, damage);
            for h in &hits {
                log::info!(
                    "hit NPC id={} hp {} -> {} (dmg {}), fatal={}",
                    (h.npc).0,
                    h.hp_before,
                    h.hp_after,
                    h.damage,
                    h.fatal
                );
                // Impact burst at hit position
                for _ in 0..16 {
                    let a = rand_unit() * std::f32::consts::TAU;
                    let r = 4.0 + rand_unit() * 1.2;
                    self.particles.push(Particle {
                        pos: h.pos,
                        vel: glam::vec3(a.cos() * r, 2.0 + rand_unit() * 1.0, a.sin() * r),
                        age: 0.0,
                        life: 0.18,
                        size: 0.02,
                        color: [1.0, 0.5, 0.2],
                    });
                }
                // Update zombie visuals: remove model/instance if dead; otherwise keep
                if h.fatal
                    && let Some(idx) = self.zombie_ids.iter().position(|id| *id == h.npc)
                {
                    self.zombie_ids.swap_remove(idx);
                    self.zombie_models.swap_remove(idx);
                    if (idx as u32) < self.zombie_count {
                        self.zombie_instances_cpu.swap_remove(idx);
                        self.zombie_count -= 1;
                        // Recompute palette_base for contiguity
                        for (i, inst) in self.zombie_instances_cpu.iter_mut().enumerate() {
                            inst.palette_base = (i as u32) * self.zombie_joints;
                        }
                        let bytes: &[u8] = bytemuck::cast_slice(&self.zombie_instances_cpu);
                        self.queue.write_buffer(&self.zombie_instances, 0, bytes);
                    }
                }
                // Damage floater above NPC head (terrain/instance-aware)
                if let Some(idx) = self.zombie_ids.iter().position(|id| *id == h.npc) {
                    let m = self
                        .zombie_models
                        .get(idx)
                        .copied()
                        .unwrap_or(glam::Mat4::IDENTITY);
                    let head = m * glam::Vec4::new(0.0, 1.6, 0.0, 1.0);
                    self.damage.spawn(head.truncate(), h.damage);
                } else if let Some(n) = self.server.npcs.iter().find(|n| n.id == h.npc) {
                    let (hgt, _n) = terrain::height_at(&self.terrain_cpu, n.pos.x, n.pos.z);
                    let pos = glam::vec3(n.pos.x, hgt + n.radius + 0.9, n.pos.z);
                    self.damage.spawn(pos, h.damage);
                } else {
                    // Fallback: snap event position to terrain height
                    let (hgt, _n) = terrain::height_at(&self.terrain_cpu, h.pos.x, h.pos.z);
                    self.damage
                        .spawn(glam::vec3(h.pos.x, hgt + 0.9, h.pos.z), h.damage);
                }
            }
            if hits.is_empty() {
                log::debug!(
                    "no hits this frame: projectiles={} npcs={}",
                    self.projectiles.len(),
                    self.server.npcs.len()
                );
            }
        }
        // Ground hit or timeout
        let mut burst: Vec<Particle> = Vec::new();
        let mut i = 0;
        while i < self.projectiles.len() {
            let pcur = self.projectiles[i].pos;
            let (h, _n) = terrain::height_at(&self.terrain_cpu, pcur.x, pcur.z);
            let kill = t >= self.projectiles[i].t_die || pcur.y <= h + 0.02;
            if kill {
                let mut hit = self.projectiles[i].pos;
                // Snap impact to terrain height
                hit.y = h;
                // much smaller flare + compact burst
                burst.push(Particle {
                    pos: hit,
                    vel: glam::Vec3::ZERO,
                    age: 0.0,
                    life: 0.12,
                    size: 0.06,
                    color: [1.0, 0.8, 0.25],
                });
                for _ in 0..10 {
                    let a = rand_unit() * std::f32::consts::TAU;
                    let r = 3.0 + rand_unit() * 0.8;
                    burst.push(Particle {
                        pos: hit,
                        vel: glam::vec3(a.cos() * r, 1.5 + rand_unit() * 1.0, a.sin() * r),
                        age: 0.0,
                        life: 0.12,
                        size: 0.015,
                        color: [1.0, 0.55, 0.15],
                    });
                }
                self.projectiles.swap_remove(i);
            } else {
                i += 1;
            }
        }

        // 2.6) Collide with wizards/PC (friendly fire on)
        if !self.projectiles.is_empty() {
            self.collide_with_wizards(dt, 10);
        }

        // 3) Upload FX instances (billboard particles) — show both bolts and impacts

        // 4) Upload FX instances (billboard particles)
        let mut inst: Vec<ParticleInstance> = Vec::with_capacity(self.projectiles.len());
        for pr in &self.projectiles {
            inst.push(ParticleInstance {
                pos: [pr.pos.x, pr.pos.y, pr.pos.z],
                size: 0.14,
                color: [1.0, 0.35, 0.08],
                _pad: 0.0,
            });
        }
        self.fx_count = inst.len() as u32;
        if self.fx_count > 0 {
            self.queue
                .write_buffer(&self.fx_instances, 0, bytemuck::cast_slice(&inst));
        }

        // 5) If no zombies remain, retire NPC wizards from the casting loop
        if !self.any_zombies_alive() {
            for i in 0..(self.wizard_count as usize) {
                if i == self.pc_index {
                    continue; // leave PC state alone
                }
                // 2 => "Waiting" (see select_clip)
                if self.wizard_anim_index[i] == 0 {
                    self.wizard_anim_index[i] = 2;
                }
            }
        }
    }

    fn collide_with_wizards(&mut self, dt: f32, damage: i32) {
        let mut i = 0usize;
        while i < self.projectiles.len() {
            let pr = self.projectiles[i];
            let p0 = pr.pos - pr.vel * dt;
            let p1 = pr.pos;
            let mut hit_someone = false;
            for j in 0..(self.wizard_count as usize) {
                if Some(j) == pr.owner_wizard {
                    continue;
                } // do not hit the caster
                let hp = self.wizard_hp.get(j).copied().unwrap_or(self.wizard_hp_max);
                if hp <= 0 {
                    continue;
                }
                let m = self.wizard_models[j].to_cols_array();
                let center = glam::vec3(m[12], m[13], m[14]);
                let r = 0.7f32; // generous cylinder radius
                if segment_hits_circle_xz(p0, p1, center, r) {
                    let before = self.wizard_hp[j];
                    let after = (before - damage).max(0);
                    self.wizard_hp[j] = after;
                    let fatal = after == 0;
                    log::info!(
                        "wizard hit: idx={} hp {} -> {} (dmg {}), fatal={}",
                        j,
                        before,
                        after,
                        damage,
                        fatal
                    );
                    // Floating damage number
                    let head = center + glam::vec3(0.0, 1.7, 0.0);
                    self.damage.spawn(head, damage);
                    if fatal {
                        if j == self.pc_index {
                            self.kill_pc();
                        } else {
                            self.remove_wizard_at(j);
                        }
                    }
                    // impact burst
                    for _ in 0..14 {
                        let a = rand_unit() * std::f32::consts::TAU;
                        let r2 = 3.5 + rand_unit() * 1.0;
                        self.particles.push(Particle {
                            pos: p1,
                            vel: glam::vec3(a.cos() * r2, 2.0 + rand_unit() * 1.0, a.sin() * r2),
                            age: 0.0,
                            life: 0.16,
                            size: 0.02,
                            color: [1.0, 0.45, 0.15],
                        });
                    }
                    self.projectiles.swap_remove(i);
                    hit_someone = true;
                    break;
                }
            }
            if !hit_someone {
                i += 1;
            }
        }
    }

    fn spawn_firebolt(
        &mut self,
        origin: glam::Vec3,
        dir: glam::Vec3,
        t: f32,
        owner: Option<usize>,
    ) {
        let mut speed = 40.0;
        // Extend projectile lifetime by 50% so paths travel farther.
        let life = 1.2 * 1.5;
        if let Some(spec) = &self.fire_bolt
            && let Some(p) = &spec.projectile
        {
            speed = p.speed_mps;
        }
        self.projectiles.push(Projectile {
            pos: origin,
            vel: dir * speed,
            t_die: t + life,
            owner_wizard: owner,
        });
    }

    fn right_hand_world(&self, clip: &AnimClip, phase: f32) -> Option<glam::Vec3> {
        let h = self.hand_right_node?;
        let m = anim::global_of_node(&self.skinned_cpu, clip, phase, h)?;
        let c = m.to_cols_array();
        Some(glam::vec3(c[12], c[13], c[14]))
    }
    #[allow(dead_code)]
    fn root_flat_forward(&self, clip: &AnimClip, phase: f32) -> Option<glam::Vec3> {
        let r = self.root_node?;
        let m = anim::global_of_node(&self.skinned_cpu, clip, phase, r)?;
        let z = (m * glam::Vec4::new(0.0, 0.0, 1.0, 0.0)).truncate();
        let mut f = z;
        f.y = 0.0;
        if f.length_squared() > 1e-6 {
            Some(f.normalize())
        } else {
            None
        }
    }
}

fn wrap_angle(a: f32) -> f32 {
    let mut x = a;
    while x > std::f32::consts::PI {
        x -= std::f32::consts::TAU;
    }
    while x < -std::f32::consts::PI {
        x += std::f32::consts::TAU;
    }
    x
}

fn rand_unit() -> f32 {
    use rand::Rng as _;
    let mut r = rand::rng();
    r.random::<f32>() * 2.0 - 1.0
}

fn segment_hits_circle_xz(p0: glam::Vec3, p1: glam::Vec3, c: glam::Vec3, r: f32) -> bool {
    let p0 = glam::vec2(p0.x, p0.z);
    let p1 = glam::vec2(p1.x, p1.z);
    let c = glam::vec2(c.x, c.z);
    let d = p1 - p0;
    let m = p0 - c;
    let a = d.dot(d);
    if a <= 1e-6 {
        return m.length() <= r;
    }
    let t = (-(m.dot(d)) / a).clamp(0.0, 1.0);
    let closest = p0 + d * t;
    (closest - c).length() <= r
}

#[cfg(test)]
mod proj_tests {
    use super::*;
    #[test]
    fn segment_circle_intersects_center_cross() {
        let c = glam::vec3(0.0, 0.0, 0.0);
        let p0 = glam::vec3(-2.0, 0.5, 0.0);
        let p1 = glam::vec3(2.0, 0.5, 0.0);
        assert!(segment_hits_circle_xz(p0, p1, c, 0.5));
    }
}
