use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use glam::{Mat4, Vec3};
use log::info;
use ra_assets::{load_gltf_skinned, SkinnedMeshCPU};
use wgpu::util::DeviceExt;
use wgpu::{rwh::HasDisplayHandle, rwh::HasWindowHandle, SurfaceTargetUnsafe};
use winit::{dpi::PhysicalSize, event::*, event_loop::EventLoop, window::WindowAttributes};

#[derive(Parser, Debug)]
#[command(name = "model-viewer")] 
#[command(about = "Minimal wgpu model viewer (GLTF/GLB, baseColor, skin bind pose)")] 
struct Cli {
    /// Path to a .gltf or .glb file
    path: PathBuf,

    /// Start in wireframe if supported
    #[arg(long)]
    wireframe: bool,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct VSkinned {
    pos: [f32; 3],
    nrm: [f32; 3],
    uv: [f32; 2],
    joints: [u16; 4],
    weights: [f32; 4],
}

impl VSkinned {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<VSkinned>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute { shader_location: 0, offset: 0, format: wgpu::VertexFormat::Float32x3 },
            wgpu::VertexAttribute { shader_location: 1, offset: 12, format: wgpu::VertexFormat::Float32x3 },
            wgpu::VertexAttribute { shader_location: 2, offset: 24, format: wgpu::VertexFormat::Float32x2 },
            wgpu::VertexAttribute { shader_location: 3, offset: 32, format: wgpu::VertexFormat::Uint16x4 },
            wgpu::VertexAttribute { shader_location: 4, offset: 40, format: wgpu::VertexFormat::Float32x4 },
        ],
    };
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();
    pollster::block_on(run(cli))
}

#[allow(deprecated)]
async fn run(cli: Cli) -> Result<()> {
    // Window + surface
    let event_loop = EventLoop::new()?;
    let window = event_loop
        .create_window(
            WindowAttributes::default()
                .with_title("Model Viewer")
                .with_inner_size(PhysicalSize::new(1280, 720)),
        )?;
    let instance = wgpu::Instance::default();
    let raw_display = window.display_handle()?.as_raw();
    let raw_window = window.window_handle()?.as_raw();
    let surface = unsafe {
        instance.create_surface_unsafe(SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: raw_display,
            raw_window_handle: raw_window,
        })
    }?;

    // Adapter/device
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
        })
        .await
        .expect("adapter");
    let needed_features = if cli.wireframe {
        wgpu::Features::POLYGON_MODE_LINE
    } else {
        wgpu::Features::empty()
    };
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("viewer-device"),
            required_features: needed_features,
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        })
        .await?;

    // Surface config
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
    let max_dim = device.limits().max_texture_dimension_2d.max(1);
    let (mut width, mut height) = scale_to_max((size.width, size.height), max_dim);
    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width,
        height,
        present_mode,
        alpha_mode,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    // Load model (prefer decompressed glTF if present)
    let model_path = ra_assets::util::prepare_gltf_path(&cli.path)?;
    let skinned: SkinnedMeshCPU = load_gltf_skinned(&model_path)?;
    info!(
        "loaded: verts={}, indices={}, joints={}, anims={}",
        skinned.vertices.len(),
        skinned.indices.len(),
        skinned.joints_nodes.len(),
        skinned.animations.len()
    );

    // Build vertex/index buffers
    let vtx: Vec<VSkinned> = skinned
        .vertices
        .iter()
        .map(|v| VSkinned { pos: v.pos, nrm: v.nrm, uv: v.uv, joints: v.joints, weights: v.weights })
        .collect();
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vb"),
        contents: bytemuck::cast_slice(&vtx),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ib"),
        contents: bytemuck::cast_slice(&skinned.indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    let index_count = skinned.indices.len() as u32;

    // Globals
    let globals = Globals { view_proj: Mat4::IDENTITY.to_cols_array_2d() };
    let globals_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("globals"),
        contents: bytemuck::bytes_of(&globals),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let globals_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("globals-bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        }],
    });
    let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("globals-bg"),
        layout: &globals_bgl,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() }],
    });

    // Skin palette for bind pose (no anim in v1)
    let palette = compute_bind_pose_palette(&skinned);
    let skin_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("skin-palette"),
        contents: bytemuck::cast_slice(&palette),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let skin_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("skin-bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        }],
    });
    let skin_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("skin-bg"),
        layout: &skin_bgl,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: skin_buf.as_entire_binding() }],
    });

    // Material (base color)
    let (tex_view, sampler) = if let Some(tex) = &skinned.base_color_texture {
        let size = wgpu::Extent3d { width: tex.width, height: tex.height, depth_or_array_layers: 1 };
        let tex_obj = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("albedo"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &tex_obj, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &tex.pixels,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * tex.width), rows_per_image: Some(tex.height) },
            size,
        );
        let view = tex_obj.create_view(&wgpu::TextureViewDescriptor::default());
        let samp = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("samp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        (view, samp)
    } else {
        // 1x1 white fallback
        let tex_obj = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("white"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &tex_obj, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
        let view = tex_obj.create_view(&wgpu::TextureViewDescriptor::default());
        let samp = device.create_sampler(&wgpu::SamplerDescriptor::default());
        (view, samp)
    };
    let mat_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mat-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } }, count: None },
        ],
    });
    let mat_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("mat-bg"),
        layout: &mat_bgl,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Sampler(&sampler) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&tex_view) },
        ],
    });

    // Pipeline
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("skinned-shader"),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("shader_skinned.wgsl"))),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pl"),
        bind_group_layouts: &[&globals_bgl, &mat_bgl, &skin_bgl],
        push_constant_ranges: &[],
    });
    let depth_format = wgpu::TextureFormat::Depth32Float;
    let mut pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("pipe"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main_skinned"), buffers: &[VSkinned::LAYOUT], compilation_options: Default::default() },
        fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs_main"), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
        primitive: wgpu::PrimitiveState { polygon_mode: if cli.wireframe { wgpu::PolygonMode::Line } else { wgpu::PolygonMode::Fill }, ..Default::default() },
        depth_stencil: Some(wgpu::DepthStencilState { format: depth_format, depth_write_enabled: true, depth_compare: wgpu::CompareFunction::Less, stencil: wgpu::StencilState::default(), bias: wgpu::DepthBiasState::default() }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let mut depth_view = create_depth(&device, width, height, depth_format);

    // Fit camera to model bounds
    let (min_b, max_b) = compute_bounds(&skinned);
    let center = 0.5 * (min_b + max_b);
    let diag = (max_b - min_b).length().max(1.0);
    let mut orbit_t = 0.0f32;

    Ok(event_loop.run(move |event, elwt| match event {
        Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => elwt.exit(),
        Event::WindowEvent { event: WindowEvent::Resized(new_size), .. } => {
            (width, height) = scale_to_max((new_size.width, new_size.height), max_dim);
            config.width = width.max(1);
            config.height = height.max(1);
            surface.configure(&device, &config);
            depth_view = create_depth(&device, width, height, depth_format);
        }
        Event::AboutToWait => {
            // Simple auto-orbit camera
            orbit_t += 0.6 / 60.0;
            let eye = center + Vec3::new(orbit_t.cos() * diag * 0.8, diag * 0.4, orbit_t.sin() * diag * 0.8);
            let view = Mat4::look_at_rh(eye, center, Vec3::Y);
            let proj = Mat4::perspective_rh_gl(60f32.to_radians(), width as f32 / height as f32, 0.05, 100.0 * diag);
            let vp = (proj * view).to_cols_array_2d();
            queue.write_buffer(&globals_buf, 0, bytemuck::bytes_of(&Globals { view_proj: vp }));

            let frame = match surface.get_current_texture() { Ok(f) => f, Err(_) => { surface.configure(&device, &config); surface.get_current_texture().expect("frame") } };
            let view_tex = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("enc") });
            {
                let mut rpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("rpass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view_tex, resolve_target: None, depth_slice: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.03, g: 0.03, b: 0.05, a: 1.0 }), store: wgpu::StoreOp::Store } })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &depth_view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });
                rpass.set_pipeline(&pipeline);
                rpass.set_bind_group(0, &globals_bg, &[]);
                rpass.set_bind_group(1, &mat_bg, &[]);
                rpass.set_bind_group(2, &skin_bg, &[]);
                rpass.set_vertex_buffer(0, vb.slice(..));
                rpass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
                rpass.draw_indexed(0..index_count, 0, 0..1);
            }
            queue.submit(Some(enc.finish()));
            frame.present();
        }
        _ => {}
    })?)
}

