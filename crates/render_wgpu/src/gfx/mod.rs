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
pub mod renderer {
    pub mod passes;
    pub mod resize;
}
mod gbuffer;
mod hiz;
mod mesh;
mod pipeline;
mod temporal;
mod types;
pub use types::Vertex;
mod anim;
mod camera_sys;
mod draw;
mod foliage;
pub mod fx;
mod material;
mod npcs;
mod rocks;
mod ruins;
mod scene;
mod sky;
pub mod terrain;
mod ui;
mod util;
mod zombies;

use data_runtime::{
    loader as data_loader,
    spell::SpellSpec,
    zone::{ZoneManifest, load_zone_manifest},
};
use ra_assets::skinning::load_gltf_skinned;
use ra_assets::skinning::merge_gltf_animations;
use ra_assets::types::{AnimClip, SkinnedMeshCPU, TrackQuat, TrackVec3};
// (scene building now encapsulated; ECS types unused here)
use anyhow::Context;
use types::{Globals, InstanceSkin, Model, ParticleInstance, VertexSkinned};
use util::scale_to_max;

use crate::server_ext::CollideProjectiles;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PcCast {
    FireBolt,
    MagicMissile,
}
use std::time::Instant;

use wgpu::{
    SurfaceError, SurfaceTargetUnsafe, rwh::HasDisplayHandle, rwh::HasWindowHandle, util::DeviceExt,
};
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

fn asset_path(rel: &str) -> std::path::PathBuf {
    // Prefer workspace-level assets so this crate works when built inside a workspace.
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../").join(rel);
    if ws.exists() { ws } else { here.join(rel) }
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
    // Offscreen scene color
    scene_color: wgpu::Texture,
    scene_view: wgpu::TextureView,
    // Read-only copy of scene for post passes that sample while writing to SceneColor
    scene_read: wgpu::Texture,
    scene_read_view: wgpu::TextureView,

    // Lighting M1: G-Buffer + Hi-Z scaffolding
    gbuffer: Option<gbuffer::GBuffer>,
    hiz: Option<hiz::HiZPyramid>,

    // --- Pipelines & BGLs ---
    pipeline: wgpu::RenderPipeline,
    inst_pipeline: wgpu::RenderPipeline,
    wire_pipeline: Option<wgpu::RenderPipeline>,
    particle_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    post_ao_pipeline: wgpu::RenderPipeline,
    ssgi_pipeline: wgpu::RenderPipeline,
    ssr_pipeline: wgpu::RenderPipeline,
    present_pipeline: wgpu::RenderPipeline,
    blit_scene_read_pipeline: wgpu::RenderPipeline,
    bloom_pipeline: wgpu::RenderPipeline,
    bloom_bg: wgpu::BindGroup,
    direct_present: bool,
    lights_buf: wgpu::Buffer,
    // Stored bind group layouts needed to rebuild views on resize
    present_bgl: wgpu::BindGroupLayout,
    post_ao_bgl: wgpu::BindGroupLayout,
    #[allow(dead_code)]
    ssgi_globals_bgl: wgpu::BindGroupLayout,
    ssgi_depth_bgl: wgpu::BindGroupLayout,
    ssgi_scene_bgl: wgpu::BindGroupLayout,
    ssr_depth_bgl: wgpu::BindGroupLayout,
    ssr_scene_bgl: wgpu::BindGroupLayout,
    palettes_bgl: wgpu::BindGroupLayout,
    globals_bg: wgpu::BindGroup,
    post_ao_bg: wgpu::BindGroup,
    ssgi_globals_bg: wgpu::BindGroup,
    ssgi_depth_bg: wgpu::BindGroup,
    ssgi_scene_bg: wgpu::BindGroup,
    ssr_depth_bg: wgpu::BindGroup,
    ssr_scene_bg: wgpu::BindGroup,
    _post_sampler: wgpu::Sampler,
    point_sampler: wgpu::Sampler,
    sky_bg: wgpu::BindGroup,
    terrain_model_bg: wgpu::BindGroup,
    shard_model_bg: wgpu::BindGroup,
    present_bg: wgpu::BindGroup,
    // frame overlay removed

    // Lighting toggles
    enable_post_ao: bool,
    enable_ssgi: bool,
    enable_ssr: bool,
    enable_bloom: bool,
    static_index: Option<collision_static::StaticIndex>,
    #[allow(dead_code)]
    frame_counter: u32,
    // Stats
    draw_calls: u32,

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

    // Rocks (instanced static mesh)
    rocks_instances: wgpu::Buffer,
    rocks_count: u32,
    rocks_vb: wgpu::Buffer,
    rocks_ib: wgpu::Buffer,
    rocks_index_count: u32,

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
    zombie_ids: Vec<server_core::NpcId>,
    zombie_prev_pos: Vec<glam::Vec3>,
    // Per-instance forward-axis offsets (authoring → world). Calibrated on movement.
    zombie_forward_offsets: Vec<f32>,

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
    hud: ui::Hud,
    hud_model: ux_hud::HudModel,

    // --- Player/Camera ---
    pc_index: usize,
    player: client_core::controller::PlayerController,
    input: client_core::input::InputState,
    cam_follow: camera_sys::FollowState,
    pc_cast_queued: bool,
    pc_cast_kind: Option<PcCast>,
    pc_anim_start: Option<f32>,
    pc_cast_time: f32,
    pc_cast_fired: bool,
    // Simple Fire Bolt cooldown tracking (seconds)
    firebolt_cd_until: f32,
    firebolt_cd_dur: f32,
    // Deprecated GCD tracking (not used when cast-time only)
    #[allow(dead_code)]
    gcd_until: f32,
    #[allow(dead_code)]
    gcd_duration: f32,
    // Orbit params
    cam_orbit_yaw: f32,
    cam_orbit_pitch: f32,
    cam_distance: f32,
    cam_lift: f32,
    cam_look_height: f32,
    rmb_down: bool,
    last_cursor_pos: Option<(f64, f64)>,

    // UI capture helpers
    screenshot_start: Option<f32>,

    // No interactive death UI — we show text only.

    // Server state (NPCs/health)
    server: server_core::ServerState,

    // Wizard health (including PC at pc_index)
    wizard_hp: Vec<i32>,
    wizard_hp_max: i32,
    pc_alive: bool,
}

impl Renderer {
    #[inline]
    fn wrap_angle(a: f32) -> f32 {
        let mut x = a;
        while x > std::f32::consts::PI {
            x -= 2.0 * std::f32::consts::PI;
        }
        while x < -std::f32::consts::PI {
            x += 2.0 * std::f32::consts::PI;
        }
        x
    }
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

