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
mod util;

use anyhow::Context;
use camera::Camera;
use types::{Globals, Instance, InstanceSkin, Model, VertexSkinned};
use util::scale_to_max;
use crate::assets::{load_gltf_mesh, load_gltf_skinned, AnimClip, SkinnedMeshCPU};
use crate::core::data::{loader as data_loader, spell::SpellSpec};
use crate::ecs::{World, Transform, RenderKind};

use std::time::Instant;
 
use wgpu::{rwh::HasDisplayHandle, rwh::HasWindowHandle, util::DeviceExt, SurfaceError, SurfaceTargetUnsafe};
use winit::dpi::PhysicalSize;
use winit::window::Window;
use std::fs;

fn asset_path(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

// --- Minimal FX structs (CPU side) ---
#[derive(Clone, Copy, Debug)]
struct Projectile { pos: glam::Vec3, vel: glam::Vec3, radius: f32, t_die: f32 }

#[derive(Clone, Copy, Debug)]
struct Particle { pos: glam::Vec3, vel: glam::Vec3, age: f32, life: f32, size: f32, color: [f32;3] }

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
    // Simple cube for FX/particles
    fx_cube_vb: wgpu::Buffer,
    fx_cube_ib: wgpu::Buffer,
    fx_cube_index_count: u32,
    ruins_vb: wgpu::Buffer,
    ruins_ib: wgpu::Buffer,
    ruins_index_count: u32,

    // Instancing buffers
    wizard_instances: wgpu::Buffer,
    wizard_count: u32,
    ruins_instances: wgpu::Buffer,
    ruins_count: u32,
    // FX instances (projectiles + particles)
    fx_instances: wgpu::Buffer,
    fx_count: u32,
    fx_capacity: u32,

    // Wizard skinning palettes
    palettes_buf: wgpu::Buffer,
    palettes_bg: wgpu::BindGroup,
    joints_per_wizard: u32,

    // Wizard pipelines
    wizard_pipeline: wgpu::RenderPipeline,
    // (wizard simple/wire debug pipelines removed)
    wizard_mat_bg: wgpu::BindGroup,
    _wizard_mat_buf: wgpu::Buffer,
    _wizard_tex_view: wgpu::TextureView,
    _wizard_sampler: wgpu::Sampler,
    // FX model (emissive)
    fx_model_bg: wgpu::BindGroup,
    _fx_model_buf: wgpu::Buffer,

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
    // Animation-driven VFX trigger data
    portalopen_strikes: Vec<f32>,
    last_center_phase: f32,
    hand_right_node: Option<usize>,
    root_node: Option<usize>,

    // Projectiles/particles (CPU side)
    projectiles: Vec<Projectile>,
    particles: Vec<Particle>,

    // Data-driven spell spec (Fire Bolt)
    fire_bolt: Option<SpellSpec>,

    // Camera focus (we orbit around a close wizard)
    cam_target: glam::Vec3,
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
        if adapter.features().contains(wgpu::Features::POLYGON_MODE_LINE) {
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
        let max_dim = device.limits().max_texture_dimension_2d.min(2048).max(1);
        let (w, h) = scale_to_max((size.width, size.height), max_dim);
        if (w, h) != (size.width, size.height) {
            log::warn!(
                "Clamping surface from {}x{} to {}x{} (max_dim={})",
                size.width, size.height, w, h, max_dim
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
        let (wizard_pipeline, _wizard_wire_pipeline_unused) =
            pipeline::create_wizard_pipelines(&device, &shader, &globals_bgl, &model_bgl, &palettes_bgl, &material_bgl, config.format);

        // --- Buffers & bind groups ---
        // Globals
        let globals_init = Globals { view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(), time_pad: [0.0; 4] };
        let globals_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globals"),
            contents: bytemuck::bytes_of(&globals_init),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals-bg"),
            layout: &globals_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() }],
        });

        // Per-draw Model buffers (plane and shard base)
        // Nudge the plane slightly downward to avoid z-fighting/overlap with wizard feet.
        let plane_model_init = Model { model: glam::Mat4::from_translation(glam::vec3(0.0, -0.05, 0.0)).to_cols_array_2d(), color: [0.05, 0.80, 0.30], emissive: 0.0, _pad: [0.0; 4] };
        let plane_model_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("plane-model"),
            contents: bytemuck::bytes_of(&plane_model_init),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let plane_model_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("plane-model-bg"),
            layout: &model_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: plane_model_buf.as_entire_binding() }],
        });

        let shard_model_init = Model { model: glam::Mat4::IDENTITY.to_cols_array_2d(), color: [0.85, 0.15, 0.15], emissive: 0.15, _pad: [0.0; 4] };
        let shard_model_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("shard-model"),
            contents: bytemuck::bytes_of(&shard_model_init),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let shard_model_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shard-model-bg"),
            layout: &model_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: shard_model_buf.as_entire_binding() }],
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
        let viewer_uv: Option<Vec<[f32;2]>> = (|| {
            let (doc, buffers, _images) = gltf::import(asset_path("assets/models/wizard.gltf")).ok()?;
            let mesh = doc.meshes().next()?;
            let prim = mesh.primitives().next()?;
            let reader = prim.reader(|b| buffers.get(b.index()).map(|bb| bb.0.as_slice()));
            let uv_set = prim.material().pbr_metallic_roughness().base_color_texture().map(|ti| ti.tex_coord()).unwrap_or(0);
            let uv = reader.read_tex_coords(uv_set)?.into_f32().collect::<Vec<[f32;2]>>();
            Some(uv)
        })();

        let wiz_vertices: Vec<VertexSkinned> = skinned_cpu
            .vertices
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let uv = if let Some(ref uvs) = viewer_uv { uvs.get(i).copied().unwrap_or(v.uv) } else { v.uv };
                VertexSkinned { pos: v.pos, nrm: v.nrm, joints: v.joints, weights: v.weights, uv }
            })
            .collect();

        // (debug diagnostic logs removed)
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

        // FX cube geometry (used for projectiles/particles)
        let (fx_cube_vb, fx_cube_ib, fx_cube_index_count) = mesh::create_cube(&device);

        // (viewer-parity simple mesh removed)

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

        // --- Build a tiny ECS world and spawn entities ---
        let mut world = World::new();
        use rand::{SeedableRng, Rng};
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);

        let place_range = plane_extent * 0.4;

        // Cluster wizards around a central one so the camera can see all of them.
        let wizard_count = 10usize;
        let center = glam::vec3(0.0, 0.0, 0.0);
        // Spawn the central wizard first (becomes camera target)
        world.spawn(Transform { translation: center, rotation: glam::Quat::IDENTITY, scale: glam::Vec3::splat(1.0) }, RenderKind::Wizard);
        // Place remaining wizards on a small ring facing the center
        let ring_radius = 3.5f32;
        for i in 1..wizard_count {
            let theta = (i as f32 - 1.0) / (wizard_count as f32 - 1.0) * std::f32::consts::TAU;
            let translation = glam::vec3(ring_radius * theta.cos(), 0.0, ring_radius * theta.sin());
            // Face the center with yaw that maps forward (-Z) toward (center - translation)
            let dx = center.x - translation.x;
            let dz = center.z - translation.z;
            // Model forward is +Z; yaw that aligns +Z to (dx,dz)
            let yaw = dx.atan2(dz);
            let rotation = glam::Quat::from_rotation_y(yaw);
            world.spawn(Transform { translation, rotation, scale: glam::Vec3::splat(1.0) }, RenderKind::Wizard);
        }
        // Place a set of ruins around the wizard circle
        // 1) Keep a few large backdrop ruins
        let ruins_positions = [
            glam::vec3(-place_range, 0.0, -place_range * 0.6),
            glam::vec3(place_range * 0.9, 0.0, 0.0),
            glam::vec3(0.0, 0.0, place_range * 0.8),
        ];
        for pos in ruins_positions {
            let rotation = glam::Quat::from_rotation_y(rng.random::<f32>() * std::f32::consts::TAU);
            world.spawn(Transform { translation: pos, rotation, scale: glam::Vec3::splat(1.0) }, RenderKind::Ruins);
        }
        // 2) Near-circle ruins disabled to keep space around the wizards clear

        // --- Create instance buffers per kind from ECS world ---
        let mut wiz_instances: Vec<InstanceSkin> = Vec::new();
        let mut ruin_instances: Vec<Instance> = Vec::new();
        let mut cam_target = glam::Vec3::ZERO;
        let mut has_cam_target = false;
        for (i, kind) in world.kinds.iter().enumerate() {
            let t = world.transforms[i];
            let m = t.matrix().to_cols_array_2d();
            match kind {
                RenderKind::Wizard => {
                    if !has_cam_target { cam_target = t.translation + glam::vec3(0.0, 1.2, 0.0); has_cam_target = true; }
                    wiz_instances.push(InstanceSkin { model: m, color: [0.20, 0.45, 0.95], selected: 0.0, palette_base: 0, _pad_inst: [0;3] })
                }
                RenderKind::Ruins => ruin_instances.push(Instance { model: m, color: [0.65, 0.66, 0.68], selected: 0.0 }),
            }
        }
        // Assign palette bases and random animations
        let joints_per_wizard = skinned_cpu.joints_nodes.len() as u32;
        // Reuse existing RNG; already imported above
        let mut rng2 = rand_chacha::ChaCha8Rng::seed_from_u64(4242);
        let mut wizard_anim_index: Vec<usize> = Vec::with_capacity(wiz_instances.len());
        let mut wizard_time_offset: Vec<f32> = Vec::with_capacity(wiz_instances.len());
        for (i, inst) in wiz_instances.iter_mut().enumerate() {
            inst.palette_base = (i as u32) * joints_per_wizard;
            // Center wizard uses PortalOpen; ring uses Waiting
            if i == 0 { wizard_anim_index.push(0); } else { wizard_anim_index.push(2); }
            wizard_time_offset.push(rng2.random::<f32>() * 1.7);
        }

        let wizard_instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("wizard-instances"),
            contents: bytemuck::cast_slice(&wiz_instances),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let ruins_instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ruins-instances"),
            contents: bytemuck::cast_slice(&ruin_instances),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        log::info!("spawned {} wizards and {} ruins", wiz_instances.len(), ruin_instances.len());
        // FX buffers (dynamic)
        let fx_capacity = 2048u32; // particles + projectiles
        let fx_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fx-instances"),
            size: (fx_capacity as usize * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // FX model (slightly emissive)
        let fx_model = Model { model: glam::Mat4::IDENTITY.to_cols_array_2d(), color: [1.0, 0.5, 0.1], emissive: 0.6, _pad: [0.0; 4] };
        let fx_model_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("fx-model"),
            contents: bytemuck::bytes_of(&fx_model),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let fx_model_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fx-model-bg"),
            layout: &model_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: fx_model_buf.as_entire_binding() }],
        });

        // Camera target: first wizard encountered above (close-up orbit)

        // Allocate storage for skinning palettes: one palette per wizard
        let total_mats = (wiz_instances.len() as u32 * joints_per_wizard) as usize;
        let palettes_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("palettes"),
            size: (total_mats * 64) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let palettes_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("palettes-bg"),
            layout: &palettes_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &palettes_buf, offset: 0, size: None }) }],
        });

        // Wizard material (albedo from glTF)
        // Material transform defaults (can be overridden by KHR_texture_transform)
        // Note: std140 layout for uniforms rounds vec2 members to 16 bytes each.
        // Expand the struct to 48 bytes to satisfy backend expectations.
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct MaterialXform {
            offset: [f32; 2], _pad0: [f32; 2], // 16 bytes
            scale:  [f32; 2], _pad1: [f32; 2], // 16 bytes
            rot: f32,       _pad2: [f32; 3],  // 16 bytes
        }
        let mut mat_xf = MaterialXform { offset: [0.0, 0.0], _pad0: [0.0;2], scale: [1.0, 1.0], _pad1: [0.0;2], rot: 0.0, _pad2: [0.0;3] };
        // Try to read KHR_texture_transform from the wizard glTF for the first primitive's material
        if let Ok(txt) = std::fs::read_to_string(asset_path("assets/models/wizard.gltf")) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&txt) {
                // Resolve first primitive material index
                let mat_index = json.get("meshes")
                    .and_then(|m| m.get(0))
                    .and_then(|m0| m0.get("primitives"))
                    .and_then(|prims| prims.get(0))
                    .and_then(|p0| p0.get("material"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                let base_tex = json.get("materials")
                    .and_then(|arr| arr.get(mat_index))
                    .and_then(|m| m.get("pbrMetallicRoughness"))
                    .and_then(|p| p.get("baseColorTexture"));
                if let Some(bct) = base_tex {
                    // Optional: texCoord selection was already handled in loader; we only read transform here
                    if let Some(ext) = bct.get("extensions").and_then(|e| e.get("KHR_texture_transform")) {
                        if let Some(off) = ext.get("offset").and_then(|v| v.as_array()) {
                            if off.len() == 2 { mat_xf.offset = [off[0].as_f64().unwrap_or(0.0) as f32, off[1].as_f64().unwrap_or(0.0) as f32]; }
                        }
                        if let Some(s) = ext.get("scale").and_then(|v| v.as_array()) {
                            if s.len() == 2 { mat_xf.scale = [s[0].as_f64().unwrap_or(1.0) as f32, s[1].as_f64().unwrap_or(1.0) as f32]; }
                        }
                        if let Some(r) = ext.get("rotation").and_then(|v| v.as_f64()) { mat_xf.rot = r as f32; }
                    }
                }
            }
        }

        log::info!(
            "material xform: offset=({:.3},{:.3}) scale=({:.3},{:.3}) rot={:.3}",
            mat_xf.offset[0], mat_xf.offset[1], mat_xf.scale[0], mat_xf.scale[1], mat_xf.rot
        );
        let wizard_mat_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("material-xform"),
            contents: bytemuck::bytes_of(&mat_xf),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let (wizard_mat_bg, _wizard_tex_view, _wizard_sampler) = if let Some(tex) = &skinned_cpu.base_color_texture {
            log::info!("wizard albedo: {}x{} (srgb={})", tex.width, tex.height, tex.srgb);
            // Debug: dump CPU albedo to disk and basic stats
            {
                let dir = asset_path("data/debug"); let _ = fs::create_dir_all(&dir);
                let out = dir.join("wizard_albedo_cpu.png");
                let _ = image::save_buffer(&out, &tex.pixels, tex.width, tex.height, image::ExtendedColorType::Rgba8);
                let mut rmin=255u8; let mut gmin=255u8; let mut bmin=255u8; let mut rmax=0u8; let mut gmax=0u8; let mut bmax=0u8;
                for px in tex.pixels.chunks_exact(4) { rmin=rmin.min(px[0]); gmin=gmin.min(px[1]); bmin=bmin.min(px[2]); rmax=rmax.max(px[0]); gmax=gmax.max(px[1]); bmax=bmax.max(px[2]); }
                log::info!("wizard albedo cpu rgb min=({},{},{}) max=({},{},{})", rmin,gmin,bmin,rmax,gmax,bmax);
            }
            let size3 = wgpu::Extent3d { width: tex.width, height: tex.height, depth_or_array_layers: 1 };
            let tex_obj = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("wizard-albedo"), size: size3, mip_level_count: 1, sample_count: 1,
                dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[]
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &tex_obj, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &tex.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * tex.width),
                    rows_per_image: Some(tex.height),
                },
                size3,
            );
            let view = tex_obj.create_view(&wgpu::TextureViewDescriptor::default());
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("wizard-sampler"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Nearest,
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                ..Default::default()
            });
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("wizard-material-bg"), layout: &material_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
                    wgpu::BindGroupEntry { binding: 2, resource: wizard_mat_buf.as_entire_binding() },
                ],
            });

            // Note: if further debugging is needed, implement a GPU readback copy here.
            (bg, view, sampler)
        } else {
            log::warn!("wizard albedo: NONE; using 1x1 fallback");
            let size3 = wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 };
            let tex_obj = device.create_texture(&wgpu::TextureDescriptor { label: Some("white-1x1"), size: size3, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8UnormSrgb, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &tex_obj, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &[255,255,255,255],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: Some(1),
                },
                size3,
            );
            let view = tex_obj.create_view(&wgpu::TextureViewDescriptor::default());
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                ..Default::default()
            });
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("wizard-material-bg"), layout: &material_bgl, entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: wizard_mat_buf.as_entire_binding() },
            ] });
            (bg, view, sampler)
        };

        // Load spell data (Fire Bolt)
        let fire_bolt = data_loader::load_spell_spec("spells/fire_bolt.json").ok();

        // Precompute strike times and init FX state
        let hand_right_node = skinned_cpu.hand_right_node;
        let portalopen_strikes = compute_portalopen_strikes(&skinned_cpu, hand_right_node);
        let root_node_copy = skinned_cpu.root_node; // copy before moving skinned_cpu

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
            fx_cube_vb,
            fx_cube_ib,
            fx_cube_index_count,
            ruins_vb,
            ruins_ib,
            ruins_index_count,
            wizard_instances,
            wizard_count: wiz_instances.len() as u32,
            ruins_instances,
            ruins_count: ruin_instances.len() as u32,
            fx_instances,
            fx_count: 0,
            fx_capacity,
            palettes_buf,
            palettes_bg,
            joints_per_wizard,
            wizard_pipeline,
            // debug pipelines removed
            wizard_mat_bg,
            _wizard_mat_buf: wizard_mat_buf,
            _wizard_tex_view,
            _wizard_sampler,
            fx_model_bg,
            _fx_model_buf: fx_model_buf,
            wire_enabled: false,

            start: Instant::now(),
            last_time: 0.0,
            wizard_anim_index,
            wizard_time_offset,
            skinned_cpu,
            portalopen_strikes,
            last_center_phase: 0.0,
            hand_right_node,
            root_node: root_node_copy,
            projectiles: Vec::new(),
            particles: Vec::new(),
            cam_target,
            fire_bolt,
        })
    }

    /// Resize the swapchain while preserving aspect and device limits.
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        let (w, h) = scale_to_max((new_size.width, new_size.height), self.max_dim);
        if (w, h) != (new_size.width, new_size.height) {
            log::warn!(
                "Resized {}x{} exceeds max {}, clamped to {}x{} (aspect kept)",
                new_size.width, new_size.height, self.max_dim, w, h
            );
        }
        self.size = PhysicalSize::new(w, h);
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        self.depth = util::create_depth_view(&self.device, self.config.width, self.config.height, self.config.format);
    }

    /// Render one frame.
    pub fn render(&mut self) -> Result<(), SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Update globals (camera + time)
        let t = self.start.elapsed().as_secs_f32();
        let dt = (t - self.last_time).max(0.0);
        self.last_time = t;
        let aspect = self.config.width as f32 / self.config.height as f32;
        // Orbit around the first wizard; ensure the whole ring is visible.
        let cam_angle = t * 0.35; // radians/sec
        let cam_radius = 8.5;     // closer framing while keeping the ring visible
        let cam = Camera::orbit(self.cam_target, cam_radius, cam_angle, aspect);
        let globals = Globals { view_proj: cam.view_proj().to_cols_array_2d(), time_pad: [t, 0.0, 0.0, 0.0] };
        self.queue.write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));

        // Keep model base identity to avoid moving instances globally
        let shard_mtx = glam::Mat4::IDENTITY;
        let shard_model = Model { model: shard_mtx.to_cols_array_2d(), color: [0.85, 0.15, 0.15], emissive: 0.05, _pad: [0.0; 4] };
        self.queue.write_buffer(&self.shard_model_buf, 0, bytemuck::bytes_of(&shard_model));

        // Update wizard skinning palettes on CPU then upload
        self.update_wizard_palettes(t);
        // Update FX (projectiles + particles)
        self.update_fx(t, dt);

        // Begin commands
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("encoder") });
        {
            use wgpu::*;
            let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("main-pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations { load: LoadOp::Clear(Color { r: 0.02, g: 0.08, b: 0.16, a: 1.0 }), store: StoreOp::Store },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(Operations { load: LoadOp::Clear(1.0), store: StoreOp::Store }),
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

            // Wizards (animated skinning + instancing) using viewer-like sampling
            rpass.set_pipeline(&self.wizard_pipeline);
            // Bind groups: 0=globals, 1=model (identity), 2=palettes, 3=material
            rpass.set_bind_group(0, &self.globals_bg, &[]);
            rpass.set_bind_group(1, &self.shard_model_bg, &[]);
            rpass.set_bind_group(2, &self.palettes_bg, &[]);
            rpass.set_bind_group(3, &self.wizard_mat_bg, &[]);
            rpass.set_vertex_buffer(0, self.wizard_vb.slice(..));
            rpass.set_vertex_buffer(1, self.wizard_instances.slice(..));
            rpass.set_index_buffer(self.wizard_ib.slice(..), IndexFormat::Uint16);
            rpass.draw_indexed(0..self.wizard_index_count, 0, 0..self.wizard_count);

            // Ruins (instanced)
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

            // FX (projectiles + particles) as small cubes
            if self.fx_count > 0 {
                rpass.set_pipeline(&self.inst_pipeline);
                rpass.set_bind_group(0, &self.globals_bg, &[]);
                rpass.set_bind_group(1, &self.fx_model_bg, &[]);
                rpass.set_vertex_buffer(0, self.fx_cube_vb.slice(..));
                rpass.set_vertex_buffer(1, self.fx_instances.slice(..));
                rpass.set_index_buffer(self.fx_cube_ib.slice(..), IndexFormat::Uint16);
                rpass.draw_indexed(0..self.fx_cube_index_count, 0, 0..self.fx_count);
            }
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

impl Renderer {
    fn update_wizard_palettes(&mut self, time_global: f32) {
        // Build palettes for each wizard with its animation + offset.
        if self.wizard_count == 0 { return; }
        let joints = self.joints_per_wizard as usize;
        let mut mats: Vec<glam::Mat4> = Vec::with_capacity(self.wizard_count as usize * joints);
        for i in 0..(self.wizard_count as usize) {
            let t = time_global + self.wizard_time_offset[i];
            let clip = self.select_clip(self.wizard_anim_index[i]);
            let palette = sample_palette(&self.skinned_cpu, clip, t);
            mats.extend(palette);
        }
        // Upload as raw f32x16
        // (debug diagnostic logs removed)
        let mut raw: Vec<[f32; 16]> = Vec::with_capacity(mats.len());
        for m in mats { raw.push(m.to_cols_array()); }
        self.queue.write_buffer(&self.palettes_buf, 0, bytemuck::cast_slice(&raw));
    }

    fn select_clip<'a>(&'a self, idx: usize) -> &'a AnimClip {
        // Choose a clip that actually affects the skin's joints.
        let prefer = [
            match idx { 0 => "PortalOpen", 1 => "Still", _ => "Waiting" },
            "Waiting",
            "Still",
            "PortalOpen",
        ];
        let joints = &self.skinned_cpu.joints_nodes;
        let mut best: Option<&AnimClip> = None;
        let mut best_cov: usize = 0;
        for name in prefer.iter() {
            if let Some(clip) = self.skinned_cpu.animations.get(*name) {
                if clip.duration <= 0.0 { continue; }
                // Coverage: how many joints have any track
                let cov = joints.iter().filter(|&&jn| clip.t_tracks.contains_key(&jn) || clip.r_tracks.contains_key(&jn) || clip.s_tracks.contains_key(&jn)).count();
                if cov > best_cov { best_cov = cov; best = Some(clip); }
            }
        }
        if let Some(c) = best { return c; }
        // Fallback: any non-empty clip
        self.skinned_cpu.animations.values().find(|c| c.duration > 0.0).or_else(|| self.skinned_cpu.animations.values().next()).expect("at least one animation clip present")
    }

    // Drive FX from animation and simulate projectiles/particles
    fn update_fx(&mut self, t: f32, dt: f32) {
        // 1) Trigger firebolt on staff strike for center wizard
        if self.wizard_count > 0 {
            let idx = 0usize; // center
            let clip = self.select_clip(self.wizard_anim_index[idx]);
            if clip.duration > 0.0 {
                let phase = (t + self.wizard_time_offset[idx]) % clip.duration;
                // Check events crossed
                if crossed_event(self.last_center_phase, phase, clip.duration, &self.portalopen_strikes) {
                    if let Some(hand) = self.hand_right_node {
                        if let Some(m_hand) = global_of_node(&self.skinned_cpu, clip, phase, hand) {
                            let c = m_hand.to_cols_array();
                            let origin = glam::vec3(c[12], c[13], c[14]);
                            // Aim using the character root's horizontal forward
                            let dir = if let Some(root) = self.root_node {
                                if let Some(m_root) = global_of_node(&self.skinned_cpu, clip, phase, root) {
                                    // Use the root's +Z axis (forward) flattened to horizontal.
                                    let z_axis = (m_root * glam::Vec4::new(0.0, 0.0, 1.0, 0.0)).truncate();
                                    let mut f = z_axis;
                                    f.y = 0.0; // flatten to horizontal
                                    if f.length_squared() > 1e-6 { f.normalize() } else { glam::Vec3::new(0.0, 0.0, 1.0) }
                                } else { glam::Vec3::new(0.0, 0.0, 1.0) }
                            } else { glam::Vec3::new(0.0, 0.0, 1.0) };
                            self.spawn_firebolt(origin + dir * 0.3, dir, t);
                        }
                    }
                }
                self.last_center_phase = phase;
            }
        }

        // 2) Integrate projectiles
        let mut new_particles: Vec<Particle> = Vec::new();
        for p in &mut self.projectiles {
            p.pos += p.vel * dt;
        }
        // Simple ground hit
        let mut i = 0;
        while i < self.projectiles.len() {
            let kill = t >= self.projectiles[i].t_die || self.projectiles[i].pos.y <= 0.05;
            if kill {
                // spawn impact burst
                for _ in 0..12 {
                    let jitter = glam::vec3(rand_unit()*0.6, rand_unit()*0.6 + 0.3, rand_unit()*0.6);
                    new_particles.push(Particle { pos: self.projectiles[i].pos, vel: jitter * 5.0, age: 0.0, life: 0.25, size: 0.15, color: [1.0, 0.6, 0.2] });
                }
                self.projectiles.swap_remove(i);
            } else { i += 1; }
        }

        // 3) Integrate particles (simple)
        for p in &mut self.particles { p.age += dt; p.pos += p.vel * dt; p.vel *= 0.92; }
        self.particles.retain(|p| p.age < p.life);
        self.particles.extend(new_particles);

        // Also emit a small trail from active projectiles
        for p in &self.projectiles {
            self.particles.push(Particle { pos: p.pos, vel: glam::Vec3::ZERO, age: 0.0, life: 0.18, size: 0.08, color: [1.0, 0.5, 0.1] });
        }

        // 4) Build instance buffer (resize if needed)
        let mut inst: Vec<Instance> = Vec::with_capacity(self.projectiles.len() + self.particles.len());
        for pr in &self.projectiles {
            inst.push(instance_from(pr.pos, 0.12, [1.0, 0.5, 0.1]));
        }
        for pa in &self.particles {
            let k = 1.0 - (pa.age / pa.life).clamp(0.0, 1.0);
            inst.push(instance_from(pa.pos, pa.size * (0.5 + 0.5 * k), pa.color));
        }
        self.fx_count = inst.len() as u32;
        if self.fx_count > 0 {
            let needed = (inst.len() * std::mem::size_of::<Instance>()) as u64;
            if needed > (self.fx_capacity as u64 * std::mem::size_of::<Instance>() as u64) {
                // grow buffer
                self.fx_capacity = (self.fx_count.next_power_of_two()).max(self.fx_capacity * 2);
                self.fx_instances = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("fx-instances"),
                    size: (self.fx_capacity as usize * std::mem::size_of::<Instance>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            self.queue.write_buffer(&self.fx_instances, 0, bytemuck::cast_slice(&inst));
        }
    }

    fn spawn_firebolt(&mut self, origin: glam::Vec3, dir: glam::Vec3, t: f32) {
        let (mut speed, mut radius, mut z_off) = (40.0f32, 0.1f32, 0.3f32);
        if let Some(spec) = &self.fire_bolt {
            if let Some(proj) = &spec.projectile { speed = proj.speed_mps; radius = proj.radius_m; z_off = proj.spawn_offset_m.z.max(0.0); }
        }
        let spawn = origin + dir * z_off;
        self.projectiles.push(Projectile { pos: spawn, vel: dir * speed, radius, t_die: t + 1.0 });
    }
}

fn sample_palette(mesh: &SkinnedMeshCPU, clip: &AnimClip, t: f32) -> Vec<glam::Mat4> {
    use std::collections::HashMap;
    let mut local_t: Vec<glam::Vec3> = mesh.base_t.clone();
    let mut local_r: Vec<glam::Quat> = mesh.base_r.clone();
    let mut local_s: Vec<glam::Vec3> = mesh.base_s.clone();

    let time = if clip.duration > 0.0 { t % clip.duration } else { 0.0 };

    // Apply tracks to local TRS
    for (node, tr) in &clip.t_tracks {
        local_t[*node] = sample_vec3(tr, time, mesh.base_t[*node]);
    }
    for (node, rr) in &clip.r_tracks {
        local_r[*node] = sample_quat(rr, time, mesh.base_r[*node]);
    }
    for (node, sr) in &clip.s_tracks {
        local_s[*node] = sample_vec3(sr, time, mesh.base_s[*node]);
    }

    // Compute global matrices for all nodes touched by joints
    let mut global: HashMap<usize, glam::Mat4> = HashMap::new();
    for &jn in &mesh.joints_nodes {
        if jn < local_t.len() {
            compute_global(jn, &mesh.parent, &local_t, &local_r, &local_s, &mut global);
        }
    }

    // Build palette: global * inverse_bind per joint in skin order
    let mut out = Vec::with_capacity(mesh.joints_nodes.len());
    for (i, &node_idx) in mesh.joints_nodes.iter().enumerate() {
        let g = if node_idx < local_t.len() { *global.get(&node_idx).unwrap_or(&glam::Mat4::IDENTITY) } else { glam::Mat4::IDENTITY };
        let ibm = mesh.inverse_bind.get(i).copied().unwrap_or(glam::Mat4::IDENTITY);
        out.push(g * ibm);
    }
    out
}

fn compute_global(
    node: usize,
    parent: &Vec<Option<usize>>,
    lt: &Vec<glam::Vec3>,
    lr: &Vec<glam::Quat>,
    ls: &Vec<glam::Vec3>,
    cache: &mut std::collections::HashMap<usize, glam::Mat4>,
) -> glam::Mat4 {
    if let Some(m) = cache.get(&node) { return *m; }
    let local = glam::Mat4::from_scale_rotation_translation(ls[node], lr[node], lt[node]);
    let m = if let Some(p) = parent[node] {
        compute_global(p, parent, lt, lr, ls, cache) * local
    } else {
        local
    };
    cache.insert(node, m);
    m
}

fn sample_vec3(tr: &crate::assets::TrackVec3, t: f32, default: glam::Vec3) -> glam::Vec3 {
    if tr.times.is_empty() { return default; }
    if t <= tr.times[0] { return tr.values[0]; }
    if t >= *tr.times.last().unwrap() { return *tr.values.last().unwrap(); }
    let mut i = 0;
    while i + 1 < tr.times.len() && !(t >= tr.times[i] && t <= tr.times[i+1]) { i += 1; }
    let t0 = tr.times[i]; let t1 = tr.times[i+1];
    let f = (t - t0) / (t1 - t0);
    tr.values[i].lerp(tr.values[i+1], f)
}

fn sample_quat(tr: &crate::assets::TrackQuat, t: f32, default: glam::Quat) -> glam::Quat {
    if tr.times.is_empty() { return default; }
    if t <= tr.times[0] { return tr.values[0]; }
    if t >= *tr.times.last().unwrap() { return *tr.values.last().unwrap(); }
    let mut i = 0;
    while i + 1 < tr.times.len() && !(t >= tr.times[i] && t <= tr.times[i+1]) { i += 1; }
    let t0 = tr.times[i]; let t1 = tr.times[i+1];
    let f = (t - t0) / (t1 - t0);
    tr.values[i].slerp(tr.values[i+1], f)
}

fn global_of_node(mesh: &SkinnedMeshCPU, clip: &AnimClip, t: f32, node_idx: usize) -> Option<glam::Mat4> {
    let mut lt = mesh.base_t.clone();
    let mut lr = mesh.base_r.clone();
    let mut ls = mesh.base_s.clone();
    let time = if clip.duration > 0.0 { t % clip.duration } else { 0.0 };
    if let Some(tr) = clip.t_tracks.get(&node_idx) { lt[node_idx] = sample_vec3(tr, time, lt[node_idx]); }
    if let Some(rr) = clip.r_tracks.get(&node_idx) { lr[node_idx] = sample_quat(rr, time, lr[node_idx]); }
    if let Some(sr) = clip.s_tracks.get(&node_idx) { ls[node_idx] = sample_vec3(sr, time, ls[node_idx]); }
    let mut cache = std::collections::HashMap::new();
    Some(compute_global(node_idx, &mesh.parent, &lt, &lr, &ls, &mut cache))
}

fn crossed_event(prev: f32, curr: f32, dur: f32, events: &Vec<f32>) -> bool {
    if events.is_empty() || dur <= 0.0 { return false; }
    if prev <= curr {
        events.iter().any(|&e| e >= prev && e < curr)
    } else {
        // wrap-around
        events.iter().any(|&e| e >= prev || e < curr)
    }
}

fn compute_portalopen_strikes(mesh: &SkinnedMeshCPU, hand_right_node: Option<usize>) -> Vec<f32> {
    let Some(hand) = hand_right_node else { return Vec::new() };
    let Some(clip) = mesh.animations.get("PortalOpen") else { return Vec::new() };
    // Use hand translation track minima on Y
    let Some(trk) = clip.t_tracks.get(&hand) else { return Vec::new() };
    if trk.times.len() < 3 { return Vec::new() }
    let mut min_y = f32::INFINITY; for v in &trk.values { if v.y < min_y { min_y = v.y; } }
    let thresh = min_y + 0.02; // near-low
    let mut out = Vec::new();
    for i in 1..(trk.times.len()-1) {
        let y0 = trk.values[i-1].y; let y1 = trk.values[i].y; let y2 = trk.values[i+1].y;
        if y1 < y0 && y1 < y2 && y1 <= thresh { out.push(trk.times[i]); }
    }
    out
}

fn instance_from(pos: glam::Vec3, scale: f32, color: [f32;3]) -> Instance {
    let m = glam::Mat4::from_scale_rotation_translation(glam::Vec3::splat(scale), glam::Quat::IDENTITY, pos);
    Instance { model: m.to_cols_array_2d(), color, selected: 0.0 }
}

fn rand_unit() -> f32 { use rand::Rng as _; let mut r = rand::rng(); r.random::<f32>() * 2.0 - 1.0 }
