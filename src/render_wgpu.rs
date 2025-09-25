use anyhow::Context;
use bytemuck::{Pod, Zeroable};
use glam::{vec3, Mat4, Vec3};
use std::time::Instant;
use wgpu::{rwh::HasDisplayHandle, rwh::HasWindowHandle, util::DeviceExt, SurfaceError, SurfaceTargetUnsafe};
use winit::dpi::PhysicalSize;
use winit::window::Window;

pub struct WgpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    depth: wgpu::TextureView,

    // Pipeline and resources
    pipeline: wgpu::RenderPipeline,
    globals_buf: wgpu::Buffer,
    globals_bg: wgpu::BindGroup,

    // Model-specific bind group (we reuse buffer per draw)
    model_buf: wgpu::Buffer,
    model_bg: wgpu::BindGroup,

    // Geometry
    cube_vb: wgpu::Buffer,
    cube_ib: wgpu::Buffer,
    cube_index_count: u32,
    plane_vb: wgpu::Buffer,
    plane_ib: wgpu::Buffer,
    plane_index_count: u32,

    // Time
    start: Instant,
}

impl WgpuState {
    pub async fn new(window: &Window) -> anyhow::Result<Self> {
        let size = window.inner_size();

        // Instance/Surface
        let instance = wgpu::Instance::default();
        // Create a surface without tying its lifetime to &Window using raw handles.
        let raw_display = window
            .display_handle()
            .map_err(|e| anyhow::anyhow!("display_handle: {e}"))?
            .as_raw();
        let raw_window = window
            .window_handle()
            .map_err(|e| anyhow::anyhow!("window_handle: {e}"))?
            .as_raw();
        let surface = unsafe {
            instance
                .create_surface_unsafe(SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: raw_display,
                    raw_window_handle: raw_window,
                })
        }
        .context("create wgpu surface (unsafe)")?;

        // Adapter/Device
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
            })
            .await
            .context("request adapter")?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("wgpu-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::default(),
            })
            .await
            .context("request device")?;

        // Surface config
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

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Depth buffer
        let depth = create_depth_view(&device, config.width, config.height, config.format);

        // Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("basic-shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(BASIC_WGSL)),
        });

        // Bind group layouts
        let globals_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let model_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("model-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline-layout"),
            bind_group_layouts: &[&globals_bgl, &model_bgl],
            push_constant_ranges: &[],
        });

        // Render pipeline
        let vertex_buffers = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
        }];

        let depth_format = wgpu::TextureFormat::Depth32Float;
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), buffers: &vertex_buffers, compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Buffers
        let globals = Globals { view_proj: Mat4::IDENTITY.to_cols_array_2d(), time_pad: [0.0; 4] };
        let globals_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globals"),
            contents: bytemuck::bytes_of(&globals),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals-bg"),
            layout: &globals_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() }],
        });

        let model = Model { model: Mat4::IDENTITY.to_cols_array_2d(), color: [1.0, 0.85, 0.3], emissive: 0.2, _pad: [0.0; 4] };
        let model_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("model"), contents: bytemuck::bytes_of(&model), usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST });
        let model_bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("model-bg"), layout: &model_bgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: model_buf.as_entire_binding() }] });

        // Geometry: cube and plane
        let (cube_vb, cube_ib, cube_index_count) = create_cube(&device);
        let (plane_vb, plane_ib, plane_index_count) = create_plane(&device);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size,
            depth,
            pipeline,
            globals_buf,
            globals_bg,
            model_buf,
            model_bg,
            cube_vb,
            cube_ib,
            cube_index_count,
            plane_vb,
            plane_ib,
            plane_index_count,
            start: Instant::now(),
        })
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth = create_depth_view(&self.device, self.config.width, self.config.height, self.config.format);
        }
    }

    pub fn reconfigure_surface(&mut self) {
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render(&mut self) -> Result<(), SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Update time and camera
        let t = self.start.elapsed().as_secs_f32();
        let aspect = self.config.width as f32 / self.config.height as f32;
        let cam = Camera::orbit(vec3(0.0, 1.0, 0.0), 4.0, t * 0.3, aspect);
        let globals = Globals { view_proj: cam.view_proj().to_cols_array_2d(), time_pad: [t, 0.0, 0.0, 0.0] };
        self.queue.write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("clear-encoder"),
            });
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

            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(0, &self.globals_bg, &[]);

            // Draw plane (water): model identity at y=0, colored blue
            // Strongly distinct plane color (green-teal)
            let plane_model = Model { model: Mat4::IDENTITY.to_cols_array_2d(), color: [0.05, 0.80, 0.30], emissive: 0.0, _pad: [0.0; 4] };
            self.queue.write_buffer(&self.model_buf, 0, bytemuck::bytes_of(&plane_model));
            rpass.set_bind_group(1, &self.model_bg, &[]);
            rpass.set_vertex_buffer(0, self.plane_vb.slice(..));
            rpass.set_index_buffer(self.plane_ib.slice(..), IndexFormat::Uint16);
            rpass.draw_indexed(0..self.plane_index_count, 0, 0..1);

            // Draw cube (shard): translate up and rotate over time
            let model = Mat4::from_rotation_y(t) * Mat4::from_translation(vec3(0.0, 1.0, 0.0));
            // High-contrast shard color (crimson)
            let shard_model = Model { model: model.to_cols_array_2d(), color: [0.85, 0.15, 0.15], emissive: 0.15, _pad: [0.0; 4] };
            self.queue.write_buffer(&self.model_buf, 0, bytemuck::bytes_of(&shard_model));
            rpass.set_bind_group(1, &self.model_bg, &[]);
            rpass.set_vertex_buffer(0, self.cube_vb.slice(..));
            rpass.set_index_buffer(self.cube_ib.slice(..), IndexFormat::Uint16);
            rpass.draw_indexed(0..self.cube_index_count, 0, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    time_pad: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Model {
    model: [[f32; 4]; 4],
    color: [f32; 3],
    emissive: f32,
    _pad: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 3],
    nrm: [f32; 3],
}

