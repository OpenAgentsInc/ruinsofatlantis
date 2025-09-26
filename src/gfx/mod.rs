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
mod ui;
mod util;

use crate::assets::{AnimClip, SkinnedMeshCPU, load_gltf_mesh, load_gltf_skinned};
use crate::core::data::{loader as data_loader, spell::SpellSpec};
// (scene building now encapsulated; ECS types unused here)
use anyhow::Context;
use types::{Globals, Model, ParticleInstance, VertexSkinned, InstanceSkin};
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
    globals_bg: wgpu::BindGroup,
    plane_model_bg: wgpu::BindGroup,
    shard_model_bg: wgpu::BindGroup,

    // --- Scene Buffers ---
    globals_buf: wgpu::Buffer,
    _plane_model_buf: wgpu::Buffer,
    shard_model_buf: wgpu::Buffer,

    // Geometry (ground plane)
    plane_vb: wgpu::Buffer,
    plane_ib: wgpu::Buffer,
    plane_index_count: u32,

    // GLTF geometry (wizard + ruins)
    wizard_vb: wgpu::Buffer,
    wizard_ib: wgpu::Buffer,
    wizard_index_count: u32,
    ruins_vb: wgpu::Buffer,
    ruins_ib: wgpu::Buffer,
    ruins_index_count: u32,

    // NPC cubes
    npc_vb: wgpu::Buffer,
    npc_ib: wgpu::Buffer,
    npc_index_count: u32,
    npc_instances: wgpu::Buffer,
    npc_count: u32,
    npc_instances_cpu: Vec<types::Instance>,
    npc_models: Vec<glam::Mat4>,

    // Instancing buffers
    wizard_instances: wgpu::Buffer,
    wizard_count: u32,
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

    // Wizard pipelines
    wizard_pipeline: wgpu::RenderPipeline,

    wizard_mat_bg: wgpu::BindGroup,
    _wizard_mat_buf: wgpu::Buffer,
    _wizard_tex_view: wgpu::TextureView,
    _wizard_sampler: wgpu::Sampler,

    // Flags
    wire_enabled: bool,

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
    root_node: Option<usize>,

    // Projectile + particle pools
    projectiles: Vec<Projectile>,
    particles: Vec<Particle>,

    // Data-driven spec
    fire_bolt: Option<SpellSpec>,

    // Camera focus (we orbit around a close wizard)

    // UI overlay
    nameplates: ui::Nameplates,

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
}

impl Renderer {
    /// Create a renderer bound to a window surface.
    pub async fn new(window: &Window) -> anyhow::Result<Self> {
        // --- Surface ---
        let instance = wgpu::Instance::default();
        // Create a surface without borrowing `window` for its lifetime.
        let raw_display = window.display_handle()?.as_raw();
        let raw_window = window.window_handle()?.as_raw();
        let surface = unsafe {
            instance.create_surface_unsafe(SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: raw_display,
                raw_window_handle: raw_window,
            })
        }
        .context("create wgpu surface (unsafe)")?;

