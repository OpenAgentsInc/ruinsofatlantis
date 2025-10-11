//! Renderer initialization moved out of `gfx/mod.rs`.
//!
//! - `new_core` contains the full constructor body (moved here).
//! - `new_renderer` remains a thin wrapper used by `gfx::Renderer::new`.

use crate::gfx::asset_path;
use anyhow::Context;
#[cfg(not(target_arch = "wasm32"))]
use data_runtime::zone::load_zone_manifest;
use data_runtime::{loader as data_loader, zone::ZoneManifest};
use rand::Rng as _;
use roa_assets::skinning::load_gltf_skinned;
// Monotonic clock: std::time::Instant isn't available on wasm32-unknown-unknown.
#[cfg(feature = "vox_onepath_demo")]
use glam::{DVec3, UVec3};
#[cfg(feature = "vox_onepath_demo")]
use server_core::destructible::config::DestructibleConfig;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(not(feature = "vox_onepath_demo"))]
use voxel_proxy::VoxelGrid;
#[cfg(feature = "vox_onepath_demo")]
use voxel_proxy::{VoxelProxyMeta, voxelize_surface_fill};
#[cfg(target_arch = "wasm32")]
use web_time::Instant;
#[cfg(not(target_arch = "wasm32"))]
use wgpu::SurfaceTargetUnsafe;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::window::Window;

// Bring parent gfx modules into scope so the moved body compiles unchanged.
use crate::gfx::types::{Globals, Model, VertexSkinned};
use crate::gfx::{
    anim, camera_sys, foliage, fx, gbuffer, hiz, material, npcs, pipeline, rocks, ruins, scene,
    sky, terrain, ui, util, zombies,
};