    fn respawn(&mut self) {
        // Rebuild scene and server similar to initial construction.
        // 1) Rebuild wizard scene instances and reset player state
        let terrain_extent = self.max_dim as f32 * 0.5;
        let ruins_base_offset = 0.4f32;
        let ruins_radius = 6.5f32;
        let scene_build = scene::build_demo_scene(
            &self.device,
            &self.skinned_cpu,
            terrain_extent,
            Some(&self.terrain_cpu),
            ruins_base_offset,
            ruins_radius,
        );
        // Snap to terrain heights again
        let mut wizard_models = scene_build.wizard_models.clone();
        for m in &mut wizard_models {
            let c = m.to_cols_array();
            let x = c[12];
            let z = c[14];
            let (h, _n) = terrain::height_at(&self.terrain_cpu, x, z);
            let pos = glam::vec3(x, h, z);
            let (s, r, _t) = glam::Mat4::from_cols_array(&c).to_scale_rotation_translation();
            *m = glam::Mat4::from_scale_rotation_translation(s, r, pos);
        }
        let mut wizard_instances_cpu = scene_build.wizard_instances_cpu.clone();
        for (i, inst) in wizard_instances_cpu.iter_mut().enumerate() {
            inst.model = wizard_models[i].to_cols_array_2d();
        }
        self.wizard_instances = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("wizard-instances"),
            contents: bytemuck::cast_slice(&wizard_instances_cpu),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        self.wizard_instances_cpu = wizard_instances_cpu;
        self.wizard_models = wizard_models;
        self.wizard_count = self.wizard_instances_cpu.len() as u32;
        self.wizard_anim_index = scene_build.wizard_anim_index;
        self.wizard_time_offset = scene_build.wizard_time_offset;
        self.wizard_last_phase = vec![0.0; self.wizard_count as usize];

        // 2) Reset player and camera
        let pc_initial_pos = {
            let m = scene_build.wizard_models[scene_build.pc_index];
            let c = m.to_cols_array();
            glam::vec3(c[12], c[13], c[14])
        };
        self.pc_index = scene_build.pc_index;
        self.player = client_core::controller::PlayerController::new(pc_initial_pos);
        self.input.clear();
        self.cam_follow = camera_sys::FollowState {
            current_pos: glam::vec3(0.0, 5.0, -10.0),
            current_look: scene_build.cam_target,
        };
        self.cam_orbit_yaw = 0.0;
        self.cam_orbit_pitch = 0.2;
        self.cam_distance = 8.5;
        self.cam_lift = 3.5;
        self.cam_look_height = 1.6;
        self.rmb_down = false;
        self.last_cursor_pos = None;
        self.pc_cast_queued = false;
        self.pc_anim_start = None;
        self.pc_cast_fired = false;

        // Reset HP and alive flag
        self.wizard_hp = vec![self.wizard_hp_max; self.wizard_count as usize];
        self.pc_alive = true;

        // 3) Reset server and zombies
        let npcs = npcs::build(&self.device, terrain_extent);
        self.npc_vb = npcs.vb;
        self.npc_ib = npcs.ib;
        self.npc_index_count = npcs.index_count;
        self.npc_instances = npcs.instances;
        self.npc_models = npcs.models;
        self.server = npcs.server;
        let (zinst, zcpu, zmodels, zids, zcount) =
            zombies::build_instances(&self.device, &self.terrain_cpu, &self.server, self.zombie_joints);
        self.zombie_instances = zinst;
        self.zombie_instances_cpu = zcpu;
        self.zombie_models = zmodels.clone();
        self.zombie_ids = zids;
        self.zombie_count = zcount;
        self.zombie_prev_pos = zmodels
            .iter()
            .map(|m| glam::vec3(m.to_cols_array()[12], m.to_cols_array()[13], m.to_cols_array()[14]))
            .collect();
        self.zombie_forward_offsets = vec![zombies::forward_offset(&self.zombie_cpu); self.zombie_count as usize];
        // Recreate zombie palette buffer sized for new count
        let total_z_mats = self.zombie_count as usize * self.zombie_joints as usize;
        self.zombie_palettes_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("zombie-palettes"),
            size: (total_z_mats * 64) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.zombie_palettes_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("zombie-palettes-bg"),
            layout: &self.palettes_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.zombie_palettes_buf.as_entire_binding(),
            }],
        });

        // 4) Clear FX
        self.projectiles.clear();
        self.particles.clear();
        log::info!("Respawn complete");
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
            // On macOS, try PRIMARY (Metal) first, then fall back to GL for stability if needed.
            &[wgpu::Backends::PRIMARY, wgpu::Backends::GL]
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
        let info = adapter.get_info();
        log::info!("Adapter: {:?} ({:?})", info.name, info.backend);
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

        // Downgrade uncaptured errors to logs so we can see details without panicking
        device.on_uncaptured_error(Box::new(|e| {
            log::error!("wgpu uncaptured error: {:?}", e);
        }));

        // --- Surface configuration (with clamping to device limits) ---
        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        // Use FIFO everywhere for stability across drivers; opt-in overrides can come later.
        let present_mode = wgpu::PresentMode::Fifo;
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
        // Offscreen SceneColor (HDR)
        let scene_color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene-color"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let scene_view = scene_color.create_view(&wgpu::TextureViewDescriptor::default());
        let scene_read = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene-read"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let scene_read_view = scene_read.create_view(&wgpu::TextureViewDescriptor::default());
        // Lighting M1: allocate G-Buffer attachments and linear depth (Hi-Z) pyramid
        let gbuffer = gbuffer::GBuffer::create(&device, config.width, config.height);
        let hiz = hiz::HiZPyramid::create(&device, config.width, config.height);

        // --- Pipelines + BGLs ---
        let shader = pipeline::create_shader(&device);
        let (globals_bgl, model_bgl) = pipeline::create_bind_group_layouts(&device);
        let palettes_bgl = pipeline::create_palettes_bgl(&device);
        let material_bgl = pipeline::create_material_bgl(&device);
        let offscreen_fmt = wgpu::TextureFormat::Rgba16Float;
        // Allow direct-present path: build main pipelines targeting swapchain format
        let direct_present = std::env::var("RA_DIRECT_PRESENT")
            .map(|v| v != "0")
            .unwrap_or(true);
        let draw_fmt = if direct_present {
            config.format
        } else {
            offscreen_fmt
        };
        let (pipeline, inst_pipeline, wire_pipeline) =
            pipeline::create_pipelines(&device, &shader, &globals_bgl, &model_bgl, draw_fmt);
        // Sky background
        let sky_bgl = pipeline::create_sky_bgl(&device);
        let sky_pipeline = pipeline::create_sky_pipeline(&device, &globals_bgl, &sky_bgl, draw_fmt);
        // Present pipeline (SceneColor -> swapchain)
        let present_bgl = pipeline::create_present_bgl(&device);
        let present_pipeline =
            pipeline::create_present_pipeline(&device, &globals_bgl, &present_bgl, config.format);
        let blit_scene_read_pipeline =
            pipeline::create_blit_pipeline(&device, &present_bgl, wgpu::TextureFormat::Rgba16Float);
        // Bloom
        let bloom_bgl = pipeline::create_bloom_bgl(&device);
        let bloom_pipeline =
            pipeline::create_bloom_pipeline(&device, &bloom_bgl, wgpu::TextureFormat::Rgba16Float);
        // (removed) frame overlay
        // Post AO pipeline
        let post_ao_bgl = pipeline::create_post_ao_bgl(&device);
        let post_ao_pipeline =
            pipeline::create_post_ao_pipeline(&device, &globals_bgl, &post_ao_bgl, offscreen_fmt);
        // SSGI pipeline (additive into SceneColor)
        let (ssgi_globals_bgl, ssgi_depth_bgl, ssgi_scene_bgl) = pipeline::create_ssgi_bgl(&device);
        let (ssr_depth_bgl, ssr_scene_bgl) = pipeline::create_ssr_bgl(&device);
        let ssgi_pipeline = pipeline::create_ssgi_pipeline(
            &device,
            &ssgi_globals_bgl,
            &ssgi_depth_bgl,
            &ssgi_scene_bgl,
            wgpu::TextureFormat::Rgba16Float,
        );
        let ssr_pipeline = pipeline::create_ssr_pipeline(
            &device,
            &ssr_depth_bgl,
            &ssr_scene_bgl,
            wgpu::TextureFormat::Rgba16Float,
        );
        let (wizard_pipeline, _wizard_wire_pipeline_unused) = pipeline::create_wizard_pipelines(
            &device,
            &shader,
            &globals_bgl,
            &model_bgl,
            &palettes_bgl,
            &material_bgl,
            draw_fmt,
        );
        let particle_pipeline =
            pipeline::create_particle_pipeline(&device, &shader, &globals_bgl, draw_fmt);

        // UI: nameplates + health bars — build against active color format (swapchain if direct-present)
        let nameplates = ui::Nameplates::new(&device, draw_fmt)?;
        let nameplates_npc = ui::Nameplates::new(&device, draw_fmt)?;
        let mut bars = ui::HealthBars::new(&device, draw_fmt)?;
        let hud = ui::Hud::new(&device, draw_fmt)?;
        let damage = ui::DamageFloaters::new(&device, draw_fmt)?;

        // --- Buffers & bind groups ---
        // Globals
        let globals_init = Globals {
            view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
            cam_right_time: [1.0, 0.0, 0.0, 0.0],
            cam_up_pad: [0.0, 1.0, 0.0, (60f32.to_radians() * 0.5).tan()],
            sun_dir_time: [0.0, 1.0, 0.0, 0.0],
            sh_coeffs: [[0.0, 0.0, 0.0, 0.0]; 9],
            fog_params: [0.0, 0.0, 0.0, 0.0],
            clip_params: [0.1, 1000.0, 1.0, 0.0],
        };
        let globals_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globals"),
            contents: bytemuck::bytes_of(&globals_init),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        // Dynamic lights buffer (packed into globals bind group at binding=1)
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct LightsRaw {
            count: u32,
            _pad: [f32; 3],
            pos_radius: [[f32; 4]; 16],
            color: [[f32; 4]; 16],
            // Trailing padding to satisfy stricter std140/uniform layout expectations on some drivers.
            // WGSL may round the struct size up; ensure our UBO is at least as large as the shader's view.
            _tail_pad: [f32; 4],
        }
        let lights_init = LightsRaw {
            count: 0,
            _pad: [0.0; 3],
            pos_radius: [[0.0; 4]; 16],
            color: [[0.0; 4]; 16],
            _tail_pad: [0.0; 4],
        };
        let lights_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("lights-ubo"),
            contents: bytemuck::bytes_of(&lights_init),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals-bg"),
            layout: &globals_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: lights_buf.as_entire_binding(),
                },
            ],
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
        // Apply optional time-of-day overrides from the Zone
        if let Some(frac) = zone.start_time_frac {
            sky_state.day_frac = frac.rem_euclid(1.0);
            sky_state.recompute();
        }
        if let Some(pause) = zone.start_paused {
            sky_state.paused = pause;
        }
        if let Some(scale) = zone.start_time_scale {
            sky_state.time_scale = scale.clamp(0.01, 1000.0);
        }
        log::info!(
            "Start TOD: day_frac={:.3} paused={} sun_elev={:.3}",
            sky_state.day_frac,
            sky_state.paused,
            sky_state.sun_dir.y
        );
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
        // Post AO bind group & sampler
        let post_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("post-ao-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        // Linear (filtering) sampler for color/depth that allow filtering
        let post_ao_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("post-ao-bg"),
            layout: &post_ao_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&depth),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&post_sampler),
                },
            ],
        });
        // Point (non-filtering) sampler for R32F linear depth sampling
        let point_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("point-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let ssgi_globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ssgi-globals-bg"),
            layout: &ssgi_globals_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buf.as_entire_binding(),
            }],
        });
        let ssgi_depth_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ssgi-depth-bg"),
            layout: &ssgi_depth_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&depth),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&post_sampler),
                },
            ],
        });
        let ssgi_scene_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ssgi-scene-bg"),
            layout: &ssgi_scene_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&scene_read_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&post_sampler),
                },
            ],
        });
        // SSR bind groups reference linear depth (Hi-Z mip chain view) and SceneRead
        let ssr_depth_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ssr-depth-bg"),
            layout: &ssr_depth_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&hiz.linear_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&point_sampler),
                },
            ],
        });
        let ssr_scene_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ssr-scene-bg"),
            layout: &ssr_scene_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&scene_read_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&post_sampler),
                },
            ],
        });
        let present_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("present-bg"),
            layout: &present_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&scene_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&post_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&depth),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&point_sampler),
                },
            ],
        });
        // Bloom bind group reads from SceneRead (copy of SceneColor)
        let bloom_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bloom-bg"),
            layout: &bloom_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&scene_read_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&post_sampler),
                },
            ],
        });
        // (Lights UBO is part of globals bind group; see earlier)

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

        // Attempt to load packed static colliders from packs/zones/<slug>/snapshot.v1
        let static_index = {
            let path = crate::gfx::asset_path(&format!(
                "packs/zones/{}/snapshot.v1/colliders.bin",
                zone.slug
            ));
            let idxp = crate::gfx::asset_path(&format!(
                "packs/zones/{}/snapshot.v1/colliders_index.bin",
                zone.slug
            ));
            if path.exists() && idxp.exists() {
                if let (Ok(bytes), Ok(_idx_bytes)) = (std::fs::read(&path), std::fs::read(&idxp)) {
                    let mut colliders: Vec<collision_static::StaticCollider> = Vec::new();
                    let mut i = 0usize;
                    while i + 48 <= bytes.len() {
                        // proto_id (2), shape (2)
                        let _proto = u16::from_le_bytes([bytes[i], bytes[i + 1]]);
                        i += 2;
                        let shape = u16::from_le_bytes([bytes[i], bytes[i + 1]]);
                        i += 2;
                        let f = |j: &mut usize| -> f32 {
                            let v = f32::from_le_bytes(bytes[*j..*j + 4].try_into().unwrap());
                            *j += 4;
                            v
                        };
                        let cx = f(&mut i);
                        let cy = f(&mut i);
                        let cz = f(&mut i);
                        let radius = f(&mut i);
                        let half_h = f(&mut i);
                        let minx = f(&mut i);
                        let miny = f(&mut i);
                        let minz = f(&mut i);
                        let maxx = f(&mut i);
                        let maxy = f(&mut i);
                        let maxz = f(&mut i);
                        // chunk_id
                        let _ = u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
                        i += 4;
                        let aabb = collision_static::Aabb {
                            min: glam::vec3(minx, miny, minz),
                            max: glam::vec3(maxx, maxy, maxz),
                        };
                        if shape == 0 {
                            let cyl = collision_static::CylinderY {
                                center: glam::vec3(cx, cy, cz),
                                radius,
                                half_height: half_h,
                            };
                            colliders.push(collision_static::StaticCollider {
                                aabb,
                                shape: collision_static::ShapeRef::Cyl(cyl),
                            });
                        }
                    }
                    Some(collision_static::StaticIndex { colliders })
                } else {
                    None
                }
            } else {
                None
            }
        };

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
        // Ruins mesh + metrics
        let ruins_gpu = ruins::build_ruins(&device).context("build ruins mesh")?;
        let ruins_base_offset = ruins_gpu.base_offset;
        let ruins_radius = ruins_gpu.radius;

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

        // Zombie vertex/index buffers (skinned)
        let zombie_assets = zombies::load_assets(&device).context("load zombie assets")?;
        let zombie_cpu = zombie_assets.cpu;
        let zombie_vb = zombie_assets.vb;
        let zombie_ib = zombie_assets.ib;
        let zombie_index_count = zombie_assets.index_count;

        let (ruins_vb, ruins_ib, ruins_index_count) =
            (ruins_gpu.vb, ruins_gpu.ib, ruins_gpu.index_count);

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

        // NPC rings: cube instances + server state for zombies
        let npcs = npcs::build(&device, terrain_extent);
        let npc_vb = npcs.vb;
        let npc_ib = npcs.ib;
        let npc_index_count = npcs.index_count;
        let npc_instances = npcs.instances;
        let npc_models = npcs.models;
        let server = npcs.server;

        // Trees: delegate to foliage module for instance generation and mesh upload
        let veg = zone
            .vegetation
            .as_ref()
            .map(|v| (v.tree_count as usize, v.tree_seed));
        let trees_gpu = foliage::build_trees(&device, &terrain_cpu, &zone.slug, veg)
            .context("build trees (instances + mesh) for zone")?;
        let trees_instances = trees_gpu.instances;
        let trees_count = trees_gpu.count;
        let (trees_vb, trees_ib, trees_index_count) =
            (trees_gpu.vb, trees_gpu.ib, trees_gpu.index_count);

        // Rocks: load GLB and scatter instances
        let rocks_gpu = rocks::build_rocks(&device, &terrain_cpu, &zone.slug, None)
            .context("build rocks (instances + mesh) for zone")?;
        let rocks_instances = rocks_gpu.instances;
        let rocks_count = rocks_gpu.count;
        let (rocks_vb, rocks_ib, rocks_index_count) =
            (rocks_gpu.vb, rocks_gpu.ib, rocks_gpu.index_count);

        // Atlases
        // Upload UI atlases
        nameplates.upload_atlas(&queue);
        nameplates_npc.upload_atlas(&queue);
        bars.queue_entries(
            &device,
            &queue,
            config.width,
            config.height,
            glam::Mat4::IDENTITY,
            &[],
        );
        hud.upload_atlas(&queue);
        // Build zombie instances from server NPCs
        let (zombie_instances, zombie_instances_cpu, zombie_models, zombie_ids, zombie_count) =
            zombies::build_instances(&device, &terrain_cpu, &server, zombie_joints);
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
        let zombie_forward_offset = zombies::forward_offset(&zombie_cpu);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size: PhysicalSize::new(w, h),
            max_dim,
            depth,
            scene_color,
            scene_view,
            scene_read,
            scene_read_view,

            pipeline,
            inst_pipeline,
            wire_pipeline,
            particle_pipeline,
            sky_pipeline,
            post_ao_pipeline,
            ssgi_pipeline,
            ssr_pipeline,
            present_pipeline,
            blit_scene_read_pipeline,
            bloom_pipeline,
            bloom_bg,
            lights_buf,
            direct_present,
            static_index,
            present_bgl: present_bgl.clone(),
            post_ao_bgl: post_ao_bgl.clone(),
            ssgi_globals_bgl: ssgi_globals_bgl.clone(),
            ssgi_depth_bgl: ssgi_depth_bgl.clone(),
            ssgi_scene_bgl: ssgi_scene_bgl.clone(),
            ssr_depth_bgl: ssr_depth_bgl.clone(),
            ssr_scene_bgl: ssr_scene_bgl.clone(),
            palettes_bgl: palettes_bgl.clone(),
            globals_bg,
            post_ao_bg,
            ssgi_globals_bg,
            ssgi_depth_bg,
            ssgi_scene_bg,
            ssr_depth_bg,
            ssr_scene_bg,
            _post_sampler: post_sampler,
            point_sampler,
            sky_bg,
            present_bg,
            // frame overlay removed
            frame_counter: 0,
            draw_calls: 0,
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
            zombie_forward_offsets: vec![zombie_forward_offset; zombie_count as usize],
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
            hud,
            hud_model: Default::default(),

            // Player/camera
            pc_index: scene_build.pc_index,
            player: client_core::controller::PlayerController::new(pc_initial_pos),
            input: Default::default(),
            cam_follow: camera_sys::FollowState {
                current_pos: glam::vec3(0.0, 5.0, -10.0),
                current_look: scene_build.cam_target,
            },
            pc_cast_queued: false,
            pc_cast_kind: Some(PcCast::FireBolt),
            pc_anim_start: None,
            pc_cast_time: 1.5,
            pc_cast_fired: false,
            firebolt_cd_until: 0.0,
            firebolt_cd_dur: 1.0,
            gcd_until: 0.0,
            gcd_duration: 1.5,
            cam_orbit_yaw: 0.0,
            cam_orbit_pitch: 0.2,
            cam_distance: 8.5,
            cam_lift: 3.5,
            cam_look_height: 1.6,
            rmb_down: false,
            last_cursor_pos: None,
            screenshot_start: None,

            npc_vb,
            npc_ib,
            npc_index_count,
            npc_instances,
            npc_count: 0, // cubes hidden; zombies replace them visually
            npc_instances_cpu: Vec::new(),
            npc_models,
            trees_instances,
            trees_count,
            trees_vb,
            trees_ib,
            trees_index_count,
            rocks_instances,
            rocks_count,
            rocks_vb,
            rocks_ib,
            rocks_index_count,
            server,
            wizard_hp: vec![100; scene_build.wizard_count as usize],
            wizard_hp_max: 100,
            pc_alive: true,
            // Lighting M1 scaffolding (disabled by default to avoid outline artifacts)
            gbuffer: Some(gbuffer),
            hiz: Some(hiz),
            enable_post_ao: false,
            enable_ssgi: false,
            enable_ssr: false,
            // Enable bloom by default to accent bright fire bolts
            enable_bloom: true,
            // frame overlay removed
        })
    }

    /// Resize the swapchain while preserving aspect and device limits.
    #[allow(unreachable_code)]
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        return renderer::resize::resize_impl(self, new_size);
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
        // Recreate SceneColor
        self.scene_color = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene-color"),
            size: wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.scene_view = self
            .scene_color
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.scene_read = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene-read"),
            size: wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.scene_read_view = self
            .scene_read
            .create_view(&wgpu::TextureViewDescriptor::default());
        // Rebuild bind groups that reference resized textures
        self.present_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("present-bg"),
            layout: &self.present_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.scene_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self._post_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.depth),
                },
            ],
        });
        self.post_ao_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("post-ao-bg"),
            layout: &self.post_ao_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.depth),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self._post_sampler),
                },
            ],
        });
        self.ssgi_depth_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ssgi-depth-bg"),
            layout: &self.ssgi_depth_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.depth),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self._post_sampler),
                },
            ],
        });
        self.ssgi_scene_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ssgi-scene-bg"),
            layout: &self.ssgi_scene_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.scene_read_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self._post_sampler),
                },
            ],
        });
        if let Some(hiz) = &self.hiz {
            self.ssr_depth_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ssr-depth-bg"),
                layout: &self.ssr_depth_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&hiz.linear_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.point_sampler),
                    },
                ],
            });
        }
        self.ssr_scene_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ssr-scene-bg"),
            layout: &self.ssr_scene_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.scene_read_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self._post_sampler),
                },
            ],
        });
        // Resize Lighting M1 resources
        self.gbuffer = Some(gbuffer::GBuffer::create(
            &self.device,
            self.config.width,
            self.config.height,
        ));
        self.hiz = Some(hiz::HiZPyramid::create(
            &self.device,
            self.config.width,
            self.config.height,
        ));
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
        // Reset per-frame stats
        self.draw_calls = 0;

        // If screenshot mode is active, auto-animate a smooth orbit for 5 seconds
        if let Some(ts) = self.screenshot_start {
            let elapsed = (t - ts).max(0.0);
            if elapsed <= 5.0 {
                let speed = 0.6; // rad/s
                self.cam_orbit_yaw = Self::wrap_angle(self.cam_orbit_yaw + dt * speed);
            } else {
                self.screenshot_start = None;
            }
        }

        // Update player transform from input (WASD) then camera follow
        self.update_player_and_camera(dt, aspect);
        // Simple AI: rotate non-PC wizards to face nearest alive zombie so firebolts aim correctly
        self.update_wizard_ai(dt);
        // Compute local orbit offsets (relative to PC orientation)
        // Adapt lift and look height as we zoom in so the close view
        // sits just behind and slightly above the wizard's head.
        let near_d = 1.6f32;
        let far_d = 25.0f32;
        let zoom_t = ((self.cam_distance - near_d) / (far_d - near_d)).clamp(0.0, 1.0);
        let near_lift = 0.5f32; // meters above anchor when fully zoomed-in
        let near_look = 0.5f32; // aim point above anchor when fully zoomed-in
        let eff_lift = near_lift * (1.0 - zoom_t) + self.cam_lift * zoom_t;
        let eff_look = near_look * (1.0 - zoom_t) + self.cam_look_height * zoom_t;
        let (off_local, look_local) = camera_sys::compute_local_orbit_offsets(
            self.cam_distance,
            self.cam_orbit_yaw,
            self.cam_orbit_pitch,
            eff_lift,
            eff_look,
        );
        #[allow(unused_assignments)]
        // Anchor camera to the center of the PC model, not the feet.
        let pc_anchor = if self.pc_alive {
            if self.pc_index < self.wizard_models.len() {
                let m = self.wizard_models[self.pc_index];
                (m * glam::Vec4::new(0.0, 1.2, 0.0, 1.0)).truncate()
            } else {
                self.player.pos + glam::vec3(0.0, 1.2, 0.0)
            }
        } else {
            // When dead, keep camera around the last known player position instead of the hidden model.
            self.player.pos + glam::vec3(0.0, 1.2, 0.0)
        };

        // While RMB is held, snap follow (no lag); otherwise use smoothed dt
        let follow_dt = if self.rmb_down { 1.0 } else { dt };
        let _ = camera_sys::third_person_follow(
            &mut self.cam_follow,
            pc_anchor,
            glam::Quat::from_rotation_y(self.player.yaw),
            off_local,
            look_local,
            aspect,
            follow_dt,
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
        let (_cam2, mut globals) = camera_sys::third_person_follow(
            &mut self.cam_follow,
            pc_anchor,
            glam::Quat::from_rotation_y(self.player.yaw),
            off_local,
            look_local,
            aspect,
            0.0,
        );
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
        // Fog color/density based on time-of-day: dark navy at night, light blue by day.
        // Night fog helps suppress the horizon band and prevents the sky from reading pink.
        if self.sky.sun_dir.y <= 0.0 {
            globals.fog_params = [0.01, 0.015, 0.02, 0.018];
        } else {
            globals.fog_params = [0.6, 0.7, 0.8, 0.0035];
        }
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
                    log::debug!(
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
            for p in &self.projectiles {
                if n >= 16 {
                    break;
                }
                raw.pos_radius[n] = [p.pos.x, p.pos.y, p.pos.z, maxr];
                raw.color[n] = [3.0, 1.2, 0.4, 0.0];
                n += 1;
            }
            raw.count = n as u32;
            // Write packed lights into globals bind group (binding=1 holds the lights UBO)
            self.queue
                .write_buffer(&self.lights_buf, 0, bytemuck::bytes_of(&raw));
        }

        // Begin commands
        // Capture validation errors locally to avoid process-wide panic
        self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encoder"),
            });
        // Optional safety mode to bypass offscreen passes
        let present_only = std::env::var("RA_PRESENT_ONLY")
            .map(|v| v == "1")
            .unwrap_or(false);
        // Direct-present path: render directly to the swapchain view instead of SceneColor
        let render_view: &wgpu::TextureView = if self.direct_present {
            &view
        } else {
            &self.scene_view
        };
        // Sky-only pass (no depth)
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
            sky.set_pipeline(&self.sky_pipeline);
            sky.set_bind_group(0, &self.globals_bg, &[]);
            sky.set_bind_group(1, &self.sky_bg, &[]);
            sky.draw(0..3, 0..1);
            self.draw_calls += 1;
        }
        // Main pass with depth; load color from sky pass when offscreen
        log::debug!("pass: main");
        if !present_only {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
                    view: &self.depth,
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
                    self.device.push_error_scope(wgpu::ErrorFilter::Validation);
                }
                rpass.set_pipeline(&self.pipeline);
                rpass.set_bind_group(0, &self.globals_bg, &[]);
                rpass.set_bind_group(1, &self.terrain_model_bg, &[]);
                // lights are packed into globals (binding=1)
                rpass.set_vertex_buffer(0, self.terrain_vb.slice(..));
                rpass.set_index_buffer(self.terrain_ib.slice(..), wgpu::IndexFormat::Uint16);
                rpass.draw_indexed(0..self.terrain_index_count, 0, 0..1);
                self.draw_calls += 1;
                if trace && let Some(e) = pollster::block_on(self.device.pop_error_scope()) {
                    log::error!("validation after terrain: {:?}", e);
                }
            }

            // Trees (instanced static mesh)
            if self.trees_count > 0 {
                log::debug!("draw: trees x{}", self.trees_count);
                if trace {
                    self.device.push_error_scope(wgpu::ErrorFilter::Validation);
                }
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
                rpass.set_index_buffer(self.trees_ib.slice(..), wgpu::IndexFormat::Uint16);
                rpass.draw_indexed(0..self.trees_index_count, 0, 0..self.trees_count);
                self.draw_calls += 1;
                if trace && let Some(e) = pollster::block_on(self.device.pop_error_scope()) {
                    log::error!("validation after trees: {:?}", e);
                }
            }

            // Rocks (instanced static mesh)
            if self.rocks_count > 0 {
                log::debug!("draw: rocks x{}", self.rocks_count);
                if trace {
                    self.device.push_error_scope(wgpu::ErrorFilter::Validation);
                }
                let inst_pipe = if self.wire_enabled {
                    self.wire_pipeline.as_ref().unwrap_or(&self.inst_pipeline)
                } else {
                    &self.inst_pipeline
                };
                rpass.set_pipeline(inst_pipe);
                rpass.set_bind_group(0, &self.globals_bg, &[]);
                rpass.set_bind_group(1, &self.shard_model_bg, &[]);
                rpass.set_vertex_buffer(0, self.rocks_vb.slice(..));
                rpass.set_vertex_buffer(1, self.rocks_instances.slice(..));
                rpass.set_index_buffer(self.rocks_ib.slice(..), wgpu::IndexFormat::Uint16);
                rpass.draw_indexed(0..self.rocks_index_count, 0, 0..self.rocks_count);
                self.draw_calls += 1;
                if trace && let Some(e) = pollster::block_on(self.device.pop_error_scope()) {
                    log::error!("validation after rocks: {:?}", e);
                }
            }

            // Wizards
            if std::env::var("RA_DRAW_WIZARDS")
                .map(|v| v != "0")
                .unwrap_or(true)
            {
                log::debug!("draw: wizards x{}", self.wizard_count);
                if trace {
                    self.device.push_error_scope(wgpu::ErrorFilter::Validation);
                }
                self.draw_wizards(&mut rpass);
                self.draw_calls += 1;
                if trace && let Some(e) = pollster::block_on(self.device.pop_error_scope()) {
                    log::error!("validation after wizards: {:?}", e);
                }
            } else {
                log::debug!("draw: wizards skipped (RA_DRAW_WIZARDS=0)");
            }
            // Zombies
            if std::env::var("RA_DRAW_ZOMBIES")
                .map(|v| v != "0")
                .unwrap_or(true)
            {
                log::debug!("draw: zombies x{}", self.zombie_count);
                if trace {
                    self.device.push_error_scope(wgpu::ErrorFilter::Validation);
                }
                self.draw_zombies(&mut rpass);
                self.draw_calls += 1;
                if trace && let Some(e) = pollster::block_on(self.device.pop_error_scope()) {
                    log::error!("validation after zombies: {:?}", e);
                }
            } else {
                log::debug!("draw: zombies skipped (RA_DRAW_ZOMBIES=0)");
            }

            // Ruins (instanced) — draw by default; allow disabling with RA_DRAW_RUINS=0
            let draw_ruins = std::env::var("RA_DRAW_RUINS")
                .map(|v| v != "0")
                .unwrap_or(true);
            if draw_ruins && self.ruins_count > 0 {
                log::debug!("draw: ruins x{} (enabled)", self.ruins_count);
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
                rpass.set_index_buffer(self.ruins_ib.slice(..), wgpu::IndexFormat::Uint16);
                rpass.draw_indexed(0..self.ruins_index_count, 0, 0..self.ruins_count);
                self.draw_calls += 1;
            } else {
                log::debug!("draw: ruins skipped (RA_DRAW_RUINS!=1)");
            }

            // NPCs (instanced cubes)
            if std::env::var("RA_DRAW_NPCS")
                .map(|v| v == "1")
                .unwrap_or(false)
                && self.npc_count > 0
            {
                log::debug!("draw: npcs x{}", self.npc_count);
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
                rpass.set_index_buffer(self.npc_ib.slice(..), wgpu::IndexFormat::Uint16);
                rpass.draw_indexed(0..self.npc_index_count, 0, 0..self.npc_count);
                self.draw_calls += 1;
            } else if self.npc_count > 0 {
                log::debug!("draw: npcs skipped (RA_DRAW_NPCS!=1)");
            }

            // FX
            self.draw_particles(&mut rpass);
            if self.fx_count > 0 {
                self.draw_calls += 1;
            }
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
        let mut npc_map: HashMap<server_core::NpcId, (i32, i32, bool, f32)> = HashMap::new();
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
        // Health bars (default on; set RA_OVERLAYS=0 to hide)
        if std::env::var("RA_OVERLAYS")
            .map(|v| v == "0")
            .unwrap_or(false)
        {
            // disabled
        } else {
            self.bars.queue_entries(
                &self.device,
                &self.queue,
                self.config.width,
                self.config.height,
                view_proj,
                &bar_entries,
            );
            // Draw bars to the active render target
            let target_view = if self.direct_present {
                &view
            } else {
                &self.scene_view
            };
            self.bars.draw(&mut encoder, target_view);
        }
        // Damage numbers (temporarily disabled while isolating a macOS validation issue)
        // self.damage.update(dt);
        // self.damage.queue(
        //     &self.device,
        //     &self.queue,
        //     self.config.width,
        //     self.config.height,
        //     view_proj,
        // );
        // self.damage.draw(&mut encoder, &self.scene_view);

        // Draw wizard nameplates first
        // Draw wizard nameplates for alive wizards only (hide dead PC/NPC labels)
        let mut wiz_alive: Vec<glam::Mat4> = Vec::new();
        for (i, m) in self.wizard_models.iter().enumerate() {
            let hp = self.wizard_hp.get(i).copied().unwrap_or(0);
            if hp > 0 {
                wiz_alive.push(*m);
            }
        }
        // Nameplates default on; set RA_OVERLAYS=0 to hide
        let draw_labels = std::env::var("RA_OVERLAYS")
            .map(|v| v != "0")
            .unwrap_or(true);
        if draw_labels {
            let target_view = if self.direct_present {
                &view
            } else {
                &self.scene_view
            };
            self.nameplates.queue_labels(
                &self.device,
                &self.queue,
                self.config.width,
                self.config.height,
                view_proj,
                &wiz_alive,
            );
            self.nameplates.draw(&mut encoder, target_view);
        }

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
        if draw_labels && !npc_positions.is_empty() {
            let target_view = if self.direct_present {
                &view
            } else {
                &self.scene_view
            };
            self.nameplates_npc.queue_npc_labels(
                &self.device,
                &self.queue,
                self.config.width,
                self.config.height,
                view_proj,
                &npc_positions,
                "Zombie",
            );
            self.nameplates_npc.draw(&mut encoder, target_view);
        }

        // Temporarily disable Hi-Z pyramid to isolate a macOS validation crash
        if false {
            let Some(hiz) = &self.hiz else { unreachable!() };
            let znear = 0.1f32; // mirrors Globals.clip.x
            let zfar = 1000.0f32; // mirrors Globals.clip.y
            hiz.build_mips(
                &self.device,
                &mut encoder,
                &self.depth,
                &self._post_sampler,
                znear,
                zfar,
            );
        }

        // Copy SceneColor to a read-only texture when SSR or SSGI need it
        if !present_only && (self.enable_ssgi || self.enable_ssr) {
            let mut blit = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit-scene-to-read"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.scene_read_view,
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
            blit.set_pipeline(&self.blit_scene_read_pipeline);
            blit.set_bind_group(0, &self.present_bg, &[]);
            blit.draw(0..3, 0..1);
            self.draw_calls += 1;
        }
        log::debug!("end: main pass");

        // Minimal mode: submit immediately after main pass and present
        if std::env::var("RA_MINIMAL")
            .map(|v| v == "1")
            .unwrap_or(false)
        {
            log::debug!("submit: minimal");
            self.queue.submit(Some(encoder.finish()));
            if let Some(e) = pollster::block_on(self.device.pop_error_scope()) {
                log::error!("wgpu validation error (minimal mode): {:?}", e);
                // Don’t panic; render loop continues
                return Ok(());
            }
            frame.present();
            return Ok(());
        }
        // Ensure SceneRead is available for bloom pass as well
        if !present_only && self.enable_bloom {
            let mut blit = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit-scene-to-read(bloom)"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.scene_read_view,
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
            blit.set_pipeline(&self.blit_scene_read_pipeline);
            blit.set_bind_group(0, &self.present_bg, &[]);
            blit.draw(0..3, 0..1);
        }

        // SSR overlay into SceneColor (alpha blend)
        if !present_only && self.enable_ssr {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ssr-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.scene_view,
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
            rp.set_pipeline(&self.ssr_pipeline);
            // linear depth (Hi-Z mip chain) + scene read
            rp.set_bind_group(0, &self.ssr_depth_bg, &[]);
            rp.set_bind_group(1, &self.ssr_scene_bg, &[]);
            rp.draw(0..3, 0..1);
            self.draw_calls += 1;
        }

        // SSGI additive overlay (fullscreen) into SceneColor
        if !present_only && self.enable_ssgi {
            let mut gi = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ssgi-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.scene_view,
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
            gi.set_pipeline(&self.ssgi_pipeline);
            gi.set_bind_group(0, &self.ssgi_globals_bg, &[]);
            gi.set_bind_group(1, &self.ssgi_depth_bg, &[]);
            gi.set_bind_group(2, &self.ssgi_scene_bg, &[]);
            gi.draw(0..3, 0..1);
            self.draw_calls += 1;
        }
        // (removed) frame overlay
        // (frame overlay removed)
        // Post-process AO overlay (fullscreen) multiplying into SceneColor
        if !present_only && self.enable_post_ao {
            {
                let mut post = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("post-ao"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.scene_view,
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
                post.set_pipeline(&self.post_ao_pipeline);
                post.set_bind_group(0, &self.globals_bg, &[]);
                post.set_bind_group(1, &self.post_ao_bg, &[]);
                post.draw(0..3, 0..1);
                self.draw_calls += 1;
            }
        }

        // Bloom additive overlay
        if self.enable_bloom {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bloom-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.scene_view,
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
            rp.set_pipeline(&self.bloom_pipeline);
            rp.set_bind_group(0, &self.bloom_bg, &[]);
            rp.draw(0..3, 0..1);
        }

        // Present: if rendering directly to swapchain, skip this pass
        if !self.direct_present {
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
            present.set_pipeline(&self.present_pipeline);
            present.set_bind_group(0, &self.globals_bg, &[]);
            present.set_bind_group(1, &self.present_bg, &[]);
            present.draw(0..3, 0..1);
            self.draw_calls += 1;
        }

        // Submit only if no validation errors occurred
        // Pop error scope AFTER submitting to ensure validation covers command submission
        if let Some(e) = pollster::block_on(self.device.pop_error_scope()) {
            // Skip submit on validation error to keep running without panicking
            log::error!("wgpu validation error (skipping frame): {:?}", e);
        } else {
            log::debug!("submit: normal path");
            // HUD: build, upload, and draw overlay before submit
            let pc_hp = self
                .wizard_hp
                .get(self.pc_index)
                .copied()
                .unwrap_or(self.wizard_hp_max);
            // Casting progress (0..1) while PortalOpen is active for the PC
            let cast_frac = if let Some(start) = self.pc_anim_start {
                if self.wizard_anim_index[self.pc_index] == 0 {
                    let dur = self.pc_cast_time.max(0.0001);
                    ((t - start) / dur).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            } else {
                0.0
            };
            // Hotbar overlay (slot 1): show Fire Bolt cooldown fraction
            let gcd_frac = if self.last_time < self.firebolt_cd_until && self.firebolt_cd_dur > 0.0 {
                ((self.firebolt_cd_until - self.last_time) / self.firebolt_cd_dur).clamp(0.0, 1.0)
            } else {
                0.0
            };
            // HUD (default on; set RA_OVERLAYS=0 to hide)
            let overlays_disabled = std::env::var("RA_OVERLAYS")
                .map(|v| v == "0")
                .unwrap_or(false);
            if !self.pc_alive {
                // Show death overlay regardless of RA_OVERLAYS setting
                self.hud.reset();
                self.hud.death_overlay(
                    self.size.width,
                    self.size.height,
                    "You died.",
                    "Press R to respawn",
                );
            } else if !overlays_disabled {
                let cast_label = if cast_frac > 0.0 {
                    match self.pc_cast_kind.unwrap_or(PcCast::FireBolt) {
                        PcCast::FireBolt => Some("Fire Bolt"),
                        PcCast::MagicMissile => Some("Magic Missile"),
                    }
                } else {
                    None
                };
                self.hud.build(
                    self.size.width,
                    self.size.height,
                    pc_hp,
                    self.wizard_hp_max,
                    cast_frac,
                    gcd_frac,
                    cast_label,
                );
                if self.hud_model.perf_enabled() {
                    let ms = dt * 1000.0;
                    let fps = if dt > 1e-5 { 1.0 / dt } else { 0.0 };
                    let line = format!("{:.2} ms  {:.0} FPS  {} draws", ms, fps, self.draw_calls);
                    self.hud
                        .append_perf_text(self.size.width, self.size.height, &line);
                }
            }
            // Queue+draw HUD (either normal or death overlay)
            self.hud.queue(&self.device, &self.queue);
            self.hud.draw(&mut encoder, &view);
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
        // Build quick lookup for attack state and radius using server NPCs
        use std::collections::HashMap;
        let mut attack_map: HashMap<server_core::NpcId, bool> = HashMap::new();
        let mut radius_map: HashMap<server_core::NpcId, f32> = HashMap::new();
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
        let find_clip = |subs: &[&str],
                         anims: &std::collections::HashMap<String, AnimClip>|
         -> Option<String> {
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
                || find_clip(
                    &["attack", "punch", "hit", "swipe", "slash", "bite"],
                    &self.zombie_cpu.animations,
                )
                .is_some();
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
            let zid = *self.zombie_ids.get(i).unwrap_or(&server_core::NpcId(0));
            let mut is_attacking = attack_map.get(&zid).copied().unwrap_or(false);
            // In-contact heuristic: nearest wizard within (z_radius + wizard_r + pad)
            let z_radius = radius_map.get(&zid).copied().unwrap_or(0.95);
            let wizard_r = 0.7f32;
            // Use a slightly larger pad than the server to keep the attack anim
            // engaged while in close proximity.
            let pad = 0.20f32;
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
                } else if let Some(_n) = find_clip(
                    &["attack", "punch", "hit", "swipe", "slash", "bite"],
                    &self.zombie_cpu.animations,
                ) {
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
                && let Some(n) = find_clip(
                    &["attack", "punch", "hit", "swipe", "slash", "bite"],
                    &self.zombie_cpu.animations,
                )
            {
                proc_name_str = Some(n);
            }
            let t = time_global + self.zombie_time_offset.get(i).copied().unwrap_or(0.0);
            let lookup = proc_name_str.as_deref().unwrap_or(clip_name);
            if let Some(clip) = self.zombie_cpu.animations.get(lookup) {
                let palette = anim::sample_palette(&self.zombie_cpu, clip, t);
                // Upload this instance's palette directly at its offset
                let mut raw: Vec<[f32; 16]> = Vec::with_capacity(joints);
                for m in palette {
                    raw.push(m.to_cols_array());
                }
                let byte_off = (i * joints * 64) as u64;
                self.queue.write_buffer(
                    &self.zombie_palettes_buf,
                    byte_off,
                    bytemuck::cast_slice(&raw),
                );
            }
        }
        // No single bulk upload; we wrote per-instance segments above.
    }

    fn update_zombies_from_server(&mut self) {
        // Build map from id -> pos
        use std::collections::HashMap;
        let mut pos_map: HashMap<server_core::NpcId, glam::Vec3> = HashMap::new();
        for n in &self.server.npcs {
            pos_map.insert(n.id, n.pos);
        }
        // Collect wizard positions to orient zombies toward nearest when stationary/attacking
        let mut wiz_pos: Vec<glam::Vec3> = Vec::with_capacity(self.wizard_models.len());
        for m in &self.wizard_models {
            let c = m.to_cols_array();
            wiz_pos.push(glam::vec3(c[12], c[13], c[14]));
        }
        let mut any = false;
        for (i, id) in self.zombie_ids.clone().iter().enumerate() {
            if let Some(p) = pos_map.get(id) {
                let m_old = self.zombie_models[i];
                let prev = self.zombie_prev_pos.get(i).copied().unwrap_or(*p);
                // If the zombie moved this frame, face the movement direction and calibrate per-instance offset.
                // Apply authoring forward-axis correction so models authored with
                // +X (or -Z) forward still look where they walk.
                let delta = *p - prev;
                let mut yaw = if delta.length_squared() > 1e-5 {
                    let desired = delta.x.atan2(delta.z);
                    let current = Self::yaw_from_model(&m_old);
                    let error = Self::wrap_angle(current - desired);
                    if let Some(off) = self.zombie_forward_offsets.get_mut(i) {
                        // Smoothly track observed error so facing matches velocity.
                        let k = 0.3f32;
                        *off = Self::wrap_angle(*off * (1.0 - k) + error * k);
                        desired - *off
                    } else {
                        desired
                    }
                } else {
                    // Stationary: orient toward nearest wizard so attack swings face the target
                    let mut best_d2 = f32::INFINITY;
                    let mut face_to: Option<glam::Vec3> = None;
                    for w in &wiz_pos {
                        let dx = w.x - p.x;
                        let dz = w.z - p.z;
                        let d2 = dx * dx + dz * dz;
                        if d2 < best_d2 {
                            best_d2 = d2;
                            face_to = Some(*w);
                        }
                    }
                    if let Some(tgt) = face_to {
                        let desired = (tgt.x - p.x).atan2(tgt.z - p.z);
                        if let Some(off) = self.zombie_forward_offsets.get(i) {
                            desired - *off
                        } else {
                            desired
                        }
                    } else {
                        Self::yaw_from_model(&m_old)
                    }
                };
                // If in melee contact, hard-face the nearest wizard regardless of small movements
                let mut best_d2 = f32::INFINITY;
                let mut face_to: Option<glam::Vec3> = None;
                for w in &wiz_pos {
                    let dx = w.x - p.x;
                    let dz = w.z - p.z;
                    let d2 = dx * dx + dz * dz;
                    if d2 < best_d2 {
                        best_d2 = d2;
                        face_to = Some(*w);
                    }
                }
                if let Some(tgt) = face_to {
                    // Obtain this zombie's radius from server
                    let z_radius = self
                        .server
                        .npcs
                        .iter()
                        .find(|n| n.id == *id)
                        .map(|n| n.radius)
                        .unwrap_or(0.95);
                    let wizard_r = 0.7f32;
                    let pad = 0.20f32;
                    let contact = z_radius + wizard_r + pad;
                    if best_d2 <= contact * contact {
                        if let Some(off) = self.zombie_forward_offsets.get(i) {
                            yaw = (tgt.x - p.x).atan2(tgt.z - p.z) - *off;
                        } else {
                            yaw = (tgt.x - p.x).atan2(tgt.z - p.z);
                        }
                    }
                }
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
                    PhysicalKey::Code(KeyCode::Digit1) | PhysicalKey::Code(KeyCode::Numpad1)
                        if self.pc_alive =>
                    {
                        if pressed {
                            if self.last_time >= self.firebolt_cd_until {
                                self.pc_cast_queued = true;
                                self.pc_cast_kind = Some(PcCast::FireBolt);
                                self.pc_cast_time = 0.0; // instant
                                log::debug!("PC cast queued: Fire Bolt");
                            } else {
                                log::debug!(
                                    "Fire Bolt on cooldown: {:.0} ms remaining",
                                    ((self.firebolt_cd_until - self.last_time) * 1000.0).max(0.0)
                                );
                            }
                        }
                    }
                    PhysicalKey::Code(KeyCode::Digit2) | PhysicalKey::Code(KeyCode::Numpad2)
                        if self.pc_alive =>
                    {
                        if pressed {
                            self.pc_cast_queued = true;
                            self.pc_cast_kind = Some(PcCast::MagicMissile);
                            self.pc_cast_time = 1.0; // Magic Missile uses Action pacing
                            log::debug!("PC cast queued: Magic Missile");
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
                    PhysicalKey::Code(KeyCode::F1) => {
                        if pressed {
                            self.hud_model.toggle_perf();
                            log::info!(
                                "Perf overlay {}",
                                if self.hud_model.perf_enabled() {
                                    "on"
                                } else {
                                    "off"
                                }
                            );
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyH) => {
                        if pressed {
                            self.hud_model.toggle_hud();
                            log::info!(
                                "HUD {}",
                                if self.hud_model.hud_enabled() {
                                    "shown"
                                } else {
                                    "hidden"
                                }
                            );
                        }
                    }
                    PhysicalKey::Code(KeyCode::F5) => {
                        if pressed {
                            // Start a 5-second smooth orbit capture
                            self.screenshot_start = Some(self.last_time);
                            log::info!("Screenshot mode: 5s orbit starting");
                        }
                    }
                    // Allow keyboard respawn as fallback when dead
                    PhysicalKey::Code(KeyCode::KeyR) | PhysicalKey::Code(KeyCode::Enter) => {
                        if pressed && !self.pc_alive {
                            log::info!("Respawn via keyboard");
                            self.respawn();
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
                    // Allow a closer near-zoom so the camera can sit just
                    // behind and slightly above the wizard's head.
                    self.cam_distance = (self.cam_distance - step).clamp(1.6, 25.0);
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
                // Use previous cursor for deltas, then update to current.
                if self.rmb_down
                    && let Some((lx, ly)) = self.last_cursor_pos
                {
                    let dx = position.x - lx;
                    let dy = position.y - ly;
                    let sens = 0.005;
                    // Fully sync player facing with mouse drag; keep camera behind the player
                    let yaw_delta = dx as f32 * sens;
                    self.player.yaw = wrap_angle(self.player.yaw - yaw_delta);
                    self.cam_orbit_yaw = 0.0;
                    // Invert pitch control (mouse up pitches camera down, and vice versa)
                    self.cam_orbit_pitch =
                        (self.cam_orbit_pitch + dy as f32 * sens).clamp(-0.6, 1.2);
                }
                // Track last cursor position
                self.last_cursor_pos = Some((position.x, position.y));
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
        let before = self.player.pos;
        self.player.update(&self.input, dt, cam_fwd);
        if let Some(idx) = &self.static_index {
            // Resolve against static colliders (capsule approx for wizard)
            let cap = collision_static::Capsule {
                p0: glam::vec3(
                    self.player.pos.x,
                    self.player.pos.y + 0.4,
                    self.player.pos.z,
                ),
                p1: glam::vec3(
                    self.player.pos.x,
                    self.player.pos.y + 1.8,
                    self.player.pos.z,
                ),
                radius: 0.4,
            };
            let a = collision_static::Aabb {
                min: glam::vec3(
                    cap.p0.x.min(cap.p1.x) - cap.radius,
                    cap.p0.y.min(cap.p1.y) - cap.radius,
                    cap.p0.z.min(cap.p1.z) - cap.radius,
                ),
                max: glam::vec3(
                    cap.p0.x.max(cap.p1.x) + cap.radius,
                    cap.p0.y.max(cap.p1.y) + cap.radius,
                    cap.p0.z.max(cap.p1.z) + cap.radius,
                ),
            };
            let _ = a; // reserved for future broadphase tuning
            let resolved =
                collision_static::resolve_slide(before, self.player.pos, &cap, idx, 0.25, 4);
            self.player.pos = resolved;
        }
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
                self.pc_cast_fired = false;
            }
        }
        if let Some(start) = self.pc_anim_start {
            if self.wizard_anim_index[self.pc_index] == 0 {
                let clip = self.select_clip(0);
                let elapsed = t - start;
                // Fire bolt exactly at cast end if not yet fired
                if !self.pc_cast_fired && elapsed >= self.pc_cast_time {
                    let phase = self.pc_cast_time;
                    if let Some(origin_local) = self.right_hand_world(clip, phase) {
                        let inst = self
                            .wizard_models
                            .get(self.pc_index)
                            .copied()
                            .unwrap_or(glam::Mat4::IDENTITY);
                        let origin_w = inst
                            * glam::Vec4::new(origin_local.x, origin_local.y, origin_local.z, 1.0);
                        let dir_w = (inst * glam::Vec4::new(0.0, 0.0, 1.0, 0.0))
                            .truncate()
                            .normalize_or_zero();
                        let right_w = (inst * glam::Vec4::new(1.0, 0.0, 0.0, 0.0))
                            .truncate()
                            .normalize_or_zero();
                        let lateral = 0.20;
                        let spawn = origin_w.truncate() + dir_w * 0.3 - right_w * lateral;
                        match self.pc_cast_kind.unwrap_or(PcCast::FireBolt) {
                            PcCast::FireBolt => {
                                log::debug!("PC Fire Bolt fired at t={:.2}", t);
                                self.spawn_firebolt(spawn, dir_w, t, Some(self.pc_index), false);
                                // Begin 1.0s cooldown
                                self.firebolt_cd_dur = 1.0;
                                self.firebolt_cd_until = self.last_time + self.firebolt_cd_dur;
                            }
                            PcCast::MagicMissile => {
                                log::debug!("PC Magic Missile fired at t={:.2}", t);
                                self.spawn_magic_missile(spawn, dir_w, t);
                            }
                        }
                        self.pc_cast_fired = true;
                    }
                    // Immediately end cast animation and start cooldown window
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
        // 1) Spawn firebolts for PortalOpen phase crossing (NPC wizards only).
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
                if allowed && crossed && i != self.pc_index {
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
                        self.spawn_firebolt(spawn, dir_w, t, Some(i), true);
                    }
                }
                self.wizard_last_phase[i] = phase;
            }
        }

        // 2) Integrate projectiles and keep them slightly above ground
        // so they don't clip into small terrain undulations.
        let ground_clearance = 0.15f32; // meters above terrain
        for p in &mut self.projectiles {
            p.pos += p.vel * dt;
            // Clamp to be a bit above the terrain height at current XZ.
            p.pos =
                crate::gfx::util::clamp_above_terrain(&self.terrain_cpu, p.pos, ground_clearance);
        }
        // 2.5) Server-side collision vs NPCs
        if !self.projectiles.is_empty() && !self.server.npcs.is_empty() {
            let damage = 10; // TODO: integrate with spell spec dice
            let hits = self
                .server
                .collide_and_damage(&mut self.projectiles, dt, damage);
            for h in &hits {
                log::debug!(
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
                        vel: glam::vec3(a.cos() * r, 2.0 + rand_unit() * 1.2, a.sin() * r),
                        age: 0.0,
                        life: 0.18,
                        size: 0.02,
                        color: [1.7, 0.85, 0.35],
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
                    self.damage
                        .spawn(h.pos + glam::vec3(0.0, 0.9, 0.0), h.damage);
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
            // Only time-out; allow clearance clamp to keep bolts skimming above ground.
            let kill = t >= self.projectiles[i].t_die;
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
                    color: [1.8, 1.2, 0.4],
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
                        color: [1.6, 0.9, 0.3],
                    });
                }
                self.projectiles.swap_remove(i);
            } else {
                i += 1;
            }
        }
        // Merge any ground-hit bursts into live particles
        if !burst.is_empty() {
            self.particles.append(&mut burst);
        }

        // 2.6) Collide with wizards/PC (friendly fire on)
        if !self.projectiles.is_empty() {
            self.collide_with_wizards(dt, 10);
        }

        // 3) Simulate impact particles (age, simple gravity, fade)
        let cam = self.cam_follow.current_pos;
        let max_d2 = 400.0 * 400.0; // cull far particles
        let mut j = 0usize;
        while j < self.particles.len() {
            let p = &mut self.particles[j];
            p.age += dt;
            // mild gravity and air drag
            p.vel.y -= 9.8 * dt * 0.5;
            p.vel *= 0.98f32.powf(dt.max(0.0) * 60.0);
            p.pos += p.vel * dt;
            if p.age >= p.life {
                self.particles.swap_remove(j);
                continue;
            }
            // Cull by distance
            if (p.pos - cam).length_squared() > max_d2 {
                self.particles.swap_remove(j);
                continue;
            }
            j += 1;
        }

        // 4) Upload FX instances (billboard particles) — bolts (with tiny trails) + impact sprites
        let mut inst: Vec<ParticleInstance> =
            Vec::with_capacity(self.projectiles.len() * 3 + self.particles.len());
        // Brighter firebolt sprites: larger head and boosted emissive color.
        // Keep additive blending; values >1.0 feed bloom nicely.
        for pr in &self.projectiles {
            // Fade as the projectile nears its lifetime end (range cap or base life)
            let mut head_fade = 1.0f32;
            let fade_window = 0.15f32;
            if pr.t_die > 0.0 {
                let remain = (pr.t_die - t).max(0.0);
                head_fade = (remain / fade_window).clamp(0.0, 1.0);
            }
            // head
            inst.push(ParticleInstance {
                pos: [pr.pos.x, pr.pos.y, pr.pos.z],
                size: 0.18,
                color: [2.6 * head_fade, 0.7 * head_fade, 0.18 * head_fade],
                _pad: 0.0,
            });
            // short trail segments behind
            let dir = pr.vel.normalize_or_zero();
            for k in 1..=2 {
                let t = k as f32 * 0.02;
                let p = pr.pos - dir * (t * pr.vel.length());
                let fade = (1.0 - (k as f32) * 0.35) * head_fade;
                inst.push(ParticleInstance {
                    pos: [p.x, p.y, p.z],
                    size: 0.13,
                    color: [2.0 * fade, 0.55 * fade, 0.16 * fade],
                    _pad: 0.0,
                });
            }
        }
        // Impacts (fade by age)
        for p in &self.particles {
            let f = 1.0 - (p.age / p.life).clamp(0.0, 1.0);
            let size = p.size * (1.0 + 0.5 * (1.0 - f));
            inst.push(ParticleInstance {
                pos: [p.pos.x, p.pos.y, p.pos.z],
                size,
                color: [
                    p.color[0] * f * 1.5,
                    p.color[1] * f * 1.5,
                    p.color[2] * f * 1.5,
                ],
                _pad: 0.0,
            });
        }
        // Cap to buffer capacity
        if (inst.len() as u32) > self._fx_capacity {
            inst.truncate(self._fx_capacity as usize);
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
                            color: [1.8, 0.8, 0.3],
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
        snap_to_ground: bool,
    ) {
        let mut speed = 40.0;
        // Base lifetime for visuals; will be clamped by spec range below.
        let base_life = 1.2 * 1.5;
        // Compute range clamp from spell spec (default 120 ft)
        let mut max_range_m = 120.0 * 0.3048;
        if let Some(spec) = &self.fire_bolt
            && let Some(p) = &spec.projectile
        {
            speed = p.speed_mps;
            max_range_m = (spec.range_ft as f32) * 0.3048;
        }
        let flight_time = if speed > 0.01 { max_range_m / speed } else { base_life };
        let life = base_life.min(flight_time);
        // Ensure initial spawn is terrain-aware.
        // - PC: keep hand height but raise if below clearance (clamp above).
        // - NPC: snap onto terrain + clearance so bolts hug the ground like the PC's do.
        let origin = if snap_to_ground {
            let (h, _n) = crate::gfx::terrain::height_at(&self.terrain_cpu, origin.x, origin.z);
            glam::vec3(origin.x, h + 0.15, origin.z)
        } else {
            crate::gfx::util::clamp_above_terrain(&self.terrain_cpu, origin, 0.15)
        };
        self.projectiles.push(Projectile {
            pos: origin,
            vel: dir * speed,
            t_die: t + life,
            owner_wizard: owner,
        });
    }

    fn spawn_magic_missile(&mut self, origin: glam::Vec3, dir: glam::Vec3, t: f32) {
        // For v1, just fire three forward darts similar to Fire Bolt visuals.
        // Slight lateral offsets for readability.
        let right = glam::vec3(dir.z, 0.0, -dir.x).normalize_or_zero();
        let offsets = [-0.12f32, 0.0, 0.12f32];
        for off in offsets {
            let o = origin + right * off;
            self.spawn_firebolt(o, dir, t, Some(self.pc_index), false);
        }
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