        // --- Adapter / Device ---
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
            })
            .await
            .context("request adapter")?;

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

        // UI: nameplates
        let nameplates = ui::Nameplates::new(&device, config.format)?;

        // --- Buffers & bind groups ---
        // Globals
        let globals_init = Globals {
            view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
            cam_right_time: [1.0, 0.0, 0.0, 0.0],
            cam_up_pad: [0.0, 1.0, 0.0, 0.0],
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

        // Per-draw Model buffers (plane and shard base)
        // Nudge the plane slightly downward to avoid z-fighting/overlap with wizard feet.
        let plane_model_init = Model {
            model: glam::Mat4::from_translation(glam::vec3(0.0, -0.05, 0.0)).to_cols_array_2d(),
            color: [0.05, 0.80, 0.30],
            emissive: 0.0,
            _pad: [0.0; 4],
        };
        let plane_model_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("plane-model"),
            contents: bytemuck::bytes_of(&plane_model_init),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let plane_model_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("plane-model-bg"),
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

        // Ground plane (choose a generous extent for the plaza)
        let plane_extent = 150.0;
        let (plane_vb, plane_ib, plane_index_count) = mesh::create_plane(&device, plane_extent);

        // --- Load GLTF assets into CPU meshes, then upload to GPU buffers ---
        let skinned_cpu = load_gltf_skinned(&asset_path("assets/models/wizard.gltf"))
            .context("load skinned wizard.gltf")?;
        let ruins_cpu_res = load_gltf_mesh(&asset_path("assets/models/ruins.gltf"));

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
        let scene_build = scene::build_demo_scene(&device, &skinned_cpu, plane_extent);
        // Precompute PC initial position from the soon-to-be-moved vector
        let pc_initial_pos = {
            let m = scene_build.wizard_models[scene_build.pc_index];
            let c = m.to_cols_array();
            glam::vec3(c[12], c[13], c[14])
        };
        // Upload text atlas once now that we have a queue
        nameplates.upload_atlas(&queue);
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

        let material_res =
            material::create_wizard_material(&device, &queue, &material_bgl, &skinned_cpu);
        let wizard_mat_bg = material_res.bind_group;
        let _wizard_mat_buf = material_res.uniform_buf;
        let _wizard_tex_view = material_res.texture_view;
        let _wizard_sampler = material_res.sampler;

        // NPCs: simple cubes as targets on multiple rings
        let (npc_vb, npc_ib, npc_index_count) = mesh::create_cube(&device);
        let mut server = crate::server::ServerState::new();
        // Configure ring distances and counts (keep existing ones, add more)
        let near_count = 10usize; // existing close ring
        let near_radius = 15.0f32;
        let mid1_count = 16usize;
        let mid1_radius = 30.0f32;
        let mid2_count = 20usize;
        let mid2_radius = 45.0f32;
        let mid3_count = 24usize;
        let mid3_radius = 60.0f32;
        let far_count = 12usize; // existing far ring
        let far_radius = plane_extent * 0.7;
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

        log::info!(
            "spawned {} NPCs across rings: near={}, mid1={}, mid2={}, mid3={}, far={}",
            server.npcs.len(),
            near_count,
            mid1_count,
            mid2_count,
            mid3_count,
            far_count
        );
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
            globals_bg,
            plane_model_bg,
            shard_model_bg,

            globals_buf,
            _plane_model_buf: plane_model_buf,
            shard_model_buf,

            plane_vb,
            plane_ib,
            plane_index_count,
            wizard_vb,
            wizard_ib,
            wizard_index_count,
            ruins_vb,
            ruins_ib,
            ruins_index_count,
            wizard_instances: scene_build.wizard_instances,
            wizard_count: scene_build.wizard_count,
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
            wizard_models: scene_build.wizard_models,
            wizard_instances_cpu: scene_build.wizard_instances_cpu,
            wizard_pipeline,
            // debug pipelines removed
            wizard_mat_bg,
            _wizard_mat_buf,
            _wizard_tex_view,
            _wizard_sampler,
            wire_enabled: false,

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
            npc_count: npc_instances_cpu.len() as u32,
            npc_instances_cpu,
            npc_models,
            server,
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
        // Compute local orbit offsets (relative to PC orientation)
        let (off_local, look_local) = camera_sys::compute_local_orbit_offsets(
            self.cam_distance,
            self.cam_orbit_yaw,
            self.cam_orbit_pitch,
            self.cam_lift,
            self.cam_look_height,
        );
        let (_cam, globals) = camera_sys::third_person_follow(
            &mut self.cam_follow,
            self.player.pos,
            glam::Quat::from_rotation_y(self.player.yaw),
            off_local,
            look_local,
            aspect,
            dt,
        );
        self.queue
            .write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));

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
        // FX update (projectiles/particles)
        self.update_fx(t, dt);

        // Begin commands
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encoder"),
            });
        {
            use wgpu::*;
            let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("main-pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.02,
                            g: 0.08,
                            b: 0.16,
                            a: 1.0,
                        }),
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

            // Ground plane
            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(0, &self.globals_bg, &[]);
            rpass.set_bind_group(1, &self.plane_model_bg, &[]);
            rpass.set_vertex_buffer(0, self.plane_vb.slice(..));
            rpass.set_index_buffer(self.plane_ib.slice(..), IndexFormat::Uint16);
            rpass.draw_indexed(0..self.plane_index_count, 0, 0..1);

            // Wizards
            self.draw_wizards(&mut rpass);

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
        // Overlay: nameplates after 3D content
        let view_proj = glam::Mat4::from_cols_array_2d(&globals.view_proj);
        self.nameplates.queue_labels(
            &self.device,
            &self.queue,
            self.config.width,
            self.config.height,
            view_proj,
            &self.wizard_models,
        );
        self.nameplates.draw(&mut encoder, &view);

        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

