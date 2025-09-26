use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use log::info;
use std::time::Instant;
use wgpu::{rwh::HasDisplayHandle, rwh::HasWindowHandle, util::DeviceExt, SurfaceTargetUnsafe};
use winit::{dpi::PhysicalSize, event::*, event_loop::EventLoop, window::WindowAttributes};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Globals { mvp: [[f32; 4]; 4] }

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Vertex { pos: [f32; 3], uv: [f32; 2] }

impl Vertex {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2],
    };
}

struct MeshCpu {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

struct TextureCpu { pixels: Vec<u8>, width: u32, height: u32 }

fn main() -> Result<()> {
    env_logger::init();
    pollster::block_on(run())
}

// NOTE: Uses deprecated EventLoop APIs for simplicity in this viewer.
// When we bump winit here, migrate to `EventLoop::run_app` and `ActiveEventLoop::create_window`.
#[allow(deprecated)]
async fn run() -> Result<()> {
    let event_loop = EventLoop::new().context("create event loop")?;
    let window = event_loop.create_window(WindowAttributes::default().with_title("Wizard Viewer").with_inner_size(PhysicalSize::new(1280, 720))).context("create window")?;

    // WGPU instance + surface
    let instance = wgpu::Instance::default();
    let raw_display = window.display_handle()?.as_raw();
    let raw_window = window.window_handle()?.as_raw();
    let surface = unsafe { instance.create_surface_unsafe(SurfaceTargetUnsafe::RawHandle { raw_display_handle: raw_display, raw_window_handle: raw_window }) }?;

    // Adapter + device
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions { compatible_surface: Some(&surface), power_preference: wgpu::PowerPreference::HighPerformance, force_fallback_adapter: false }).await.context("request adapter")?;
    let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor { label: Some("viewer-device"), required_features: wgpu::Features::empty(), required_limits: wgpu::Limits::downlevel_defaults(), memory_hints: wgpu::MemoryHints::Performance, trace: wgpu::Trace::default() }).await.context("request device")?;

    // Surface config (clamp to max texture dim to avoid validation error)
    let size = window.inner_size();
    let caps = surface.get_capabilities(&adapter);
    let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);
    let present_mode = caps.present_modes.iter().copied().find(|m| *m == wgpu::PresentMode::Mailbox).unwrap_or(wgpu::PresentMode::Fifo);
    let alpha_mode = caps.alpha_modes[0];
    let max_dim = device.limits().max_texture_dimension_2d.max(1);
    let (width, height) = scale_to_max((size.width, size.height), max_dim);
    if (width, height) != (size.width, size.height) {
        log::warn!("Clamping surface from {}x{} to {}x{} (max_dim={})", size.width, size.height, width, height, max_dim);
    }
    let mut config = wgpu::SurfaceConfiguration { usage: wgpu::TextureUsages::RENDER_ATTACHMENT, format, width, height, present_mode, alpha_mode, view_formats: vec![], desired_maximum_frame_latency: 2 };
    surface.configure(&device, &config);

    // Load mesh + texture from assets/models/wizard.gltf
    let (mesh, tex_cpu) = load_gltf_mesh_and_basecolor("assets/models/wizard.gltf")?;
    info!("viewer: verts={}, indices={}, tex={}x{}", mesh.vertices.len(), mesh.indices.len(), tex_cpu.width, tex_cpu.height);

    // Upload GPU resources
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("vb"), contents: bytemuck::cast_slice(&mesh.vertices), usage: wgpu::BufferUsages::VERTEX });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("ib"), contents: bytemuck::cast_slice(&mesh.indices), usage: wgpu::BufferUsages::INDEX });
    let index_count = mesh.indices.len() as u32;

    let globals = Globals { mvp: Mat4::IDENTITY.to_cols_array_2d() };
    let globals_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("globals"), contents: bytemuck::bytes_of(&globals), usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST });
    let globals_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("globals-bgl"), entries: &[wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None }] });
    let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("globals-bg"), layout: &globals_bgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() }] });

    // Texture + sampler + material BGL
    let tex_size = wgpu::Extent3d { width: tex_cpu.width, height: tex_cpu.height, depth_or_array_layers: 1 };
    let tex_obj = device.create_texture(&wgpu::TextureDescriptor { label: Some("albedo"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8UnormSrgb, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
    queue.write_texture(wgpu::TexelCopyTextureInfo { texture: &tex_obj, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All }, &tex_cpu.pixels, wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * tex_cpu.width), rows_per_image: Some(tex_cpu.height) }, tex_size);
    let tex_view = tex_obj.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("sampler"), mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, mipmap_filter: wgpu::FilterMode::Nearest, address_mode_u: wgpu::AddressMode::Repeat, address_mode_v: wgpu::AddressMode::Repeat, ..Default::default() });
    let material_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("mat-bgl"), entries: &[
        wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } }, count: None },
        wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
    ] });
    let material_bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("mat-bg"), layout: &material_bgl, entries: &[
        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&tex_view) },
        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
    ] });

    // Pipeline
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("viewer-shader"), source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("wizard_viewer.wgsl"))) });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("pl"), bind_group_layouts: &[&globals_bgl, &material_bgl], push_constant_ranges: &[] });
    let depth_format = wgpu::TextureFormat::Depth32Float;
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor { label: Some("pipe"), layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), buffers: &[Vertex::LAYOUT], compilation_options: Default::default() },
        fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs_main"), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState { format: depth_format, depth_write_enabled: true, depth_compare: wgpu::CompareFunction::Less, stencil: wgpu::StencilState::default(), bias: wgpu::DepthBiasState::default() }),
        multisample: wgpu::MultisampleState::default(), multiview: None, cache: None });

    let mut depth_view = create_depth(&device, config.width, config.height, depth_format);

    let start = Instant::now();
    Ok(event_loop.run(move |event, elwt| match event {
        Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => elwt.exit(),
        Event::WindowEvent { event: WindowEvent::Resized(new_size), .. } => {
            let (w, h) = scale_to_max((new_size.width, new_size.height), max_dim);
            config.width = w.max(1); config.height = h.max(1);
            surface.configure(&device, &config);
            depth_view = create_depth(&device, config.width, config.height, depth_format);
        }
        Event::AboutToWait => {
            // Camera (simple orbit)
            let t = start.elapsed().as_secs_f32();
            let eye = Vec3::new(t.cos()*2.5, 1.6, t.sin()*2.5);
            let center = Vec3::new(0.0, 1.0, 0.0);
            let up = Vec3::Y;
            let view = Mat4::look_at_rh(eye, center, up);
            let proj = Mat4::perspective_rh_gl(60_f32.to_radians(), config.width as f32 / config.height as f32, 0.05, 100.0);
            let mvp = (proj * view).to_cols_array_2d();
            queue.write_buffer(&globals_buf, 0, bytemuck::bytes_of(&Globals { mvp }));

            let frame = match surface.get_current_texture() { Ok(f) => f, Err(_) => { surface.configure(&device, &config); surface.get_current_texture().expect("acquire frame") } };
            let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("enc") });
            {
                let mut rpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor { label: Some("pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, resolve_target: None, depth_slice: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.02, g: 0.08, b: 0.16, a: 1.0 }), store: wgpu::StoreOp::Store } })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &depth_view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                    occlusion_query_set: None, timestamp_writes: None });
                rpass.set_pipeline(&pipeline);
                rpass.set_bind_group(0, &globals_bg, &[]);
                rpass.set_bind_group(1, &material_bg, &[]);
                rpass.set_vertex_buffer(0, vb.slice(..));
                rpass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                rpass.draw_indexed(0..index_count, 0, 0..1);
            }
            queue.submit(Some(enc.finish()));
            frame.present();
        }
        _ => {}
    })?)
}

