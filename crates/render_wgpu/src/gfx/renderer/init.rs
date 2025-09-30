//! Renderer initialization moved out of `gfx/mod.rs`.
//!
//! - `new_core` contains the full constructor body (moved here).
//! - `new_renderer` remains a thin wrapper used by `gfx::Renderer::new`.

use anyhow::Context;
use data_runtime::{
    loader as data_loader,
    zone::{ZoneManifest, load_zone_manifest},
};
use ra_assets::skinning::load_gltf_skinned;
use rand::Rng as _;
use std::time::Instant;
use wgpu::{SurfaceTargetUnsafe, util::DeviceExt};
use winit::dpi::PhysicalSize;
use winit::window::Window;

// Bring parent gfx modules into scope so the moved body compiles unchanged.
use crate::gfx::types::{Globals, Model, VertexSkinned};
use crate::gfx::{
    anim, asset_path, camera_sys, foliage, fx, gbuffer, hiz, material, npcs, pipeline, rocks,
    ruins, scene, sky, terrain, ui, util, zombies,
};

pub async fn new_renderer(window: &Window) -> anyhow::Result<crate::gfx::Renderer> {
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

    // Create an instance/surface pair and obtain an adapter
    let (surface, adapter) = {
        let mut picked: Option<(wgpu::Surface<'static>, wgpu::Adapter)> = None;
        for &bmask in candidates {
            // Leak the Instance to extend its lifetime to 'static so the Surface can be 'static too.
            let inst: &'static wgpu::Instance =
                Box::leak(Box::new(wgpu::Instance::new(&wgpu::InstanceDescriptor {
                    backends: bmask,
                    flags: wgpu::InstanceFlags::empty(),
                    ..Default::default()
                })));
            // Surface creation per-target
            #[cfg(target_arch = "wasm32")]
            let surf: wgpu::Surface<'static> = {
                use winit::platform::web::WindowExtWebSys;
                let canvas = window
                    .canvas()
                    .expect("winit web: canvas not available on window");
                // Safe creation path; result has a scoped lifetime. We transmute to 'static
                // because the canvas is attached to the DOM for the lifetime of the page.
                let s = inst
                    .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
                    .context("create wgpu surface (web canvas)")?;
                unsafe { std::mem::transmute::<wgpu::Surface<'_>, wgpu::Surface<'static>>(s) }
            };
            #[cfg(not(target_arch = "wasm32"))]
            let surf = {
                use wgpu::rwh::{HasDisplayHandle, HasWindowHandle};
                let raw_display = window.display_handle()?.as_raw();
                let raw_window = window.window_handle()?.as_raw();
                unsafe {
                    inst.create_surface_unsafe(SurfaceTargetUnsafe::RawHandle {
                        raw_display_handle: raw_display,
                        raw_window_handle: raw_window,
                    })
                }
                .context("create wgpu surface (unsafe)")?
            };

            match inst
                .request_adapter(&wgpu::RequestAdapterOptions {
                    compatible_surface: Some(&surf),
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    force_fallback_adapter: false,
                })
                .await
            {
                Ok(a) => {
                    picked = Some((surf, a));
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
    let (w, h) = util::scale_to_max((size.width, size.height), max_dim);
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
    let attachments = super::Attachments::create(
        &device,
        config.width,
        config.height,
        config.format,
        wgpu::TextureFormat::Rgba16Float,
    );
    // Legacy fields: mirror attachments for existing struct layout
    let _depth = attachments.depth_view.clone();
    let _scene_color = attachments.scene_color.clone();
    let _scene_view = attachments.scene_view.clone();
    let _scene_read = attachments.scene_read.clone();
    let _scene_read_view = attachments.scene_read_view.clone();
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

    // Model buffers for terrain/shards
    let plane_model_init = Model {
        model: glam::Mat4::IDENTITY.to_cols_array_2d(),
        color: [0.20, 0.35, 0.22],
        emissive: 0.0,
        _pad: [0.0; 4],
    };
    let _plane_model_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terrain-model"),
        contents: bytemuck::bytes_of(&plane_model_init),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let plane_model_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("terrain-model-bg"),
        layout: &model_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: _plane_model_buf.as_entire_binding(),
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

    // Sky uniforms and zone setup
    // On wasm, avoid std::fs and synthesize a minimal zone manifest.
    #[cfg(target_arch = "wasm32")]
    let zone: ZoneManifest = ZoneManifest {
        zone_id: 1,
        slug: "wizard_woods".to_string(),
        display_name: "Wizard Woods".to_string(),
        plane: data_runtime::zone::ZonePlane::Material,
        terrain: data_runtime::zone::TerrainSpec { size: 129, extent: 50.0, seed: 4242 },
        weather: None,
        vegetation: Some(data_runtime::zone::VegetationSpec { tree_count: 0, tree_seed: 0 }),
        start_time_frac: Some(0.48),
        start_paused: Some(false),
        start_time_scale: Some(1.0),
    };
    #[cfg(not(target_arch = "wasm32"))]
    let zone: ZoneManifest =
        load_zone_manifest("wizard_woods").context("load zone manifest: wizard_woods")?;
    log::info!(
        "Zone '{}' (id={}, plane={:?})",
        zone.display_name,
        zone.zone_id,
        zone.plane
    );
    let mut sky_state = sky::SkyStateCPU::new();
    if let Some(w) = zone.weather {
        sky_state.weather = crate::gfx::sky::Weather {
            turbidity: w.turbidity,
            ground_albedo: w.ground_albedo,
        };
        sky_state.recompute();
    }
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
    // Samplers used across post/present passes
    // Non-filtering sampler for depth sampling (required by WebGPU when not using comparison)
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
    // Filtering sampler for color textures
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
    let post_ao_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("post-ao-bg"),
        layout: &post_ao_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&attachments.depth_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                // Non-filtering sampler for depth sampling
                resource: wgpu::BindingResource::Sampler(&point_sampler),
            },
        ],
    });
    // point_sampler already created above
    let ssgi_globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ssgi-globals-bg"),
        layout: &ssgi_globals_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: globals_buf.as_entire_binding(),
        }],
    });

    // Bloom bind group reads from SceneRead (copy of SceneColor)
    let bloom_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bloom-bg"),
        layout: &bloom_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&attachments.scene_read_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&post_sampler),
            },
        ],
    });

    // Present & post bindings referencing sized textures
    let present_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("present-bg"),
        layout: &present_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&attachments.scene_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&post_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&attachments.depth_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&point_sampler),
            },
        ],
    });
    let ssgi_depth_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ssgi-depth-bg"),
        layout: &ssgi_depth_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&attachments.depth_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                // Non-filtering sampler for depth sampling
                resource: wgpu::BindingResource::Sampler(&point_sampler),
            },
        ],
    });
    // SSR depth BG uses the linear depth view from Hi-Z pyramid
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
    let ssgi_scene_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ssgi-scene-bg"),
        layout: &ssgi_scene_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&attachments.scene_read_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&post_sampler),
            },
        ],
    });
    let ssr_scene_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ssr-scene-bg"),
        layout: &ssr_scene_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&attachments.scene_read_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&post_sampler),
            },
        ],
    });

    // Terrain & world
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
    let ZoneManifest { slug, .. } = zone.clone();

    // Ruins mesh + metrics
    #[cfg(not(target_arch = "wasm32"))]
    let ruins_gpu = ruins::build_ruins(&device).context("build ruins mesh")?;
    #[cfg(target_arch = "wasm32")]
    let ruins_gpu = ruins::build_ruins(&device).unwrap_or_else(|_| {
        // Fallback to a cube if GLTF fails (should not, as we embed ruins.gltf)
        let (vb, ib, index_count) = super::super::mesh::create_cube(&device);
        super::super::ruins::RuinsGpu { vb, ib, index_count, base_offset: 0.0, radius: 1.0 }
    });
    let ruins_base_offset = ruins_gpu.base_offset;
    let ruins_radius = ruins_gpu.radius;
    let (ruins_vb, ruins_ib, ruins_index_count) =
        (ruins_gpu.vb, ruins_gpu.ib, ruins_gpu.index_count);

    // Load wizard GLTF, possibly merging UVs from simple loader for robustness
    let skinned_cpu = load_gltf_skinned(&asset_path("assets/models/wizard.gltf"))
        .context("load skinned wizard.gltf")?;
    let viewer_uv: Option<Vec<[f32; 2]>> = (|| {
        let (doc, buffers, _images) = gltf::import(asset_path("assets/models/wizard.gltf")).ok()?;
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

    // Zombie assets (skinned)
    let zombie_assets = zombies::load_assets(&device).context("load zombie assets")?;
    let zombie_cpu = zombie_assets.cpu;
    let zombie_vb = zombie_assets.vb;
    let zombie_ib = zombie_assets.ib;
    let zombie_index_count = zombie_assets.index_count;

    // Scene assembly
    let scene_build = scene::build_demo_scene(
        &device,
        &skinned_cpu,
        terrain_extent,
        Some(&terrain_cpu),
        ruins_base_offset,
        ruins_radius,
    );
    // Snap initial wizard ring onto terrain height
    let mut wizard_models = scene_build.wizard_models.clone();
    for m in &mut wizard_models {
        let c = m.to_cols_array();
        let (h, _n) = terrain::height_at(&terrain_cpu, c[12], c[14]);
        let (s, r, _t) = glam::Mat4::from_cols_array(&c).to_scale_rotation_translation();
        *m = glam::Mat4::from_scale_rotation_translation(s, r, glam::vec3(c[12], h, c[14]));
    }
    let mut wizard_instances_cpu = scene_build.wizard_instances_cpu.clone();
    for (i, inst) in wizard_instances_cpu.iter_mut().enumerate() {
        inst.model = wizard_models[i].to_cols_array_2d();
    }
    let wizard_instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("wizard-instances"),
        contents: bytemuck::cast_slice(&wizard_instances_cpu),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });
    let pc_initial_pos = {
        let m = scene_build.wizard_models[scene_build.pc_index];
        let c = m.to_cols_array();
        glam::vec3(c[12], c[13], c[14])
    };

    // Upload UI atlases
    nameplates.upload_atlas(&queue);
    nameplates_npc.upload_atlas(&queue);
    damage.upload_atlas(&queue);

    // FX resources
    let fx_res = fx::create_fx_resources(&device, &model_bgl);
    let fx_instances = fx_res.instances;
    let _fx_model_bg = fx_res.model_bg;
    let quad_vb = fx_res.quad_vb;
    let fx_capacity = fx_res.capacity;
    let fx_count: u32 = 0;
    let fire_bolt = data_loader::load_spell_spec("spells/fire_bolt.json").ok();
    let hand_right_node = skinned_cpu.hand_right_node;
    let root_node = skinned_cpu.root_node;
    let _strikes_tmp = anim::compute_portalopen_strikes(&skinned_cpu, hand_right_node, root_node);

    // Palettes
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
    let zombie_joints = zombie_cpu.joints_nodes.len() as u32;

    // Materials
    let wmat = material::create_wizard_material(&device, &queue, &material_bgl, &skinned_cpu);
    let wizard_mat_bg = wmat.bind_group;
    let _wizard_mat_buf = wmat.uniform_buf;
    let _wizard_tex_view = wmat.texture_view;
    let _wizard_sampler = wmat.sampler;
    let zmat = material::create_wizard_material(&device, &queue, &material_bgl, &zombie_cpu);
    let zombie_mat_bg = zmat.bind_group;
    let _zombie_mat_buf = zmat.uniform_buf;
    let _zombie_tex_view = zmat.texture_view;
    let _zombie_sampler = zmat.sampler;

    // Death Knight assets (skinned, single instance)
    let dk_assets =
        super::super::deathknight::load_assets(&device).context("load deathknight assets")?;
    let dk_cpu = dk_assets.cpu;
    let dk_vb = dk_assets.vb;
    let dk_ib = dk_assets.ib;
    let dk_index_count = dk_assets.index_count;
    let dk_joints = dk_cpu.joints_nodes.len() as u32;
    let dk_mat = material::create_wizard_material(&device, &queue, &material_bgl, &dk_cpu);
    let dk_mat_bg = dk_mat.bind_group;
    let _dk_mat_buf = dk_mat.uniform_buf;
    let _dk_tex_view = dk_mat.texture_view;
    let _dk_sampler = dk_mat.sampler;

    // NPCs and server
    let npcs = npcs::build(&device, terrain_extent);
    let npc_vb = npcs.vb;
    let npc_ib = npcs.ib;
    let npc_index_count = npcs.index_count;
    let npc_instances = npcs.instances;
    let npc_models = npcs.models;
    let mut server = npcs.server;

    // Vegetation
    let veg = zone
        .vegetation
        .as_ref()
        .map(|v| (v.tree_count as usize, v.tree_seed));
    // On wasm, tree GLTF uses external .bin; skip trees by setting count=0 above.
    let trees_gpu = foliage::build_trees(&device, &terrain_cpu, &slug, veg)
        .context("build trees (instances + mesh) for zone")?;
    let trees_instances = trees_gpu.instances;
    let trees_count = trees_gpu.count;
    let (trees_vb, trees_ib, trees_index_count) =
        (trees_gpu.vb, trees_gpu.ib, trees_gpu.index_count);

    // Rocks
    #[cfg(not(target_arch = "wasm32"))]
    let rocks_gpu = rocks::build_rocks(&device, &terrain_cpu, &slug, None)
        .context("build rocks (instances + mesh) for zone")?;
    #[cfg(target_arch = "wasm32")]
    let rocks_gpu = {
        // Build zero rock instances; still upload a small cube mesh for bindings.
        let instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rocks-instances-empty"),
            contents: &[],
            usage: wgpu::BufferUsages::VERTEX,
        });
        let (vb, ib, index_count) = super::super::mesh::create_cube(&device);
        super::super::rocks::RocksGpu { instances, count: 0, vb, ib, index_count }
    };
    let rocks_instances = rocks_gpu.instances;
    let rocks_count = rocks_gpu.count;
    let (rocks_vb, rocks_ib, rocks_index_count) =
        (rocks_gpu.vb, rocks_gpu.ib, rocks_gpu.index_count);

    // UI prep
    bars.queue_entries(
        &device,
        &queue,
        config.width,
        config.height,
        glam::Mat4::IDENTITY,
        &[],
    );
    hud.upload_atlas(&queue);

    // Zombie instances from server
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

    // Death Knight single-instance buffers and palettes
    let (dk_instances, dk_instances_cpu, dk_models, dk_count) =
        super::super::deathknight::build_instances(&device, &terrain_cpu, dk_joints);
    // Spawn Death Knight into the server so spells collide and AI runs
    let dk_spawn_pos = {
        let c = dk_models[0].to_cols_array();
        glam::vec3(c[12], c[13], c[14])
    };
    let dk_id = {
        let radius = 2.5f32; // generous cylinder radius for scaled model
        let hp = 1000i32; // 5x previous hitpoints
        let id = server.spawn_npc(dk_spawn_pos, radius, hp);
        // Scale Death Knight damage 10x over a zombie (5 → 50) and double speed
        if let Some(n) = server.npcs.iter_mut().find(|n| n.id == id) {
            n.damage = 50;
            n.speed = 4.0;
        }
        id
    };
    let total_dk_mats = dk_count as usize * dk_joints as usize;
    let dk_palettes_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("deathknight-palettes"),
        size: (total_dk_mats * 64) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let dk_palettes_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("deathknight-palettes-bg"),
        layout: &palettes_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: dk_palettes_buf.as_entire_binding(),
        }],
    });

    // Determine asset forward offset from the zombie root node (if present).
    let zombie_forward_offset = zombies::forward_offset(&zombie_cpu);

    // Ability timings from SpecDb (canonical)
    let specdb = data_runtime::specdb::SpecDb::load_default();
    let (pc_cast_time, firebolt_cd_dur) = if let Some(fb) = specdb.get_spell("wiz.fire_bolt.srd521")
    {
        (fb.cast_time_s, fb.cooldown_s)
    } else {
        (1.5, 0.5)
    };
    let (mm_cast_time, mm_cd_dur) = if let Some(mm) = specdb.get_spell("wiz.magic_missile.srd521") {
        (mm.cast_time_s, mm.cooldown_s)
    } else {
        (1.0, 0.0)
    };
    let (fb_cast_time, fb_cd_dur) = if let Some(fb) = specdb.get_spell("wiz.fireball.srd521") {
        (fb.cast_time_s, fb.cooldown_s)
    } else {
        (1.0, 2.0)
    };

    Ok(crate::gfx::Renderer {
        surface,
        device,
        queue,
        config,
        size: PhysicalSize::new(w, h),
        max_dim,
        attachments,
        gbuffer: Some(gbuffer),
        hiz: Some(hiz),
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
        direct_present,
        lights_buf,
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
        terrain_model_bg: plane_model_bg,
        shard_model_bg,
        present_bg,
        enable_post_ao: false,
        enable_ssgi: false,
        enable_ssr: false,
        enable_bloom: true,
        static_index: None,
        frame_counter: 0,
        draw_calls: 0,
        globals_buf,
        sky_buf,
        _plane_model_buf,
        shard_model_buf,
        terrain_vb,
        terrain_ib,
        terrain_index_count,
        wizard_vb,
        wizard_ib,
        wizard_index_count,
        dk_vb,
        dk_ib,
        dk_index_count,
        zombie_vb,
        zombie_ib,
        zombie_index_count,
        ruins_vb,
        ruins_ib,
        ruins_index_count,
        npc_vb,
        npc_ib,
        npc_index_count,
        npc_instances,
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
        wizard_instances,
        wizard_count: wizard_instances_cpu.len() as u32,
        dk_instances,
        dk_count,
        dk_instances_cpu,
        zombie_instances,
        zombie_count: zombie_instances_cpu.len() as u32,
        zombie_instances_cpu,
        ruins_instances: scene_build.ruins_instances,
        ruins_count: scene_build.ruins_count,
        fx_instances,
        _fx_capacity: fx_capacity,
        fx_count,
        _fx_model_bg,
        quad_vb,
        palettes_buf,
        palettes_bg,
        joints_per_wizard: scene_build.joints_per_wizard,
        wizard_models,
        wizard_instances_cpu,
        wizard_pipeline,
        wizard_mat_bg,
        _wizard_mat_buf,
        _wizard_tex_view,
        _wizard_sampler,
        dk_mat_bg,
        _dk_mat_buf,
        _dk_tex_view,
        _dk_sampler,
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
        pc_index: scene_build.pc_index,
        player: client_core::controller::PlayerController::new(pc_initial_pos),
        scene_inputs: client_runtime::SceneInputs::new(pc_initial_pos),
        input: Default::default(),
        cam_follow: camera_sys::FollowState {
            current_pos: glam::vec3(0.0, 5.0, -10.0),
            current_look: scene_build.cam_target,
        },
        pc_cast_queued: false,
        pc_cast_kind: Some(super::super::PcCast::FireBolt),
        pc_anim_start: None,
        pc_cast_time,
        magic_missile_cast_time: mm_cast_time,
        magic_missile_cd_dur: mm_cd_dur,
        fireball_cast_time: fb_cast_time,
        fireball_cd_dur: fb_cd_dur,
        pc_cast_fired: false,
        firebolt_cd_dur,
        cam_orbit_yaw: 0.0,
        cam_orbit_pitch: 0.2,
        cam_distance: 8.5,
        cam_lift: 3.5,
        cam_look_height: 1.6,
        rmb_down: false,
        last_cursor_pos: None,
        screenshot_start: None,
        server,
        wizard_hp: vec![100; scene_build.wizard_count as usize],
        wizard_hp_max: 100,
        pc_alive: true,
        wizards_hostile_to_pc: false,
        wizard_fire_cycle_count: vec![0; scene_build.wizard_count as usize],
        wizard_fireball_next_at: {
            let mut v = vec![0u32; scene_build.wizard_count as usize];
            // randomize thresholds 3..=5 for NPC wizards; PC index ignored at use-site
            for x in &mut v {
                let mut r = rand::rng();
                let t: u32 = r.random_range(3..=5);
                *x = t;
            }
            v
        },
        dk_palettes_buf,
        dk_palettes_bg,
        dk_joints,
        dk_models: dk_models.clone(),
        dk_cpu,
        dk_time_offset: (0..dk_count as usize).map(|_| 0.0f32).collect(),
        dk_id: Some(dk_id),
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
                let c = m.to_cols_array();
                glam::vec3(c[12], c[13], c[14])
            })
            .collect(),
        zombie_forward_offsets: vec![zombie_forward_offset; zombie_count as usize],
        dk_prev_pos: dk_models
            .first()
            .map(|m| {
                let c = m.to_cols_array();
                glam::vec3(c[12], c[13], c[14])
            })
            .unwrap_or(glam::Vec3::ZERO),
    })
}