impl Renderer {
    /// Handle platform window events that affect input (keyboard focus/keys).
    pub fn handle_window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state.is_pressed();
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::KeyW) => self.input.forward = pressed,
                    PhysicalKey::Code(KeyCode::KeyS) => self.input.backward = pressed,
                    PhysicalKey::Code(KeyCode::KeyA) => self.input.left = pressed,
                    PhysicalKey::Code(KeyCode::KeyD) => self.input.right = pressed,
                    PhysicalKey::Code(KeyCode::ShiftLeft) | PhysicalKey::Code(KeyCode::ShiftRight) => {
                        self.input.run = pressed
                    }
                    PhysicalKey::Code(KeyCode::Digit1) | PhysicalKey::Code(KeyCode::Numpad1) => {
                        if pressed { self.pc_cast_queued = true; log::info!("PC cast queued: Fire Bolt"); }
                    }
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let mut step = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => *y,
                    winit::event::MouseScrollDelta::PixelDelta(p) => (p.y as f32) * 0.05,
                };
                if step.abs() < 1e-3 { step = 0.0; }
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
                        self.cam_orbit_pitch = (self.cam_orbit_pitch + dy as f32 * sens).clamp(-0.6, 1.2);
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
        if self.wizard_count == 0 { return; }
        let cam_fwd = self.cam_follow.current_look - self.cam_follow.current_pos;
        self.player.update(&self.input, dt, cam_fwd);
        self.apply_pc_transform();
    }

    fn apply_pc_transform(&mut self) {
        // Update CPU model matrix and upload only the PC instance
        let rot = glam::Quat::from_rotation_y(self.player.yaw);
        let m = glam::Mat4::from_scale_rotation_translation(glam::Vec3::splat(1.0), rot, self.player.pos);
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
            let palette = if i == self.pc_index {
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
        // 1) Spawn firebolts for all wizards playing PortalOpen when their phase crosses the trigger.
        if self.wizard_count > 0 {
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
                if crossed {
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
                        let dir_local = self
                            .root_flat_forward(clip, clip_time)
                            .unwrap_or(glam::Vec3::new(0.0, 0.0, 1.0));
                        let dir_w = (inst
                            * glam::Vec4::new(dir_local.x, dir_local.y, dir_local.z, 0.0))
                        .truncate()
                        .normalize_or_zero();
                        if i == self.pc_index {
                            log::info!("PC Fire Bolt fired at t={:.2}", t);
                        }
                        self.spawn_firebolt(origin_w.truncate() + dir_w * 0.3, dir_w, t);
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
            let hits = self.server.collide_and_damage(&mut self.projectiles, dt, damage);
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
                // Update NPC visuals: darken color on hit; remove if dead
                if let Some(idx) = self.server.npcs.iter().position(|n| n.id == h.npc) {
                    if h.fatal {
                        // swap remove from instance list and server vectors stay authoritative
                        if (idx as u32) < self.npc_count {
                            self.npc_instances_cpu.swap_remove(idx);
                            self.npc_models.swap_remove(idx);
                            self.npc_count -= 1;
                            let bytes: &[u8] = bytemuck::cast_slice(&self.npc_instances_cpu);
                            self.queue.write_buffer(&self.npc_instances, 0, bytes);
                        }
                    } else {
                        let mut inst = self.npc_instances_cpu[idx];
                        inst.color = [0.6, 0.15, 0.15];
                        self.npc_instances_cpu[idx] = inst;
                        let offset = (idx * std::mem::size_of::<types::Instance>()) as u64;
                        self.queue.write_buffer(&self.npc_instances, offset, bytemuck::bytes_of(&inst));
                    }
                }
            }
            if hits.is_empty() {
                log::debug!("no hits this frame: projectiles={} npcs={}", self.projectiles.len(), self.server.npcs.len());
            }
        }
        // Ground hit or timeout
        let mut burst: Vec<Particle> = Vec::new();
        let mut i = 0;
        while i < self.projectiles.len() {
            let kill = t >= self.projectiles[i].t_die || self.projectiles[i].pos.y <= 0.05;
            if kill {
                let hit = self.projectiles[i].pos;
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
    }

    fn spawn_firebolt(&mut self, origin: glam::Vec3, dir: glam::Vec3, t: f32) {
        let mut speed = 40.0;
        let life = 1.2;
        if let Some(spec) = &self.fire_bolt
            && let Some(p) = &spec.projectile
        {
            speed = p.speed_mps;
        }
        self.projectiles.push(Projectile {
            pos: origin,
            vel: dir * speed,
            t_die: t + life,
        });
    }

    fn right_hand_world(&self, clip: &AnimClip, phase: f32) -> Option<glam::Vec3> {
        let h = self.hand_right_node?;
        let m = anim::global_of_node(&self.skinned_cpu, clip, phase, h)?;
        let c = m.to_cols_array();
        Some(glam::vec3(c[12], c[13], c[14]))
    }
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
    while x > std::f32::consts::PI { x -= std::f32::consts::TAU; }
    while x < -std::f32::consts::PI { x += std::f32::consts::TAU; }
    x
}

fn rand_unit() -> f32 {
    use rand::Rng as _;
    let mut r = rand::rng();
    r.random::<f32>() * 2.0 - 1.0
}