fn compute_bind_pose_palette(model: &SkinnedMeshCPU) -> Vec<[[f32; 4]; 4]> {
    let n = model.parent.len();
    let mut globals = vec![Mat4::IDENTITY; n];
    // Compute global matrices via DFS
    fn compute(i: usize, parent: &[Option<usize>], t: &[Vec3], r: &[glam::Quat], s: &[Vec3], out: &mut [Mat4]) {
        if out[i] != Mat4::IDENTITY { return; }
        if let Some(p) = parent[i] {
            if out[p] == Mat4::IDENTITY { compute(p, parent, t, r, s, out); }
            out[i] = out[p] * Mat4::from_scale_rotation_translation(s[i], r[i], t[i]);
        } else {
            out[i] = Mat4::from_scale_rotation_translation(s[i], r[i], t[i]);
        }
    }
    for i in 0..n { compute(i, &model.parent, &model.base_t, &model.base_r, &model.base_s, &mut globals); }
    let mut palette: Vec<[[f32; 4]; 4]> = Vec::with_capacity(model.joints_nodes.len());
    for (j, &node_idx) in model.joints_nodes.iter().enumerate() {
        let m = globals[node_idx] * model.inverse_bind[j];
        palette.push(m.to_cols_array_2d());
    }
    palette
}

fn compute_bounds(model: &SkinnedMeshCPU) -> (Vec3, Vec3) {
    let mut min_b = Vec3::splat(f32::INFINITY);
    let mut max_b = Vec3::splat(f32::NEG_INFINITY);
    for v in &model.vertices {
        let p = Vec3::from(v.pos);
        min_b = min_b.min(p);
        max_b = max_b.max(p);
    }
    (min_b, max_b)
}

fn create_depth(device: &wgpu::Device, w: u32, h: u32, fmt: wgpu::TextureFormat) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: fmt,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}

fn scale_to_max((w, h): (u32, u32), max_dim: u32) -> (u32, u32) {
    if w <= max_dim && h <= max_dim { return (w, h); }
    let aspect = (w as f32) / (h as f32);
    if w >= h {
        let nw = max_dim;
        let nh = (max_dim as f32 / aspect).round().max(1.0) as u32;
        (nw, nh)
    } else {
        let nh = max_dim;
        let nw = (max_dim as f32 * aspect).round().max(1.0) as u32;
        (nw, nh)
    }
}