fn scale_to_max((w, h): (u32, u32), max_dim: u32) -> (u32, u32) {
    if w <= max_dim && h <= max_dim { return (w, h); }
    let aspect = (w as f32) / (h as f32);
    if w >= h {
        let nw = max_dim; let nh = (max_dim as f32 / aspect).round().max(1.0) as u32; (nw, nh)
    } else {
        let nh = max_dim; let nw = (max_dim as f32 * aspect).round().max(1.0) as u32; (nw, nh)
    }
}

fn create_depth(device: &wgpu::Device, w: u32, h: u32, fmt: wgpu::TextureFormat) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor { label: Some("depth"), size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: fmt, usage: wgpu::TextureUsages::RENDER_ATTACHMENT, view_formats: &[] });
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}

fn load_gltf_mesh_and_basecolor(path: &str) -> Result<(MeshCpu, TextureCpu)> {
    let (doc, buffers, images) = gltf::import(path).with_context(|| format!("import glTF: {}", path))?;
    let mesh = doc.meshes().next().context("no mesh in glTF")?;
    let prim = mesh.primitives().next().context("no primitive in glTF mesh")?;
    let reader = prim.reader(|b| buffers.get(b.index()).map(|bb| bb.0.as_slice()));
    let pos: Vec<[f32;3]> = reader.read_positions().context("positions missing")?.collect();
    let uv_set = prim.material().pbr_metallic_roughness().base_color_texture().map(|ti| ti.tex_coord()).unwrap_or(0);
    let uv_opt = reader.read_tex_coords(uv_set).map(|tc| tc.into_f32());
    let uv: Vec<[f32;2]> = if let Some(it) = uv_opt { it.collect() } else { pos.iter().map(|p| [0.5 + 0.5*p[0], 0.5 - 0.5*p[2]]).collect() };
    let indices: Vec<u32> = match reader.read_indices() { Some(gltf::mesh::util::ReadIndices::U16(it)) => it.map(|v| v as u32).collect(), Some(gltf::mesh::util::ReadIndices::U32(it)) => it.collect(), Some(gltf::mesh::util::ReadIndices::U8(it)) => it.map(|v| v as u32).collect(), None => (0..pos.len() as u32).collect() };
    let vertices: Vec<Vertex> = pos.into_iter().zip(uv.into_iter()).map(|(p, t)| Vertex { pos: p, uv: t }).collect();

    // baseColor texture
    let texinfo = prim.material().pbr_metallic_roughness().base_color_texture().context("baseColorTexture missing in material")?;
    let img_idx = texinfo.texture().source().index();
    let img = images.get(img_idx).context("base color image not found")?;
    let (w, h) = (img.width, img.height);
    let pixels = match img.format {
        gltf::image::Format::R8G8B8A8 => img.pixels.clone(),
        gltf::image::Format::R8G8B8 => {
            let mut out = Vec::with_capacity((w*h*4) as usize);
            for c in img.pixels.chunks_exact(3) { out.extend_from_slice(&[c[0], c[1], c[2], 255]); }
            out
        }
        gltf::image::Format::R8 => { let mut out = Vec::with_capacity((w*h*4) as usize); for &r in &img.pixels { out.extend_from_slice(&[r,r,r,255]); } out }
        _ => img.pixels.clone(),
    };

    Ok((MeshCpu { vertices, indices }, TextureCpu { pixels, width: w, height: h }))
}
