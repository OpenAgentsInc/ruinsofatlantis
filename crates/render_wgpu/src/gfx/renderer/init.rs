//! Renderer initialization (`new`) extracted from gfx/mod.rs

use anyhow::Context;
use winit::window::Window;
use wgpu::{rwh::HasDisplayHandle, rwh::HasWindowHandle, SurfaceTargetUnsafe, util::DeviceExt};

use crate::gfx::{self, pipeline, ui, util, types::Globals};

use super::Renderer;

impl Renderer {
    /// Create a renderer bound to a window surface.
    pub async fn new(window: &Window) -> anyhow::Result<Self> {
        fn backend_from_env() -> Option<wgpu::Backends> {
            match std::env::var("RA_BACKEND").ok().as_deref() {
                Some("vulkan" | "VULKAN" | "vk") => Some(wgpu::Backends::VULKAN),
                Some("gl" | "GL" | "opengl") => Some(wgpu::Backends::GL),
                Some("primary" | "PRIMARY" | "all") => Some(wgpu::Backends::PRIMARY),
                _ => None,
            }
        }
        let candidates: &[wgpu::Backends] = if let Some(b) = backend_from_env() {
            if b == wgpu::Backends::PRIMARY { &[wgpu::Backends::PRIMARY] } else { &[b, wgpu::Backends::PRIMARY] }
        } else if cfg!(target_os = "linux") {
            &[wgpu::Backends::VULKAN, wgpu::Backends::GL, wgpu::Backends::PRIMARY]
        } else {
            &[wgpu::Backends::PRIMARY, wgpu::Backends::GL]
        };

        // Create a surface per candidate instance and try to get an adapter
        let raw_display = window.display_handle()?.as_raw();
        let raw_window = window.window_handle()?.as_raw();
        let (_instance, surface, adapter) = {
            let mut picked: Option<(wgpu::Instance, wgpu::Surface<'static>, wgpu::Adapter)> = None;
            for &bmask in candidates {
                let inst = wgpu::Instance::new(&wgpu::InstanceDescriptor { backends: bmask, flags: wgpu::InstanceFlags::empty(), ..Default::default() });
                let surf = unsafe { inst.create_surface_unsafe(SurfaceTargetUnsafe::RawHandle { raw_display_handle: raw_display, raw_window_handle: raw_window }) }
                    .context("create wgpu surface (unsafe)")?;
                match inst.request_adapter(&wgpu::RequestAdapterOptions { compatible_surface: Some(&surf), power_preference: wgpu::PowerPreference::HighPerformance, force_fallback_adapter: false }).await {
                    Ok(a) => { picked = Some((inst, surf, a)); break; }
                    Err(_) => {}
                }
            }
            picked.ok_or_else(|| anyhow::anyhow!("no suitable GPU adapter across backends {:?}", candidates))?
        };

        let mut req_features = wgpu::Features::empty();
        if adapter.features().contains(wgpu::Features::POLYGON_MODE_LINE) {
            req_features |= wgpu::Features::POLYGON_MODE_LINE;
        }
        let info = adapter.get_info();
        log::info!("Adapter: {:?} ({:?})", info.name, info.backend);
        let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("wgpu-device"),
            required_features: req_features,
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        }).await.context("request device")?;

        // Log validation instead of panicking
        device.on_uncaptured_error(Box::new(|e| { log::error!("wgpu uncaptured error: {:?}", e); }));