fn create_depth_view(device: &wgpu::Device, width: u32, height: u32, _color_format: wgpu::TextureFormat) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth-texture"),
        size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}

fn create_cube(device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer, u32) {
    let p = 0.5f32;
    let vertices = [
        // +X
        ([p, -p, -p], [1.0, 0.0, 0.0]),
        ([p, p, -p], [1.0, 0.0, 0.0]),
        ([p, p, p], [1.0, 0.0, 0.0]),
        ([p, -p, p], [1.0, 0.0, 0.0]),
        // -X
        ([-p, -p, p], [-1.0, 0.0, 0.0]),
        ([-p, p, p], [-1.0, 0.0, 0.0]),
        ([-p, p, -p], [-1.0, 0.0, 0.0]),
        ([-p, -p, -p], [-1.0, 0.0, 0.0]),
        // +Y
        ([-p, p, -p], [0.0, 1.0, 0.0]),
        ([p, p, -p], [0.0, 1.0, 0.0]),
        ([p, p, p], [0.0, 1.0, 0.0]),
        ([-p, p, p], [0.0, 1.0, 0.0]),
        // -Y
        ([-p, -p, p], [0.0, -1.0, 0.0]),
        ([p, -p, p], [0.0, -1.0, 0.0]),
        ([p, -p, -p], [0.0, -1.0, 0.0]),
        ([-p, -p, -p], [0.0, -1.0, 0.0]),
        // +Z
        ([-p, -p, p], [0.0, 0.0, 1.0]),
        ([p, -p, p], [0.0, 0.0, 1.0]),
        ([p, p, p], [0.0, 0.0, 1.0]),
        ([-p, p, p], [0.0, 0.0, 1.0]),
        // -Z
        ([p, -p, -p], [0.0, 0.0, -1.0]),
        ([-p, -p, -p], [0.0, 0.0, -1.0]),
        ([-p, p, -p], [0.0, 0.0, -1.0]),
        ([p, p, -p], [0.0, 0.0, -1.0]),
    ];
    let verts: Vec<Vertex> = vertices.iter().map(|(p, n)| Vertex { pos: *p, nrm: *n }).collect();
    let indices: [u16; 36] = [
        0, 1, 2, 0, 2, 3, // +X
        4, 5, 6, 4, 6, 7, // -X
        8, 9, 10, 8, 10, 11, // +Y
        12, 13, 14, 12, 14, 15, // -Y
        16, 17, 18, 16, 18, 19, // +Z
        20, 21, 22, 20, 22, 23, // -Z
    ];
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("cube-vb"), contents: bytemuck::cast_slice(&verts), usage: wgpu::BufferUsages::VERTEX });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("cube-ib"), contents: bytemuck::cast_slice(&indices), usage: wgpu::BufferUsages::INDEX });
    (vb, ib, indices.len() as u32)
}

