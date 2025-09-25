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
use crate::ecs::{World, Transform, RenderKind};

use std::time::Instant;
 
use wgpu::{rwh::HasDisplayHandle, rwh::HasWindowHandle, util::DeviceExt, SurfaceError, SurfaceTargetUnsafe};
use winit::dpi::PhysicalSize;
use winit::window::Window;
use std::fs;

fn asset_path(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

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
    ruins_vb: wgpu::Buffer,
    ruins_ib: wgpu::Buffer,
    ruins_index_count: u32,

    // Instancing buffers
    wizard_instances: wgpu::Buffer,
    wizard_count: u32,
    ruins_instances: wgpu::Buffer,
    ruins_count: u32,

    // Wizard skinning palettes
    palettes_buf: wgpu::Buffer,
    palettes_bg: wgpu::BindGroup,
    joints_per_wizard: u32,

    // Wizard pipelines
    wizard_pipeline: wgpu::RenderPipeline,
    wizard_wire_pipeline: Option<wgpu::RenderPipeline>,
    wizard_mat_bg: wgpu::BindGroup,
    _wizard_mat_buf: wgpu::Buffer,
    _wizard_tex_view: wgpu::TextureView,
    _wizard_sampler: wgpu::Sampler,

    // Flags
    wire_enabled: bool,

    // Time base for animation
    start: Instant,

    // Wizard animation selection and time offsets
    wizard_anim_index: Vec<usize>,
    wizard_time_offset: Vec<f32>,

    // CPU-side skinned mesh data
    skinned_cpu: SkinnedMeshCPU,

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
        let (wizard_pipeline, wizard_wire_pipeline) =
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
        let plane_model_init = Model { model: glam::Mat4::IDENTITY.to_cols_array_2d(), color: [0.05, 0.80, 0.30], emissive: 0.0, _pad: [0.0; 4] };
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

        let wiz_vertices: Vec<VertexSkinned> = skinned_cpu
            .vertices
            .iter()
            .map(|v| VertexSkinned { pos: v.pos, nrm: v.nrm, joints: v.joints, weights: v.weights, uv: v.uv })
            .collect();

        // Debug: UV range
        if !wiz_vertices.is_empty() {
            let mut umin = f32::INFINITY; let mut vmin = f32::INFINITY; let mut umax = f32::NEG_INFINITY; let mut vmax = f32::NEG_INFINITY;
            for v in &wiz_vertices { umin = umin.min(v.uv[0]); umax = umax.max(v.uv[0]); vmin = vmin.min(v.uv[1]); vmax = vmax.max(v.uv[1]); }
            log::info!("wizard UV range: u=[{:.3},{:.3}] v=[{:.3},{:.3}] verts={}", umin, umax, vmin, vmax, wiz_vertices.len());
        }
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

        // --- Build a tiny ECS world and spawn entities ---
        let mut world = World::new();
        use rand::{SeedableRng, Rng};
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);

        let place_range = plane_extent * 0.4;

        for _ in 0..10 { // 10 wizards
            let translation = glam::vec3(
                rng.random_range(-place_range..place_range),
                0.0,
                rng.random_range(-place_range..place_range),
            );
            let rotation = glam::Quat::from_rotation_y(rng.random::<f32>() * std::f32::consts::TAU);
            world.spawn(Transform { translation, rotation, scale: glam::Vec3::splat(1.0) }, RenderKind::Wizard);
        }
        // Place ~3 ruins spread out
        let ruins_positions = [
            glam::vec3(-place_range, 0.0, -place_range * 0.6),
            glam::vec3(place_range * 0.9, 0.0, 0.0),
            glam::vec3(0.0, 0.0, place_range * 0.8),
        ];
        for pos in ruins_positions { 
            let rotation = glam::Quat::from_rotation_y(rng.random::<f32>() * std::f32::consts::TAU);
            world.spawn(Transform { translation: pos, rotation, scale: glam::Vec3::splat(1.0) }, RenderKind::Ruins);
        }

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
            // 0=PortalOpen,1=Still,2=Waiting (fallback to whatever exists)
            let pick = rng2.random_range(0..3);
            wizard_anim_index.push(pick);
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
            ruins_vb,
            ruins_ib,
            ruins_index_count,
            wizard_instances,
            wizard_count: wiz_instances.len() as u32,
            ruins_instances,
            ruins_count: ruin_instances.len() as u32,
            palettes_buf,
            palettes_bg,
            joints_per_wizard,
            wizard_pipeline,
            wizard_wire_pipeline,
            wizard_mat_bg,
            _wizard_mat_buf: wizard_mat_buf,
            _wizard_tex_view,
            _wizard_sampler,
            wire_enabled: false,

            start: Instant::now(),
            wizard_anim_index,
            wizard_time_offset,
            skinned_cpu,
            cam_target,
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
        let aspect = self.config.width as f32 / self.config.height as f32;
        // Orbit closely around the first wizard we spawned
        // Zoom in further and slow orbit speed by 75%
        let cam = Camera::orbit(self.cam_target, 3.0, t * 0.15, aspect);
        let globals = Globals { view_proj: cam.view_proj().to_cols_array_2d(), time_pad: [t, 0.0, 0.0, 0.0] };
        self.queue.write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));

        // Keep model base identity to avoid moving instances globally
        let shard_mtx = glam::Mat4::IDENTITY;
        let shard_model = Model { model: shard_mtx.to_cols_array_2d(), color: [0.85, 0.15, 0.15], emissive: 0.05, _pad: [0.0; 4] };
        self.queue.write_buffer(&self.shard_model_buf, 0, bytemuck::bytes_of(&shard_model));

        // Update wizard skinning palettes on CPU then upload
        self.update_wizard_palettes(t);

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

            // Wizards (skinned + instanced)
            let wiz_pipe = if self.wire_enabled { self.wizard_wire_pipeline.as_ref().unwrap_or(&self.wizard_pipeline) } else { &self.wizard_pipeline };
            rpass.set_pipeline(wiz_pipe);
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
            rpass.set_vertex_buffer(0, self.ruins_vb.slice(..));
            rpass.set_vertex_buffer(1, self.ruins_instances.slice(..));
            rpass.set_index_buffer(self.ruins_ib.slice(..), IndexFormat::Uint16);
            rpass.draw_indexed(0..self.ruins_index_count, 0, 0..self.ruins_count);
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
        let mut raw: Vec<[f32; 16]> = Vec::with_capacity(mats.len());
        for m in mats { raw.push(m.to_cols_array()); }
        self.queue.write_buffer(&self.palettes_buf, 0, bytemuck::cast_slice(&raw));
    }

    fn select_clip<'a>(&'a self, idx: usize) -> &'a AnimClip {
        // Map 0..2 to PortalOpen/Still/Waiting, fallback to any available
        let name = match idx { 0 => "PortalOpen", 1 => "Still", _ => "Waiting" };
        self.skinned_cpu
            .animations
            .get(name)
            .or_else(|| self.skinned_cpu.animations.values().next())
            .expect("at least one animation clip present")
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