        // Surface configuration
        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);
        let present_mode = wgpu::PresentMode::Fifo;
        let alpha_mode = caps.alpha_modes[0];
        let max_dim = device.limits().max_texture_dimension_2d.clamp(1, 2048);
        let (w, h) = gfx::util::scale_to_max((size.width, size.height), max_dim);
        if (w, h) != (size.width, size.height) {
            log::warn!("Clamping surface from {}x{} to {}x{} (max_dim={})", size.width, size.height, w, h, max_dim);
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
        // Offscreen SceneColor (HDR) + SceneRead
        let scene_color = device.create_texture(&wgpu::TextureDescriptor { label: Some("scene-color"), size: wgpu::Extent3d { width: config.width, height: config.height, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba16Float, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC, view_formats: &[] });
        let scene_view = scene_color.create_view(&wgpu::TextureViewDescriptor::default());
        let scene_read = device.create_texture(&wgpu::TextureDescriptor { label: Some("scene-read"), size: wgpu::Extent3d { width: config.width, height: config.height, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba16Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT, view_formats: &[] });
        let scene_read_view = scene_read.create_view(&wgpu::TextureViewDescriptor::default());

        // Lighting M1 resources
        let gbuffer = Some(gfx::gbuffer::GBuffer::create(&device, config.width, config.height));
        let hiz = Some(gfx::hiz::HiZPyramid::create(&device, config.width, config.height));

        // Pipelines/BGLs
        let shader = pipeline::create_shader(&device);
        let (globals_bgl, model_bgl) = pipeline::create_bind_group_layouts(&device);
        let palettes_bgl = pipeline::create_palettes_bgl(&device);
        let material_bgl = pipeline::create_material_bgl(&device);
        let offscreen_fmt = wgpu::TextureFormat::Rgba16Float;
        let direct_present = std::env::var("RA_DIRECT_PRESENT").map(|v| v != "0").unwrap_or(true);
        let draw_fmt = if direct_present { config.format } else { offscreen_fmt };
        let (pipeline, inst_pipeline, wire_pipeline) = pipeline::create_pipelines(&device, &shader, &globals_bgl, &model_bgl, draw_fmt);
        let sky_bgl = pipeline::create_sky_bgl(&device);
        let sky_pipeline = pipeline::create_sky_pipeline(&device, &globals_bgl, &sky_bgl, draw_fmt);
        let present_bgl = pipeline::create_present_bgl(&device);
        let present_pipeline = pipeline::create_present_pipeline(&device, &globals_bgl, &present_bgl, config.format);
        let blit_scene_read_pipeline = pipeline::create_blit_pipeline(&device, &present_bgl, wgpu::TextureFormat::Rgba16Float);
        let bloom_bgl = pipeline::create_bloom_bgl(&device);
        let bloom_pipeline = pipeline::create_bloom_pipeline(&device, &bloom_bgl, wgpu::TextureFormat::Rgba16Float);
        let post_ao_bgl = pipeline::create_post_ao_bgl(&device);
        let post_ao_pipeline = pipeline::create_post_ao_pipeline(&device, &globals_bgl, &post_ao_bgl, offscreen_fmt);
        let (ssgi_globals_bgl, ssgi_depth_bgl, ssgi_scene_bgl) = pipeline::create_ssgi_bgl(&device);
        let (ssr_depth_bgl, ssr_scene_bgl) = pipeline::create_ssr_bgl(&device);
        let ssgi_pipeline = pipeline::create_ssgi_pipeline(&device, &ssgi_globals_bgl, &ssgi_depth_bgl, &ssgi_scene_bgl, wgpu::TextureFormat::Rgba16Float);
        let ssr_pipeline = pipeline::create_ssr_pipeline(&device, &ssr_depth_bgl, &ssr_scene_bgl, wgpu::TextureFormat::Rgba16Float);
        let (wizard_pipeline, _wizard_wire_pipeline_unused) = pipeline::create_wizard_pipelines(&device, &shader, &globals_bgl, &model_bgl, &palettes_bgl, &material_bgl, draw_fmt);
        let particle_pipeline = pipeline::create_particle_pipeline(&device, &shader, &globals_bgl, draw_fmt);

        // UI
        let nameplates = ui::Nameplates::new(&device, draw_fmt)?;
        let nameplates_npc = ui::Nameplates::new(&device, draw_fmt)?;
        let mut bars = ui::HealthBars::new(&device, draw_fmt)?;
        let hud = ui::Hud::new(&device, draw_fmt)?;
        let damage = ui::DamageFloaters::new(&device, draw_fmt)?;

        // Globals buffers and bind groups
        let globals_init = Globals { view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(), cam_right_time: [1.0, 0.0, 0.0, 0.0], cam_up_pad: [0.0, 1.0, 0.0, (60f32.to_radians() * 0.5).tan()], sun_dir_time: [0.0, 1.0, 0.0, 0.0], sh_coeffs: [[0.0, 0.0, 0.0, 0.0]; 9], fog_params: [0.0, 0.0, 0.0, 0.0], clip_params: [0.1, 1000.0, 1.0, 0.0] };
        let globals_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("globals"), contents: bytemuck::bytes_of(&globals_init), usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST });
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct LightsRaw { count: u32, _pad: [f32; 3], pos_radius: [[f32; 4]; 16], color: [[f32; 4]; 16], _tail_pad: [f32; 4] }
        let lights_init = LightsRaw { count: 0, _pad: [0.0; 3], pos_radius: [[0.0; 4]; 16], color: [[0.0; 4]; 16], _tail_pad: [0.0; 4] };
        let lights_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("lights-ubo"), contents: bytemuck::bytes_of(&lights_init), usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST });
        let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("globals-bg"), layout: &globals_bgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: lights_buf.as_entire_binding() }] });

        // Sky uniforms from zone manifest
        let zone: data_runtime::zone::ZoneManifest = data_runtime::zone::load_zone_manifest("wizard_woods").context("load zone manifest: wizard_woods")?;
        log::info!("Zone '{}' (id={}, plane={:?})", zone.display_name, zone.zone_id, zone.plane);
        let mut sky_state = gfx::sky::SkyStateCPU::new();
        if let Some(w) = zone.weather { sky_state.weather = gfx::sky::Weather { turbidity: w.turbidity, ground_albedo: w.ground_albedo }; sky_state.recompute(); }
        if let Some(frac) = zone.start_time_frac { sky_state.day_frac = frac.rem_euclid(1.0); sky_state.recompute(); }
        if let Some(pause) = zone.start_paused { sky_state.paused = pause; }
        if let Some(scale) = zone.start_time_scale { sky_state.time_scale = scale.clamp(0.01, 1000.0); }
        let sky_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("sky-uniform"), contents: bytemuck::bytes_of(&sky_state.sky_uniform), usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST });
        let sky_bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("sky-bg"), layout: &sky_bgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: sky_buf.as_entire_binding() }] });

        // Post bind groups & samplers
        let post_sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("post-ao-sampler"), address_mode_u: wgpu::AddressMode::ClampToEdge, address_mode_v: wgpu::AddressMode::ClampToEdge, address_mode_w: wgpu::AddressMode::ClampToEdge, mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, mipmap_filter: wgpu::FilterMode::Nearest, ..Default::default() });
        let post_ao_bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("post-ao-bg"), layout: &post_ao_bgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&depth) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&post_sampler) }] });
        let point_sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("point-sampler"), address_mode_u: wgpu::AddressMode::ClampToEdge, address_mode_v: wgpu::AddressMode::ClampToEdge, address_mode_w: wgpu::AddressMode::ClampToEdge, mag_filter: wgpu::FilterMode::Nearest, min_filter: wgpu::FilterMode::Nearest, mipmap_filter: wgpu::FilterMode::Nearest, ..Default::default() });
        let ssgi_globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("ssgi-globals-bg"), layout: &ssgi_globals_bgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() }] });
        let ssgi_depth_bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("ssgi-depth-bg"), layout: &ssgi_depth_bgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&depth) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&post_sampler) }] });
        let ssgi_scene_bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("ssgi-scene-bg"), layout: &ssgi_scene_bgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&scene_read_view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&post_sampler) }] });
        let present_bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("present-bg"), layout: &present_bgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&scene_view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&post_sampler) }, wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&depth) }, wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&point_sampler) }] });
        let bloom_bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("bloom-bg"), layout: &bloom_bgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&scene_read_view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&post_sampler) }] });

        // Terrain + NPCs + foliage + ruins
        let terrain_build = gfx::terrain::build(&device);
        let terrain_vb = terrain_build.vb;
        let terrain_ib = terrain_build.ib;
        let terrain_index_count = terrain_build.index_count;
        let terrain_extent = (config.width.min(config.height)) as f32 * 0.5;
        let ruins_build = gfx::ruins::build_ruins(&device).context("build ruins mesh")?;
        let ruins_vb = ruins_build.vb;
        let ruins_ib = ruins_build.ib;
        let ruins_index_count = ruins_build.index_count;
        let ruins_base_offset = ruins_build.base_offset;
        let ruins_radius = ruins_build.radius;

        // Wizard model + zombie
        let skinned_cpu = ra_assets::skinning::load_gltf_skinned(&gfx::asset_path("assets/models/wizard.gltf")).context("load skinned wizard.gltf")?;
        let zombie_model_path = "assets/models/zombie.glb";
        let mut zombie_cpu = ra_assets::skinning::load_gltf_skinned(&gfx::asset_path(zombie_model_path)).with_context(|| format!("load skinned {}", zombie_model_path))?;
        for (_alias, file) in [("Idle", "idle.glb"), ("Walk", "walk.glb"), ("Run", "run.glb"), ("Attack", "attack.glb")] {
            let p = gfx::asset_path(&format!("assets/models/zombie_clips/{}", file));
            if p.exists() { let _ = ra_assets::skinning::merge_gltf_animations(&mut zombie_cpu, &p); }
        }

        // Scene assembly: wizards + ruins + terrain clamp
        let scene_build = gfx::scene::build_demo_scene(&device, &skinned_cpu, terrain_extent, None, ruins_base_offset, ruins_radius);

        // Wizard/Zombie GPU buffers
        let wiz_vertices: Vec<gfx::types::VertexSkinned> = skinned_cpu.vertices.iter().map(|v| gfx::types::VertexSkinned { pos: v.pos, uv: v.uv, normal: v.normal, joints: v.joints, weights: v.weights }).collect();
        let wizard_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("wizard-vb"), contents: bytemuck::cast_slice(&wiz_vertices), usage: wgpu::BufferUsages::VERTEX });
        let wizard_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("wizard-ib"), contents: bytemuck::cast_slice(&skinned_cpu.indices), usage: wgpu::BufferUsages::INDEX });
        let wizard_index_count = skinned_cpu.indices.len() as u32;
        let zombie_vertices: Vec<gfx::types::VertexSkinned> = zombie_cpu.vertices.iter().map(|v| gfx::types::VertexSkinned { pos: v.pos, uv: v.uv, normal: v.normal, joints: v.joints, weights: v.weights }).collect();
        let zombie_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("zombie-vb"), contents: bytemuck::cast_slice(&zombie_vertices), usage: wgpu::BufferUsages::VERTEX });
        let zombie_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("zombie-ib"), contents: bytemuck::cast_slice(&zombie_cpu.indices), usage: wgpu::BufferUsages::INDEX });
        let zombie_index_count = zombie_cpu.indices.len() as u32;

        // NPCs/foliage/rocks
        let npcs_build = gfx::npcs::build(&device, terrain_extent);
        let rocks_build = gfx::rocks::build(&device, terrain_extent);
        let trees_build = gfx::foliage::build(&device, terrain_extent);

        // FX buffers
        let fx_capacity = 8192u32;
        let fx_instances = device.create_buffer(&wgpu::BufferDescriptor { label: Some("fx-instances"), size: (fx_capacity as usize * std::mem::size_of::<gfx::types::ParticleInstance>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let quad_vb = gfx::fx::create_quad_vb(&device);

        // Skinning palettes
        let joints_per_wizard = skinned_cpu.joints as u32;
        let palettes_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("palettes"), size: (scene_build.wizard_count as usize * joints_per_wizard as usize * 64) as u64, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let palettes_bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("palettes-bg"), layout: &palettes_bgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: palettes_buf.as_entire_binding() }] });
        let zombie_joints = zombie_cpu.joints as u32;
        let zombie_palettes_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("zombie-palettes"), size: (zombie_joints as usize * 64 * npcs_build.count as usize) as u64, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let zombie_palettes_bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("zombie-palettes-bg"), layout: &palettes_bgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: zombie_palettes_buf.as_entire_binding() }] });

        // Materials
        let wizard_mat = gfx::material::wizard_material(&device, &queue)?;
        let wizard_mat_bg = wizard_mat.bg;
        let _wizard_mat_buf = wizard_mat.buf;
        let _wizard_tex_view = wizard_mat.tex_view;
        let _wizard_sampler = wizard_mat.sampler;
        let zombie_mat = gfx::material::zombie_material(&device, &queue)?;
        let zombie_mat_bg = zombie_mat.bg;
        let _zombie_mat_buf = zombie_mat.buf;
        let _zombie_tex_view = zombie_mat.tex_view;
        let _zombie_sampler = zombie_mat.sampler;

        // Health bars buffers
        bars.prepare_instance_buffers(&device, scene_build.wizard_count);

        let hud_model = ux_hud::HudModel::default();
        let fire_bolt = data_runtime::loader::load_spell("fire_bolt").ok();

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size,
            max_dim,
            depth,
            scene_color,
            scene_view,
            scene_read,
            scene_read_view,
            gbuffer,
            hiz,
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
            present_bgl,
            post_ao_bgl,
            ssgi_globals_bgl,
            ssgi_depth_bgl,
            ssgi_scene_bgl,
            ssr_depth_bgl,
            ssr_scene_bgl,
            palettes_bgl,
            globals_bg,
            post_ao_bg,
            ssgi_globals_bg,
            ssgi_depth_bg,
            ssgi_scene_bg,
            ssr_depth_bg: wgpu::BindGroup::dummy(&device),
            ssr_scene_bg: wgpu::BindGroup::dummy(&device),
            _post_sampler: post_sampler,
            point_sampler,
            sky_bg,
            terrain_model_bg: wgpu::BindGroup::dummy(&device),
            shard_model_bg: wgpu::BindGroup::dummy(&device),
            present_bg,
            enable_post_ao: true,
            enable_ssgi: false,
            enable_ssr: false,
            enable_bloom: true,
            static_index: None,
            frame_counter: 0,
            draw_calls: 0,
            globals_buf,
            sky_buf,
            _plane_model_buf: device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("plane-model"), contents: bytemuck::bytes_of(&gfx::types::Model::identity()), usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST }),
            shard_model_buf: device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("shard-model"), contents: bytemuck::bytes_of(&gfx::types::Model { model: glam::Mat4::IDENTITY.to_cols_array_2d(), color: [0.85, 0.15, 0.15], emissive: 0.05, _pad: [0.0; 4] }), usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST }),
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
            npc_vb: npcs_build.vb,
            npc_ib: npcs_build.ib,
            npc_index_count: npcs_build.index_count,
            npc_instances: npcs_build.instances,
            npc_count: npcs_build.count,
            npc_instances_cpu: npcs_build.instances_cpu,
            npc_models: npcs_build.models,
            trees_instances: trees_build.instances,
            trees_count: trees_build.count,
            trees_vb: trees_build.vb,
            trees_ib: trees_build.ib,
            trees_index_count: trees_build.index_count,
            rocks_instances: rocks_build.instances,
            rocks_count: rocks_build.count,
            rocks_vb: rocks_build.vb,
            rocks_ib: rocks_build.ib,
            rocks_index_count: rocks_build.index_count,
            wizard_instances: scene_build.wizard_instances,
            wizard_count: scene_build.wizard_count,
            zombie_instances: wgpu::Buffer::dummy(&device),
            zombie_count: 0,
            zombie_instances_cpu: Vec::new(),
            ruins_instances: ruins_build.instances,
            ruins_count: ruins_build.count,
            fx_instances,
            _fx_capacity: fx_capacity,
            fx_count: 0,
            _fx_model_bg: wgpu::BindGroup::dummy(&device),
            quad_vb,
            palettes_buf,
            palettes_bg,
            joints_per_wizard,
            wizard_models: scene_build.wizard_models,
            wizard_instances_cpu: scene_build.wizard_instances_cpu,
            zombie_palettes_buf,
            zombie_palettes_bg,
            zombie_joints,
            zombie_models: Vec::new(),
            zombie_cpu,
            zombie_time_offset: Vec::new(),
            zombie_ids: Vec::new(),
            zombie_prev_pos: Vec::new(),
            zombie_forward_offsets: Vec::new(),
            wizard_pipeline,
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
            terrain_cpu: terrain_build.cpu,
            start: std::time::Instant::now(),
            last_time: 0.0,
            wizard_anim_index: scene_build.wizard_anim_index,
            wizard_time_offset: scene_build.wizard_time_offset,
            skinned_cpu,
            wizard_last_phase: vec![0.0; scene_build.wizard_count as usize],
            hand_right_node: scene_build.hand_right_node,
            root_node: scene_build.root_node,
            projectiles: Vec::new(),
            particles: Vec::new(),
            fire_bolt,
            nameplates,
            nameplates_npc,
            bars,
            damage,
            hud,
            hud_model,
            pc_index: scene_build.pc_index,
            player: client_core::controller::PlayerController::new(scene_build.pc_pos),
            input: client_core::input::InputState::default(),
            cam_follow: gfx::camera_sys::FollowState { current_pos: glam::vec3(0.0, 5.0, -10.0), current_look: scene_build.cam_target },
            pc_cast_queued: false,
            pc_cast_kind: None,
            pc_anim_start: None,
            pc_cast_time: 0.0,
            pc_cast_fired: false,
            firebolt_cd_until: 0.0,
            firebolt_cd_dur: 0.0,
            gcd_until: 0.0,
            gcd_duration: 0.0,
            cam_orbit_yaw: 0.0,
            cam_orbit_pitch: 0.25,
            cam_distance: 8.5,
            cam_lift: 3.5,
            cam_look_height: 1.6,
            rmb_down: false,
            last_cursor_pos: None,
            screenshot_start: None,
            server: npcs_build.server,
            wizard_hp: vec![100; scene_build.wizard_count as usize],
            wizard_hp_max: 100,
            pc_alive: true,
        })
    }
}