fn create_plane(device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer, u32) {
    // A simple 2x2 plane centered at origin on XZ
    let s = 10.0f32;
    let verts = [
        Vertex { pos: [-s, 0.0, -s], nrm: [0.0, 1.0, 0.0] },
        Vertex { pos: [ s, 0.0, -s], nrm: [0.0, 1.0, 0.0] },
        Vertex { pos: [ s, 0.0,  s], nrm: [0.0, 1.0, 0.0] },
        Vertex { pos: [-s, 0.0,  s], nrm: [0.0, 1.0, 0.0] },
    ];
    let idx: [u16; 6] = [0, 1, 2, 0, 2, 3];
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("plane-vb"), contents: bytemuck::cast_slice(&verts), usage: wgpu::BufferUsages::VERTEX });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("plane-ib"), contents: bytemuck::cast_slice(&idx), usage: wgpu::BufferUsages::INDEX });
    (vb, ib, idx.len() as u32)
}

struct Camera {
    eye: Vec3,
    target: Vec3,
    up: Vec3,
    aspect: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,
}

impl Camera {
    fn orbit(target: Vec3, radius: f32, angle: f32, aspect: f32) -> Self {
        let eye = Vec3::new(angle.cos() * radius, radius * 0.6, angle.sin() * radius);
        Self { eye, target, up: Vec3::Y, aspect, fovy: 60f32.to_radians(), znear: 0.1, zfar: 100.0 }
    }
    fn view_proj(&self) -> Mat4 {
        let view = Mat4::look_at_rh(self.eye, self.target, self.up);
        let proj = Mat4::perspective_rh(self.fovy, self.aspect, self.znear, self.zfar);
        proj * view
    }
}

const BASIC_WGSL: &str = r#"
struct Globals { view_proj: mat4x4<f32>, time_pad: vec4<f32> };
@group(0) @binding(0) var<uniform> globals: Globals;

struct Model { model: mat4x4<f32>, color: vec3<f32>, emissive: f32, _pad: vec2<f32> };
@group(1) @binding(0) var<uniform> model_u: Model;

struct VSIn {
  @location(0) pos: vec3<f32>,
  @location(1) nrm: vec3<f32>,
};

struct VSOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) nrm: vec3<f32>,
  @location(1) world: vec3<f32>,
};

@vertex
fn vs_main(input: VSIn) -> VSOut {
  var p = input.pos;
  // Simple water ripple if y==0 plane (approx by checking normal)
  if (abs(input.nrm.y) > 0.9 && abs(p.y) < 0.0001) {
    let amp = 0.05;
    let freq = 0.5;
    let t = globals.time_pad.x;
    p.y = amp * sin(p.x * freq + t * 1.5) + amp * cos(p.z * freq + t);
  }
  let world_pos = (model_u.model * vec4<f32>(p, 1.0)).xyz;
  var out: VSOut;
  out.world = world_pos;
  out.nrm = normalize((model_u.model * vec4<f32>(input.nrm, 0.0)).xyz);
  out.pos = globals.view_proj * vec4<f32>(world_pos, 1.0);
  return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
  let light_dir = normalize(vec3<f32>(0.3, 1.0, 0.4));
  let ndl = max(dot(in.nrm, light_dir), 0.0);
  let base = model_u.color * (0.2 + 0.8 * ndl) + model_u.emissive;
  return vec4<f32>(base, 1.0);
}
"#;