pub async fn new_renderer(window: &Window) -> anyhow::Result<crate::gfx::Renderer> {
    // Only load heavy actor/NPC assets up front when a zone is explicitly selected (native).
    let load_actor_assets = std::env::var("ROA_ZONE").ok().is_some();

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
    log::debug!("Adapter: {:?} ({:?})", info.name, info.backend);
    log::debug!("features: {:?}", adapter.features());
    log::debug!("limits:   {:?}", adapter.limits());
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
    #[cfg(target_arch = "wasm32")]
    let format = {
        // On WebGPU (Dawn), some Linux/Wayland stacks fail to create BGRA shared images.
        // Prefer RGBA8 to avoid "SharedImageBackingFactory ... format: BGRA_8888" errors.
        let formats = caps.formats.clone();
        formats
            .iter()
            .copied()
            .find(|f| *f == wgpu::TextureFormat::Rgba8UnormSrgb)
            .or_else(|| {
                formats
                    .iter()
                    .copied()
                    .find(|f| *f == wgpu::TextureFormat::Rgba8Unorm)
            })
            .or_else(|| {
                formats
                    .iter()
                    .copied()
                    .find(|f| *f == wgpu::TextureFormat::Bgra8UnormSrgb)
            })
            .or_else(|| {
                formats
                    .iter()
                    .copied()
                    .find(|f| *f == wgpu::TextureFormat::Bgra8Unorm)
            })
            .unwrap_or(caps.formats[0])
    };
    #[cfg(not(target_arch = "wasm32"))]
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
        log::debug!(
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
    log::debug!(
        "swapchain configured: fmt={:?} srgb={} size={}x{} present={:?}",
        config.format,
        config.format.is_srgb(),
        config.width,
        config.height,
        present_mode
    );
    // Choose offscreen color format
    // Web: prefer Rgba8Unorm for maximum compatibility across Linux/Wayland stacks.
    // We still apply linear->sRGB in the present pass when swapchain is non‑sRGB.
    #[cfg(target_arch = "wasm32")]
    let offscreen_fmt = wgpu::TextureFormat::Rgba8Unorm;
    #[cfg(not(target_arch = "wasm32"))]
    let offscreen_fmt = wgpu::TextureFormat::Rgba16Float;

    let attachments = super::Attachments::create(
        &device,
        config.width,
        config.height,
        config.format,
        offscreen_fmt,
    );
    log::debug!(
        "attachments: swapchain={:?} offscreen={:?}",
        config.format,
        offscreen_fmt
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
    // Web: prefer offscreen → present when the swapchain is not sRGB so we can
    // apply tonemap + linear→sRGB encode in `present.wgsl`. Direct-presenting
    // to a non‑sRGB swapchain produces an image that looks far too dark.
    #[cfg(target_arch = "wasm32")]
    let mut direct_present = false;
    #[cfg(not(target_arch = "wasm32"))]
    let direct_present = std::env::var("RA_DIRECT_PRESENT")
        .map(|v| v != "0")
        .unwrap_or(true);
    // If swapchain format is sRGB we can safely direct-present; otherwise keep
    // offscreen so present.wgsl can sRGB-encode for correct brightness.
    #[cfg(target_arch = "wasm32")]
    {
        if config.format.is_srgb() {
            direct_present = true;
            log::debug!(
                "swapchain {:?} is sRGB; enabling direct-present on web",
                config.format
            );
        } else {
            log::debug!(
                "swapchain {:?} is not sRGB; using offscreen+present for gamma-correct output",
                config.format
            );
        }
    }

    let draw_fmt = if direct_present {
        config.format
    } else {
        offscreen_fmt
    };
    log::debug!(
        "render path: direct_present={} draw_fmt={:?}",
        direct_present,
        draw_fmt
    );
    let (pipeline, inst_pipeline, wire_pipeline) =
        pipeline::create_pipelines(&device, &shader, &globals_bgl, &model_bgl, draw_fmt);
    let inst_tex_pipeline = pipeline::create_textured_inst_pipeline(
        &device,
        &shader,
        &globals_bgl,
        &model_bgl,
        &palettes_bgl,
        &material_bgl,
        draw_fmt,
    );
    // Create a default 1x1 white material BG for textured pipelines (trees fallback)
    let mat_xf_zero = [0.0f32; 8];
    let default_mat_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("material-xform(default)"),
        contents: bytemuck::bytes_of(&mat_xf_zero),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let white_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("white-1x1"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &white_tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &[255, 255, 255, 255],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let white_view = white_tex.create_view(&wgpu::TextureViewDescriptor::default());
    let white_sam = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("white-sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });
    let default_material_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("default-material-bg"),
        layout: &material_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&white_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&white_sam),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: default_mat_buf.as_entire_binding(),
            },
        ],
    });
    // Sky background
    let sky_bgl = pipeline::create_sky_bgl(&device);
    let sky_pipeline = pipeline::create_sky_pipeline(&device, &globals_bgl, &sky_bgl, draw_fmt);
    // Present pipeline (SceneColor -> swapchain)
    let present_bgl = pipeline::create_present_bgl(&device);
    let present_pipeline =
        pipeline::create_present_pipeline(&device, &globals_bgl, &present_bgl, config.format);
    let blit_scene_read_pipeline =
        pipeline::create_blit_pipeline(&device, &present_bgl, offscreen_fmt);
    // Bloom
    let bloom_bgl = pipeline::create_bloom_bgl(&device);
    let bloom_pipeline = pipeline::create_bloom_pipeline(&device, &bloom_bgl, offscreen_fmt);
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
        offscreen_fmt,
    );
    let ssr_pipeline =
        pipeline::create_ssr_pipeline(&device, &ssr_depth_bgl, &ssr_scene_bgl, offscreen_fmt);
    let (wizard_pipeline, _wizard_wire_pipeline_unused) = pipeline::create_wizard_pipelines(
        &device,
        &shader,
        &globals_bgl,
        &model_bgl,
        &palettes_bgl,
        &material_bgl,
        draw_fmt,
    );
    let wizard_pipeline_debug = Some(pipeline::create_wizard_pipeline_debug(
        &device,
        &shader,
        &globals_bgl,
        &model_bgl,
        &palettes_bgl,
        &material_bgl,
        draw_fmt,
    ));
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
    // HUD is drawn after present directly to the swapchain; build it against the
    // swapchain format to avoid attachment mismatches when offscreen is RGBA8.
    let hud = ui::Hud::new(&device, config.format)?;
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
    // On wasm, avoid std::fs and synthesize the zone manifest to mirror desktop.
    #[cfg(target_arch = "wasm32")]
    let zone: ZoneManifest = ZoneManifest {
        zone_id: 1001,
        slug: "wizard_woods".to_string(),
        display_name: "Wizard Woods".to_string(),
        plane: data_runtime::zone::ZonePlane::Material,
        terrain: data_runtime::zone::TerrainSpec {
            size: 129,
            extent: 150.0,
            seed: 1337,
        },
        weather: Some(data_runtime::zone::WeatherSpec {
            turbidity: 3.0,
            ground_albedo: [0.10, 0.10, 0.10],
        }),
        vegetation: Some(data_runtime::zone::VegetationSpec {
            tree_count: 0,
            tree_seed: 20250926,
        }),
        start_time_frac: Some(0.95),
        start_paused: Some(true),
        start_time_scale: Some(6.0),
    };
    #[cfg(not(target_arch = "wasm32"))]
    // For sky/terrain defaults during renderer init (desktop), load a baseline manifest.
    // This does not control gameplay; platform-selected zones still drive presentation.
    let zone: ZoneManifest =
        load_zone_manifest("wizard_woods").context("load zone manifest: wizard_woods")?;
    log::debug!(
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
    log::debug!(
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
                // Non‑filtering sampler to support non‑filterable HDR formats on Web
                resource: wgpu::BindingResource::Sampler(&point_sampler),
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
        super::super::ruins::RuinsGpu {
            vb,
            ib,
            index_count,
            base_offset: 0.0,
            radius: 1.0,
        }
    });
    let ruins_base_offset = ruins_gpu.base_offset;
    let ruins_radius = ruins_gpu.radius;
    let (ruins_vb, ruins_ib, ruins_index_count) =
        (ruins_gpu.vb, ruins_gpu.ib, ruins_gpu.index_count);

    // Load wizard GLTF, possibly merging UVs from simple loader for robustness
    let skinned_cpu = if load_actor_assets {
        load_gltf_skinned(&asset_path("assets/models/wizard.gltf"))
            .context("load skinned wizard.gltf")?
    } else {
        crate::gfx::Renderer::empty_skinned_cpu()
    };
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
                joints: [
                    v.joints[0] as u32,
                    v.joints[1] as u32,
                    v.joints[2] as u32,
                    v.joints[3] as u32,
                ],
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
    let (zombie_cpu, zombie_vb, zombie_ib, zombie_index_count) = if load_actor_assets {
        let a = zombies::load_assets(&device).context("load zombie assets")?;
        (a.cpu, a.vb, a.ib, a.index_count)
    } else {
        let b = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("zombie-empty"),
            size: 4,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        (
            crate::gfx::Renderer::empty_skinned_cpu(),
            b.clone(),
            b,
            0u32,
        )
    };

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
        size: ((total_mats.max(1)) * 64) as u64,
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

    // PC (UBC male) assets: load model + merge animation library
    let (
        pc_vb,
        pc_ib,
        pc_index_count,
        pc_joints,
        pc_mat_bg,
        pc_cpu,
        pc_instances,
        pc_palettes_buf,
        pc_palettes_bg,
    ) = if load_actor_assets {
        use crate::gfx::types::{InstanceSkin, VertexSkinned};
        use roa_assets::skinning::{load_gltf_skinned, merge_gltf_animations};
        let ubc_rel = "assets/models/ubc/godot/Superhero_Male.gltf";
        let ubc_path = super::super::asset_path(ubc_rel);
        let cpu_pc = match load_gltf_skinned(&ubc_path) {
            Ok(c) => Some(c),
            Err(e) => {
                #[cfg(target_arch = "wasm32")]
                log::warn!(
                    "PC: failed to load UBC male at {}: {e:?}. Falling back to wizard rig for PC.",
                    ubc_rel
                );
                #[cfg(not(target_arch = "wasm32"))]
                log::warn!(
                    "PC: failed to load UBC male at {:?}: {e:?}. Falling back to wizard rig for PC.",
                    ubc_path
                );
                None
            }
        };
        if let Some(mut cpu_pc) = cpu_pc {
            if !cpu_pc.animations.is_empty() {
                log::info!(
                    target: "model_viewer",
                    "PC: UBC male loaded: verts={}, indices={}, joints={}",
                    cpu_pc.vertices.len(),
                    cpu_pc.indices.len(),
                    cpu_pc.joints_nodes.len()
                );
            }
            // Merge universal animation library if present
            let lib_path = super::super::asset_path("assets/anims/universal/AnimationLibrary.glb");
            if lib_path.exists() {
                if let Err(e) = merge_gltf_animations(&mut cpu_pc, &lib_path) {
                    log::warn!(
                        "PC: failed to merge animation library at {:?}: {e:?}",
                        lib_path
                    );
                } else {
                    let names: Vec<String> = cpu_pc.animations.keys().cloned().collect();
                    log::info!(
                        "PC: merged GLTF animations from {:?} ({} clips)",
                        lib_path,
                        names.len()
                    );
                    log::info!("PC: available clips: {}", names.join(", "));
                }
            }
            // Build VB/IB for PC if loaded
            if cpu_pc.vertices.is_empty() || cpu_pc.indices.is_empty() {
                (None, None, 0u32, 0u32, None, None, None, None, None)
            } else {
                let verts: Vec<VertexSkinned> = cpu_pc
                    .vertices
                    .iter()
                    .map(|v| VertexSkinned {
                        pos: v.pos,
                        nrm: v.nrm,
                        uv: v.uv,
                        joints: [
                            v.joints[0] as u32,
                            v.joints[1] as u32,
                            v.joints[2] as u32,
                            v.joints[3] as u32,
                        ],
                        weights: v.weights,
                    })
                    .collect();
                let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("pc-ubc-vb"),
                    contents: bytemuck::cast_slice(&verts),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("pc-ubc-ib"),
                    contents: bytemuck::cast_slice(&cpu_pc.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
                let index_count = cpu_pc.indices.len() as u32;
                let joints = cpu_pc.joints_nodes.len() as u32;
                let pc_mat =
                    material::create_wizard_material(&device, &queue, &material_bgl, &cpu_pc);
                // Instance at initial PC position
                let pc_initial_pos = {
                    let m = scene_build.wizard_models[scene_build.pc_index];
                    let c = m.to_cols_array();
                    glam::vec3(c[12], c[13], c[14])
                };
                let m = glam::Mat4::from_scale_rotation_translation(
                    glam::Vec3::splat(1.0),
                    glam::Quat::IDENTITY,
                    pc_initial_pos,
                );
                let inst_cpu = InstanceSkin {
                    model: m.to_cols_array_2d(),
                    color: [1.0, 1.0, 1.0],
                    selected: 1.0,
                    palette_base: 0,
                    _pad_inst: [0; 3],
                };
                let inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("pc-ubc-instance"),
                    contents: bytemuck::bytes_of(&inst_cpu),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });
                // Palette buffer (single instance)
                let pc_pal_buf = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("pc-ubc-palettes"),
                    size: ((joints.max(1) as usize) * 64) as u64,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let pc_pal_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("pc-ubc-palettes-bg"),
                    layout: &palettes_bgl,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: pc_pal_buf.as_entire_binding(),
                    }],
                });
                (
                    Some(vb),
                    Some(ib),
                    index_count,
                    joints,
                    Some(pc_mat.bind_group),
                    Some(cpu_pc),
                    Some(inst_buf),
                    Some(pc_pal_buf),
                    Some(pc_pal_bg),
                )
            }
        } else {
            // Disable separate PC rig: yield Nones from this block
            (None, None, 0u32, 0u32, None, None, None, None, None)
        }
    } else {
        (None, None, 0u32, 0u32, None, None, None, None, None)
    };

    // Death Knight assets (skinned, single instance)
    let (
        dk_cpu,
        dk_vb,
        dk_ib,
        dk_index_count,
        dk_joints,
        dk_mat_bg,
        _dk_mat_buf,
        _dk_tex_view,
        _dk_sampler,
    ) = if load_actor_assets {
        let a =
            super::super::deathknight::load_assets(&device).context("load deathknight assets")?;
        let cpu = a.cpu;
        let joints = cpu.joints_nodes.len() as u32;
        let mat = material::create_wizard_material(&device, &queue, &material_bgl, &cpu);
        (
            cpu,
            a.vb,
            a.ib,
            a.index_count,
            joints,
            mat.bind_group,
            mat.uniform_buf,
            mat.texture_view,
            mat.sampler,
        )
    } else {
        let dummy = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dk-empty"),
            size: 4,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        let empty_cpu = crate::gfx::Renderer::empty_skinned_cpu();
        let mat = material::create_wizard_material(&device, &queue, &material_bgl, &empty_cpu);
        (
            empty_cpu,
            dummy.clone(),
            dummy,
            0u32,
            0u32,
            mat.bind_group,
            mat.uniform_buf,
            mat.texture_view,
            mat.sampler,
        )
    };

    // NPCs and server
    let npcs = npcs::build(&device, terrain_extent);
    let npc_vb = npcs.vb;
    let npc_ib = npcs.ib;
    let npc_index_count = npcs.index_count;
    let npc_instances = npcs.instances;
    let npc_models = npcs.models;
    // legacy client AI removed; local server is not held by renderer

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

    // Rocks: enable full mesh + instances on Web too (we embed rock.glb bytes in roa_assets).
    let rocks_gpu = rocks::build_rocks(&device, &terrain_cpu, &slug, None)
        .context("build rocks (instances + mesh) for zone")?;
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

    // Zombie instances (replication-driven default)
    let (zombie_instances, zombie_instances_cpu, zombie_models, zombie_ids, zombie_count) =
        zombies::build_instances(&device, &terrain_cpu, zombie_joints);
    let total_z_mats = zombie_count as usize * zombie_joints as usize;
    let zombie_palettes_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("zombie-palettes"),
        size: ((total_z_mats.max(1)) * 64) as u64,
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
    // Server authority: do not spawn Death Knight from renderer. The visual
    // may still be present for demo scenes; dk_id remains None here.
    let dk_id = None;
    // Derive DK model position for dependent placements (e.g., sorceress)
    let dk_model_pos = if dk_count > 0 {
        let c = dk_models[0].to_cols_array();
        glam::vec3(c[12], c[13], c[14])
    } else {
        glam::Vec3::ZERO
    };
    let total_dk_mats = dk_count as usize * dk_joints as usize;
    let dk_palettes_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("deathknight-palettes"),
        size: ((total_dk_mats.max(1)) * 64) as u64,
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

    // Sorceress — gate under actor assets
    if !load_actor_assets { /* leave placeholders below */ }
    let sorc_cfg = data_runtime::configs::sorceress::load_default().unwrap_or_default();
    let (
        sorc_cpu,
        sorc_vb,
        sorc_ib,
        sorc_index_count,
        sorc_joints,
        sorc_mat_bg,
        _sorc_mat_buf,
        _sorc_tex_view,
        _sorc_sampler,
    ) = if !load_actor_assets {
        let dummy = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sorc-empty"),
            size: 4,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        let empty_cpu = crate::gfx::Renderer::empty_skinned_cpu();
        let mat = material::create_wizard_material(&device, &queue, &material_bgl, &empty_cpu);
        (
            empty_cpu,
            dummy.clone(),
            dummy,
            0u32,
            0u32,
            mat.bind_group,
            mat.uniform_buf,
            mat.texture_view,
            mat.sampler,
        )
    } else if let Some(model_rel) = sorc_cfg.model.as_deref() {
        if let Ok(sa) = super::super::sorceress::load_assets(&device, model_rel) {
            let mut cpu = sa.cpu;
            // Try to merge the universal animation library so Sorceress can use the same Walk/Idle clips as the PC.
            {
                use roa_assets::skinning::merge_gltf_animations;
                let lib_path =
                    super::super::asset_path("assets/anims/universal/AnimationLibrary.glb");
                if lib_path.exists() {
                    match merge_gltf_animations(&mut cpu, &lib_path) {
                        Ok(n) => {
                            log::info!(
                                target: "model_viewer",
                                "sorceress: merged GLTF animations from {:?} ({} clips)",
                                lib_path,
                                n
                            );
                        }
                        Err(e) => log::warn!(
                            "sorceress: failed to merge animation library at {:?}: {e:?}",
                            lib_path
                        ),
                    }
                }
            }
            let joints = cpu.joints_nodes.len() as u32;
            let mat = material::create_wizard_material(&device, &queue, &material_bgl, &cpu);
            (
                cpu,
                sa.vb,
                sa.ib,
                sa.index_count,
                joints,
                mat.bind_group,
                mat.uniform_buf,
                mat.texture_view,
                mat.sampler,
            )
        } else {
            log::warn!(
                "sorceress: failed to load {}; NPC will be disabled",
                model_rel
            );
            let dummy = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("sorc-empty"),
                size: 4,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            });
            let empty_cpu = crate::gfx::Renderer::empty_skinned_cpu();
            let mat = material::create_wizard_material(&device, &queue, &material_bgl, &empty_cpu);
            (
                crate::gfx::Renderer::empty_skinned_cpu(),
                dummy.clone(),
                dummy,
                0u32,
                0u32,
                mat.bind_group,
                mat.uniform_buf,
                mat.texture_view,
                mat.sampler,
            )
        }
    } else {
        log::info!("sorceress: no model configured; NPC disabled");
        let dummy = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sorc-empty"),
            size: 4,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        let empty_cpu = crate::gfx::Renderer::empty_skinned_cpu();
        let mat = material::create_wizard_material(&device, &queue, &material_bgl, &empty_cpu);
        (
            crate::gfx::Renderer::empty_skinned_cpu(),
            dummy.clone(),
            dummy,
            0u32,
            0u32,
            mat.bind_group,
            mat.uniform_buf,
            mat.texture_view,
            mat.sampler,
        )
    };
    // Position: behind DK along +Z
    let sorc_pos = {
        let mut p = dk_model_pos + glam::vec3(0.0, 0.0, 35.0);
        let (h, _n) = terrain::height_at(&terrain_cpu, p.x, p.z);
        p.y = h;
        p
    };
    let (sorc_instances, sorc_instances_cpu, sorc_models, sorc_count) = if sorc_index_count > 0 {
        super::super::sorceress::build_instance_at(&device, sorc_pos)
    } else {
        let b = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sorc-instances-empty"),
            size: 4,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        (b, Vec::new(), Vec::new(), 0u32)
    };
    let total_sorc_mats = sorc_count as usize * sorc_joints as usize;
    let sorc_palettes_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("sorceress-palettes"),
        size: ((total_sorc_mats.max(1)) * 64) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let sorc_palettes_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sorceress-palettes-bg"),
        layout: &palettes_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: sorc_palettes_buf.as_entire_binding(),
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

    // Parse CLI flags for destructibles
    #[cfg(feature = "vox_onepath_demo")]
    #[allow(unused_mut)]
    let mut dcfg = DestructibleConfig::from_args(std::env::args());
    // On web, allow enabling demo via query param ?vox=1 (unless a model is specified)
    #[cfg(all(feature = "vox_onepath_demo", target_arch = "wasm32"))]
    {
        if dcfg.voxel_model.is_none() && !dcfg.demo_grid {
            if let Some(win) = web_sys::window() {
                if let Ok(href) = win.location().href() {
                    if href.contains("vox=1") {
                        dcfg.demo_grid = true;
                    }
                }
            }
        }
    }

    // In the main scene, avoid seeding a demo voxel grid unless explicitly requested
    #[cfg(feature = "vox_onepath_demo")]
    if std::env::var("RA_VOX_DEMO")
        .map(|v| v != "1")
        .unwrap_or(true)
    {
        dcfg.demo_grid = false;
    }

    // Prepare neutral gray voxel model BG (before moving device into struct)
    let voxel_model_bg = {
        // Enable triplanar path for voxel meshes by setting _pad[0]=1, and
        // set tile frequency via _pad[1] derived from voxel size (meters).
        #[cfg(feature = "vox_onepath_demo")]
        let mut tiles_per_meter = (1.0f32 / (dcfg.voxel_size_m.0 as f32).max(1e-3)) * 0.25;
        #[cfg(feature = "vox_onepath_demo")]
        if let Some(t) = dcfg.vox_tiles_per_meter {
            tiles_per_meter = t.max(0.01);
        }
        #[cfg(not(feature = "vox_onepath_demo"))]
        let tiles_per_meter = 0.25f32;
        let mdl = crate::gfx::types::Model {
            model: glam::Mat4::IDENTITY.to_cols_array_2d(),
            color: [0.6, 0.6, 0.6],
            emissive: 0.02,
            _pad: [1.0, tiles_per_meter, 0.0, 0.0],
        };
        let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("voxel-model"),
            contents: bytemuck::bytes_of(&mdl),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("voxel-model-bg"),
            layout: &model_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buf.as_entire_binding(),
            }],
        })
    };

    // Debris instancing: unit cube VB/IB and instance buffer
    let (debris_vb, debris_ib, debris_index_count) = crate::gfx::mesh::create_cube(&device);
    #[cfg(feature = "vox_onepath_demo")]
    let debris_capacity = dcfg.max_debris as u32;
    #[cfg(not(feature = "vox_onepath_demo"))]
    let debris_capacity = 0u32;
    let debris_instances = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("debris-instances"),
        size: (debris_capacity as usize * std::mem::size_of::<crate::gfx::types::Instance>())
            as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let debris_model_bg = {
        let mdl = crate::gfx::types::Model {
            model: glam::Mat4::IDENTITY.to_cols_array_2d(),
            color: [0.55, 0.55, 0.55],
            emissive: 0.0,
            _pad: [0.0; 4],
        };
        let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("debris-model"),
            contents: bytemuck::bytes_of(&mdl),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("debris-model-bg"),
            layout: &model_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buf.as_entire_binding(),
            }],
        })
    };

    // Optionally create a voxel grid from a model (--voxel-model) or demo shell (--voxel-demo)
    #[cfg(feature = "vox_onepath_demo")]
    let voxel_grid = if let Some(ref model_path) = dcfg.voxel_model {
        // Load triangles and voxelize a surface shell, then flood-fill
        match roa_assets::gltf::load_gltf_mesh(std::path::Path::new(model_path)) {
            Ok(cpu) => {
                let mut min = glam::Vec3::splat(f32::INFINITY);
                let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
                for v in &cpu.vertices {
                    let p = glam::Vec3::from(v.pos);
                    min = min.min(p);
                    max = max.max(p);
                }
                if !min.is_finite() || !max.is_finite() {
                    log::warn!("voxel-model: invalid bounds; falling back to demo grid");
                    None
                } else {
                    // Clamp voxel density so total cells <= budget
                    let mut vm = (dcfg.voxel_size_m.0 as f32).max(1e-4);
                    // Compute origin; allow optional offset to position model near the player
                    let center_world = glam::DVec3::new(
                        0.5 * (min.x + max.x) as f64,
                        0.5 * (min.y + max.y) as f64,
                        0.5 * (min.z + max.z) as f64,
                    );
                    let origin = if let Some(off) = dcfg.vox_offset {
                        off - center_world
                    } else if dcfg.vox_sandbox {
                        // Default: place ruin ~8m in front of PC at terrain height
                        let off = glam::DVec3::new(
                            pc_initial_pos.x as f64,
                            pc_initial_pos.y as f64,
                            (pc_initial_pos.z + 8.0) as f64,
                        );
                        off - center_world
                    } else {
                        glam::DVec3::new(min.x as f64, min.y as f64, min.z as f64)
                    };
                    let size = max - min;
                    const MAX_VOXELS: u64 = 8_000_000;
                    let dims;
                    loop {
                        let dx = ((size.x / vm).ceil().max(1.0) as u32) + 2;
                        let dy = ((size.y / vm).ceil().max(1.0) as u32) + 2;
                        let dz = ((size.z / vm).ceil().max(1.0) as u32) + 2;
                        let total = dx as u64 * dy as u64 * dz as u64;
                        if total <= MAX_VOXELS {
                            dims = glam::UVec3::new(dx, dy, dz);
                            log::info!(
                                "[vox] dims={}x{}x{} (~{:.2}M)",
                                dx,
                                dy,
                                dz,
                                total as f32 / 1e6
                            );
                            break;
                        }
                        vm *= 1.25; // coarsen until under budget
                        log::warn!("[vox] too dense → increasing voxel size to {:.3} m", vm);
                    }
                    let voxel_m = core_units::Length::meters(vm as f64);
                    let meta = VoxelProxyMeta {
                        object_id: voxel_proxy::GlobalId(1),
                        origin_m: origin,
                        voxel_m,
                        dims,
                        chunk: dcfg.chunk.min(dims.max(glam::UVec3::splat(1))),
                        material: dcfg.material,
                    };
                    let mut surf = vec![0u8; (dims.x * dims.y * dims.z) as usize];
                    let idx = |x: u32, y: u32, z: u32| -> usize {
                        (x + y * dims.x + z * dims.x * dims.y) as usize
                    };
                    // SAT triangle–AABB test helper (triangle and box expressed in voxel coords)
                    #[inline]
                    fn tri_intersects_box(
                        a: glam::Vec3,
                        b: glam::Vec3,
                        c: glam::Vec3,
                        center: glam::Vec3,
                        half: f32,
                    ) -> bool {
                        let v0 = a - center;
                        let v1 = b - center;
                        let v2 = c - center;
                        let e0 = v1 - v0;
                        let e1 = v2 - v1;
                        let e2 = v0 - v2;
                        let h = glam::Vec3::splat(half);
                        let axes = [
                            glam::Vec3::new(0.0, -e0.z, e0.y),
                            glam::Vec3::new(0.0, -e1.z, e1.y),
                            glam::Vec3::new(0.0, -e2.z, e2.y),
                            glam::Vec3::new(e0.z, 0.0, -e0.x),
                            glam::Vec3::new(e1.z, 0.0, -e1.x),
                            glam::Vec3::new(e2.z, 0.0, -e2.x),
                            glam::Vec3::new(-e0.y, e0.x, 0.0),
                            glam::Vec3::new(-e1.y, e1.x, 0.0),
                            glam::Vec3::new(-e2.y, e2.x, 0.0),
                        ];
                        for ax in axes.iter() {
                            if ax.length_squared() > 1e-12 {
                                let p0 = v0.dot(*ax);
                                let p1 = v1.dot(*ax);
                                let p2 = v2.dot(*ax);
                                let r = h.x * ax.x.abs() + h.y * ax.y.abs() + h.z * ax.z.abs();
                                let minp = p0.min(p1.min(p2));
                                let maxp = p0.max(p1.max(p2));
                                if minp > r || maxp < -r {
                                    return false;
                                }
                            }
                        }
                        // Box axes
                        let minv = glam::Vec3::new(
                            v0.x.min(v1.x.min(v2.x)),
                            v0.y.min(v1.y.min(v2.y)),
                            v0.z.min(v1.z.min(v2.z)),
                        );
                        let maxv = glam::Vec3::new(
                            v0.x.max(v1.x.max(v2.x)),
                            v0.y.max(v1.y.max(v2.y)),
                            v0.z.max(v1.z.max(v2.z)),
                        );
                        if minv.x > h.x || maxv.x < -h.x {
                            return false;
                        }
                        if minv.y > h.y || maxv.y < -h.y {
                            return false;
                        }
                        if minv.z > h.z || maxv.z < -h.z {
                            return false;
                        }
                        // Plane-box overlap
                        let n = e0.cross(e1);
                        if n.length_squared() > 1e-12 {
                            let d = v0.dot(n);
                            let r = h.x * n.x.abs() + h.y * n.y.abs() + h.z * n.z.abs();
                            if d.abs() > r {
                                return false;
                            }
                        }
                        true
                    }

                    let verts = &cpu.vertices;
                    let inds = &cpu.indices;
                    let inv_vm = 1.0 / vm;
                    for tri in inds.chunks_exact(3) {
                        // Convert to voxel space
                        let a_w = glam::Vec3::from(verts[tri[0] as usize].pos);
                        let b_w = glam::Vec3::from(verts[tri[1] as usize].pos);
                        let c_w = glam::Vec3::from(verts[tri[2] as usize].pos);
                        let av = ((a_w.as_dvec3() - origin).as_vec3()) * inv_vm;
                        let bv = ((b_w.as_dvec3() - origin).as_vec3()) * inv_vm;
                        let cv = ((c_w.as_dvec3() - origin).as_vec3()) * inv_vm;
                        // Voxel AABB of triangle (with 1-cell pad)
                        let minv = av.min(bv.min(cv)).floor() - glam::Vec3::ONE;
                        let maxv = av.max(bv.max(cv)).ceil() + glam::Vec3::ONE;
                        let xi0 = minv.x.max(0.0) as u32;
                        let yi0 = minv.y.max(0.0) as u32;
                        let zi0 = minv.z.max(0.0) as u32;
                        let xi1 = maxv.x.min((dims.x - 1) as f32) as u32;
                        let yi1 = maxv.y.min((dims.y - 1) as f32) as u32;
                        let zi1 = maxv.z.min((dims.z - 1) as f32) as u32;
                        for z in zi0..=zi1 {
                            for y in yi0..=yi1 {
                                for x in xi0..=xi1 {
                                    let center = glam::Vec3::new(
                                        x as f32 + 0.5,
                                        y as f32 + 0.5,
                                        z as f32 + 0.5,
                                    );
                                    if tri_intersects_box(av, bv, cv, center, 0.5) {
                                        surf[idx(x, y, z)] = 1;
                                    }
                                }
                            }
                        }
                    }
                    let grid = voxelize_surface_fill(meta.clone(), &surf, dcfg.close_surfaces);
                    if grid.solid_count() == 0 {
                        log::warn!("[vox] flood fill empty; retrying with --close-surfaces (auto)");
                        let grid2 = voxelize_surface_fill(meta.clone(), &surf, true);
                        if grid2.solid_count() == 0 {
                            log::warn!(
                                "[vox] model voxelization produced no solids; falling back to demo grid"
                            );
                            None
                        } else {
                            Some(grid2)
                        }
                    } else {
                        Some(grid)
                    }
                }
            }
            Err(e) => {
                log::warn!(
                    "voxel-model load failed ({}): {:?}. Falling back to demo",
                    model_path,
                    e
                );
                None
            }
        }
    } else if dcfg.demo_grid {
        let chunk = dcfg.chunk;
        let dims = UVec3::new(chunk.x * 2, chunk.y, chunk.z * 2);
        let meta = VoxelProxyMeta {
            object_id: voxel_proxy::GlobalId(1),
            origin_m: DVec3::ZERO,
            voxel_m: dcfg.voxel_size_m,
            dims,
            chunk,
            material: dcfg.material,
        };
        // Shell surface marks for a centered box
        let mut surf = vec![0u8; (dims.x * dims.y * dims.z) as usize];
        let idx =
            |x: u32, y: u32, z: u32| -> usize { (x + y * dims.x + z * dims.x * dims.y) as usize };
        let (x0, x1) = (2, dims.x.saturating_sub(3));
        let (y0, y1) = (2, dims.y.saturating_sub(3));
        let (z0, z1) = (2, dims.z.saturating_sub(3));
        for z in z0..=z1 {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    if x == x0 || x == x1 || y == y0 || y == y1 || z == z0 || z == z1 {
                        surf[idx(x, y, z)] = 1;
                    }
                }
            }
        }
        let grid = voxelize_surface_fill(meta, &surf, dcfg.close_surfaces);
        Some(grid)
    } else {
        None
    };
    #[cfg(not(feature = "vox_onepath_demo"))]
    let voxel_grid: Option<VoxelGrid> = None;

    // Build the renderer struct first so we can optionally seed the voxel chunk queue
    // Load controller input/camera config (optional file)
    let icfg = data_runtime::configs::input_camera::load_default().unwrap_or_default();
    let ml_cfg = client_core::systems::mouselook::MouselookConfig {
        sensitivity_deg_per_count: icfg.sensitivity_deg_per_count.unwrap_or(0.15),
        invert_y: icfg.invert_y.unwrap_or(false),
        min_pitch_deg: icfg.min_pitch_deg.unwrap_or(-80.0),
        max_pitch_deg: icfg.max_pitch_deg.unwrap_or(80.0),
    };
    let alt_hold = icfg.alt_hold.unwrap_or(false);

    // Load explicit PC animation name mapping (optional)
    let pc_anim_cfg = data_runtime::configs::pc_animations::load_default().unwrap_or_default();

    let mut renderer = crate::gfx::Renderer {
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
        inst_tex_pipeline,
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
        material_bgl: material_bgl.clone(),
        default_material_bg,
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
        // With direct-present on web, disable post passes that rely on
        // offscreen SceneColor/SceneRead for now to match desktop visuals.
        enable_post_ao: false,
        enable_ssgi: false,
        enable_ssr: false,
        // Disable bloom on wasm to reduce pipeline churn while stabilizing
        #[cfg(target_arch = "wasm32")]
        enable_bloom: false,
        #[cfg(not(target_arch = "wasm32"))]
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
        pc_vb,
        pc_ib,
        pc_index_count,
        pc_index_format: wgpu::IndexFormat::Uint16,
        pc_instances,
        dk_vb,
        dk_ib,
        dk_index_count,
        sorc_vb,
        sorc_ib,
        sorc_index_count,
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
        trees_groups: Vec::new(),
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
        sorc_instances,
        sorc_count,
        sorc_instances_cpu,
        zombie_instances,
        zombie_count: zombie_instances_cpu.len() as u32,
        zombie_instances_cpu,
        ruins_instances: scene_build.ruins_instances,
        ruins_count: scene_build.ruins_count,
        ruins_instances_cpu: scene_build.ruins_instances_cpu,
        fx_instances,
        _fx_capacity: fx_capacity,
        fx_count,
        _fx_model_bg,
        quad_vb,
        palettes_buf,
        palettes_bg,
        pc_palettes_buf,
        pc_palettes_bg,
        joints_per_wizard: scene_build.joints_per_wizard,
        wizard_models: wizard_models.clone(),
        wizard_slot_map: std::collections::HashMap::new(),
        wizard_slot_id: vec![None; wizard_models.len()],
        wizard_free_slots: (0..wizard_models.len()).collect(),
        wizard_instances_cpu,
        wizard_pipeline,
        wizard_pipeline_debug,
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
        pc_debug_warned_not_ready: false,
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
        npc_hp_overlay: std::collections::HashMap::new(),
        wiz_hp_overlay: std::collections::HashMap::new(),
        zombie_melee_cd: std::collections::HashMap::new(),
        fire_bolt,
        nameplates,
        nameplates_npc,
        bars,
        damage,
        hud,
        hud_model: Default::default(),
        controller_state: Default::default(),
        // Do not auto-lock the pointer; WoW-style mouselook occurs while RMB is held.
        pointer_lock_request: None,
        pointer_locked: false,
        controller_ml_cfg: ml_cfg,
        controller_alt_hold: alt_hold,
        // Destructible defaults; leave grid None until provided by a loader/demo
        #[cfg(feature = "vox_onepath_demo")]
        destruct_cfg: dcfg,
        voxel_grid: voxel_grid.clone(),
        #[cfg(feature = "vox_onepath_demo")]
        chunk_queue: server_core::destructible::queue::ChunkQueue::new(),
        chunk_colliders: Vec::new(),
        vox_last_chunks: 0,
        vox_queue_len: 0,
        vox_debris_last: 0,
        vox_remesh_ms_last: 0.0,
        vox_collider_ms_last: 0.0,
        vox_skipped_last: 0,
        vox_onepath_ui: None,
        voxel_meshes: std::collections::HashMap::new(),
        voxel_hashes: std::collections::HashMap::new(),
        destr_voxels: std::collections::HashMap::new(),
        destruct_meshes_cpu: scene_build.destruct_meshes_cpu,
        destruct_instances: scene_build.destruct_instances,
        repl_rx: None,
        repl_buf: Default::default(),
        boss_status_next_emit: 0.0,
        voxel_model_bg,
        debris_vb,
        debris_ib,
        debris_index_count,
        debris_instances,
        debris_capacity,
        debris_count: 0,
        debris: Vec::new(),
        debris_model_bg,
        voxel_grid_initial: voxel_grid,
        recent_impacts: Vec::new(),
        // demo_hint_until removed (no initial hint overlay)
        impact_id: 0,
        last_repl_projectiles: std::collections::HashMap::new(),
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
        pc_prev_airborne: false,
        pc_jump_start_time: None,
        pc_land_start_time: None,
        pc_cast_shoot_time: None,
        pc_cast_end_time: None,
        cam_yaw_prev: 0.0,
        cam_yaw_changed_at: 0.0,
        cam_face_reset_at: 0.0,
        cam_prev_panic: false,
        shift_down: false,
        pc_cast_time,
        magic_missile_cast_time: mm_cast_time,
        magic_missile_cd_dur: mm_cd_dur,
        fireball_cast_time: fb_cast_time,
        fireball_cd_dur: fb_cd_dur,
        pc_cast_fired: false,
        firebolt_cd_dur,
        rmb_down: false,
        lmb_down: false,
        a_down: false,
        d_down: false,
        q_down: false,
        e_down: false,
        prev_rmb_down: false,
        last_cursor_pos: None,
        screenshot_start: None,
        #[cfg(any())]
        server,
        wizard_hp: vec![100; scene_build.wizard_count as usize],
        wizard_hp_max: 100,
        cmd_tx: None,
        pc_alive: true,
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
        dk_id,
        pc_joints,
        pc_cpu,
        pc_mat_bg,
        pc_prev_pos: pc_initial_pos,
        pc_rep_id_last: None,
        sorc_palettes_buf,
        sorc_palettes_bg,
        sorc_joints,
        sorc_models: sorc_models.clone(),
        sorc_cpu,
        sorc_time_offset: (0..sorc_count as usize).map(|_| 0.0f32).collect(),
        sorc_prev_pos: sorc_pos,
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
        sorc_mat_bg,
        _sorc_mat_buf,
        _sorc_tex_view,
        _sorc_sampler,
        pc_anim_cfg,
        pc_anim_missing_warned: Default::default(),
        zone_batches: None,
        // Picker overlay state
        picker_items: Vec::new(),
        picker_selected: 0,
        picker_chosen_slug: None,
        zone_policy: Default::default(),
    };

    // Apply default input profile from config if provided
    let prof = crate::gfx::renderer::controls::parse_profile_name(icfg.profile.as_deref());
    renderer.controller_state.profile = prof;
    // Start in Cursor mode (no mouselook) to match WoW default
    renderer.controller_state.mode = ecs_core::components::ControllerMode::Cursor;

    // If a demo voxel grid was created, enqueue all chunks once so it renders immediately
    #[cfg(feature = "vox_onepath_demo")]
    if let Some(ref grid) = renderer.voxel_grid {
        let dims = grid.dims();
        let csz = grid.meta().chunk;
        let nx = dims.x.div_ceil(csz.x);
        let ny = dims.y.div_ceil(csz.y);
        let nz = dims.z.div_ceil(csz.z);
        for cz in 0..nz {
            for cy in 0..ny {
                for cx in 0..nx {
                    renderer
                        .chunk_queue
                        .enqueue_many([glam::UVec3::new(cx, cy, cz)]);
                }
            }
        }
        renderer.impact_id = 0; // reset deterministic seeding for a fresh grid
        renderer.vox_queue_len = renderer.chunk_queue.len();
    }

    // Optionally apply a replay file of impacts to the current grid (native only)
    #[cfg(all(feature = "vox_onepath_demo", not(target_arch = "wasm32")))]
    if let (Some(grid), Some(ref path)) = (
        &mut renderer.voxel_grid,
        renderer.destruct_cfg.replay.as_ref(),
    ) && let Ok(txt) = std::fs::read_to_string(path)
    {
        for line in txt.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                let center = val
                    .get("center")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let radius = val.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.0);
                if center.len() == 3 && radius > 0.0 {
                    let cx = center[0].as_f64().unwrap_or(0.0);
                    let cy = center[1].as_f64().unwrap_or(0.0);
                    let cz = center[2].as_f64().unwrap_or(0.0);
                    let impact = glam::DVec3::new(cx, cy, cz);
                    let _out = server_core::destructible::carve_and_spawn_debris(
                        grid,
                        impact,
                        core_units::Length::meters(radius),
                        renderer.destruct_cfg.seed,
                        0,
                        renderer.destruct_cfg.max_debris,
                    );
                    // Enqueue all dirty chunks
                    let enq = grid.pop_dirty_chunks(usize::MAX);
                    renderer.chunk_queue.enqueue_many(enq);
                }
            }
        }
        renderer.vox_queue_len = renderer.chunk_queue.len();
    }

    // Vox sandbox: remove mobs/boss for a clean destructible demo
    #[cfg(feature = "vox_onepath_demo")]
    if renderer.destruct_cfg.vox_sandbox {
        #[cfg(any())]
        {
            renderer.server.npcs.clear();
        }
        renderer.zombie_count = 0;
        renderer.dk_count = 0;
        renderer.dk_id = None;
    }

    // Note: generic destructibles should be provided by SceneBuild and replicated
    // by the server. The local chunk-mesh delta loop is already wired via
    // `set_replication_rx`; registry replication will be integrated next.

    Ok(renderer)
}
