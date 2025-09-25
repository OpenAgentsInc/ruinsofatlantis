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
    max_dim: u32,
    depth: wgpu::TextureView,

    // Pipeline and resources
    pipeline: wgpu::RenderPipeline,
    inst_pipeline: wgpu::RenderPipeline,
    wire_pipeline: Option<wgpu::RenderPipeline>,
    globals_buf: wgpu::Buffer,
    globals_bg: wgpu::BindGroup,

    // Model-specific bind groups (separate buffers for plane and shard)
    plane_model_buf: wgpu::Buffer,
    plane_model_bg: wgpu::BindGroup,
    shard_model_buf: wgpu::Buffer,
    shard_model_bg: wgpu::BindGroup,

    // Geometry
    cube_vb: wgpu::Buffer,
    cube_ib: wgpu::Buffer,
    cube_index_count: u32,
    plane_vb: wgpu::Buffer,
    plane_ib: wgpu::Buffer,
    plane_index_count: u32,

    // Instancing
    instance_buf: wgpu::Buffer,
    _instances: Vec<Instance>,
    grid_cols: u32,
    grid_rows: u32,
    culling_enabled: bool,
    _selected: Option<usize>,
    wire_enabled: bool,

    // Time
    start: Instant,
}

impl WgpuState {
    fn scale_to_max((w0, h0): (u32, u32), max_dim: u32) -> (u32, u32) {
        let (mut w, mut h) = (w0.max(1), h0.max(1));
        if w > max_dim || h > max_dim {
            let scale = (w as f32 / max_dim as f32).max(h as f32 / max_dim as f32);
            w = ((w as f32 / scale).floor() as u32).clamp(1, max_dim);
            h = ((h as f32 / scale).floor() as u32).clamp(1, max_dim);
        }
        (w, h)
    }
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

        // Determine max supported dimension from the actual device limits (not adapter)
        let dev_limits = device.limits();
        // Extra safety: hard-cap to 2048 for surfaces on older drivers that report larger device limits.
        let max_dim = dev_limits.max_texture_dimension_2d.max(1).min(2048);
        let (w, h) = Self::scale_to_max((size.width, size.height), max_dim);
        if w != size.width || h != size.height {
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

        // Render pipelines (non-instanced + instanced; optional wireframe)
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

        // Instanced pipeline (adds per-instance transform+color)
        let instance_buffers = [
            wgpu::VertexBufferLayout {
                array_stride: vertex_buffers[0].array_stride,
                step_mode: vertex_buffers[0].step_mode,
                attributes: vertex_buffers[0].attributes,
            },
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Instance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![
                    2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, // mat4
                    6 => Float32x3, // color
                    7 => Float32    // selected
                ],
            },
        ];
        let inst_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("inst-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_inst"), buffers: &instance_buffers, compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_inst"),
                targets: &[Some(wgpu::ColorTargetState { format: config.format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState { format: depth_format, depth_write_enabled: true, depth_compare: wgpu::CompareFunction::Less, stencil: wgpu::StencilState::default(), bias: wgpu::DepthBiasState::default() }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Optional wireframe pipeline if supported
        let features = device.features();
        let wire_pipeline = if features.contains(wgpu::Features::POLYGON_MODE_LINE) {
            Some(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("inst-pipeline-wire"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_inst"), buffers: &instance_buffers, compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs_inst"), targets: &[Some(wgpu::ColorTargetState { format: config.format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
                primitive: wgpu::PrimitiveState { polygon_mode: wgpu::PolygonMode::Line, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState { format: depth_format, depth_write_enabled: true, depth_compare: wgpu::CompareFunction::Less, stencil: wgpu::StencilState::default(), bias: wgpu::DepthBiasState::default() }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            }))
        } else { None };

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

        let plane_model_init = Model { model: Mat4::IDENTITY.to_cols_array_2d(), color: [0.0, 1.0, 0.0], emissive: 0.0, _pad: [0.0; 4] };
        let plane_model_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("plane-model"), contents: bytemuck::bytes_of(&plane_model_init), usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST });
        let plane_model_bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("plane-model-bg"), layout: &model_bgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: plane_model_buf.as_entire_binding() }] });

        let shard_model_init = Model { model: Mat4::IDENTITY.to_cols_array_2d(), color: [1.0, 0.0, 0.0], emissive: 0.2, _pad: [0.0; 4] };
        let shard_model_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("shard-model"), contents: bytemuck::bytes_of(&shard_model_init), usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST });
        let shard_model_bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("shard-model-bg"), layout: &model_bgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: shard_model_buf.as_entire_binding() }] });

        // Geometry: cube and plane
        let (cube_vb, cube_ib, cube_index_count) = create_cube(&device);
        // Make plane large enough to sit under the full grid
        let grid_cols = 100u32; // 10k instances
        let grid_rows = 100u32;
        let spacing = 2.5f32;
        let plane_extent = spacing * (grid_cols.max(grid_rows) as f32);
        let (plane_vb, plane_ib, plane_index_count) = create_plane(&device, plane_extent);

        // Instances grid
        // Build grid instances
        let mut instances = Vec::with_capacity((grid_cols * grid_rows) as usize);
        for r in 0..grid_rows {
            for c in 0..grid_cols {
                let x = (c as f32 - grid_cols as f32 * 0.5) * spacing;
                let z = (r as f32 - grid_rows as f32 * 0.5) * spacing;
                let model = Mat4::from_translation(Vec3::new(x, 1.0, z));
                instances.push(Instance { model: model.to_cols_array_2d(), color: [0.85, 0.15, 0.15], selected: 0.0 });
            }
        }
        let instance_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("instance-buf"),
            contents: bytemuck::cast_slice(&instances),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

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
            globals_buf,
            globals_bg,
            plane_model_buf,
            plane_model_bg,
            shard_model_buf,
            shard_model_bg,
            cube_vb,
            cube_ib,
            cube_index_count,
            plane_vb,
            plane_ib,
            plane_index_count,
            instance_buf,
            _instances: instances,
            grid_cols,
            grid_rows,
            culling_enabled: true,
            _selected: None,
            wire_enabled: false,
            start: Instant::now(),
        })
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            let (w, h) = Self::scale_to_max((new_size.width, new_size.height), self.max_dim);
            if w != new_size.width || h != new_size.height {
                log::warn!(
                    "Resized {}x{} exceeds max {}, clamped to {}x{} (aspect kept)",
                    new_size.width, new_size.height, self.max_dim, w, h
                );
            }
            self.size = PhysicalSize::new(w, h);
            self.config.width = w;
            self.config.height = h;
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
        // Lift the camera higher and farther so the grid reads as a plaza
        let cam = Camera::orbit(vec3(0.0, 0.0, 0.0), 40.0, t * 0.15, aspect);
        let globals = Globals { view_proj: cam.view_proj().to_cols_array_2d(), time_pad: [t, 0.0, 0.0, 0.0] };
        self.queue.write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));

        // Prepare per-model uniforms before encoding the pass
        let plane_model = Model { model: Mat4::IDENTITY.to_cols_array_2d(), color: [0.05, 0.80, 0.30], emissive: 0.0, _pad: [0.0; 4] };
        let model_mtx = Mat4::from_rotation_y(t) * Mat4::from_translation(vec3(0.0, 1.0, 0.0));
        let shard_model = Model { model: model_mtx.to_cols_array_2d(), color: [0.85, 0.15, 0.15], emissive: 0.15, _pad: [0.0; 4] };
        self.queue.write_buffer(&self.plane_model_buf, 0, bytemuck::bytes_of(&plane_model));
        self.queue.write_buffer(&self.shard_model_buf, 0, bytemuck::bytes_of(&shard_model));

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

            // Draw plane using its dedicated model buffer/bind-group
            rpass.set_bind_group(1, &self.plane_model_bg, &[]);
            rpass.set_vertex_buffer(0, self.plane_vb.slice(..));
            rpass.set_index_buffer(self.plane_ib.slice(..), IndexFormat::Uint16);
            rpass.draw_indexed(0..self.plane_index_count, 0, 0..1);

            // Draw shard using its dedicated model buffer/bind-group
            // Instanced shards: choose pipeline
            let inst_pipe = if self.wire_enabled { self.wire_pipeline.as_ref().unwrap_or(&self.inst_pipeline) } else { &self.inst_pipeline };
            rpass.set_pipeline(inst_pipe);
            rpass.set_bind_group(0, &self.globals_bg, &[]);
            rpass.set_bind_group(1, &self.shard_model_bg, &[]); // per-draw model (rotation)
            rpass.set_vertex_buffer(0, self.cube_vb.slice(..));
            rpass.set_vertex_buffer(1, self.instance_buf.slice(..));
            rpass.set_index_buffer(self.cube_ib.slice(..), IndexFormat::Uint16);

            // Simple CPU culling by rows: compute visible range and draw only those rows
            let cam_z = cam.eye.z;
            let mut _draws = 0u32;
            for r in 0..self.grid_rows {
                let z_world = (r as f32 - self.grid_rows as f32 * 0.5) * 2.5;
                // If culling disabled or row near camera
                let visible = !self.culling_enabled || (z_world - cam_z).abs() < 150.0;
                if visible {
                    let first = r * self.grid_cols;
                    rpass.draw_indexed(0..self.cube_index_count, 0, first..first + self.grid_cols);
                    _draws += 1;
                }
            }
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
struct Instance {
    model: [[f32; 4]; 4],
    color: [f32; 3],
    selected: f32,
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

fn create_plane(device: &wgpu::Device, s: f32) -> (wgpu::Buffer, wgpu::Buffer, u32) {
    // A large plane centered at origin on XZ
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

// Instanced pipeline
struct InstIn {
  @location(0) pos: vec3<f32>,
  @location(1) nrm: vec3<f32>,
  // per-instance transform (mat4 split across 4 attrs)
  @location(2) i0: vec4<f32>,
  @location(3) i1: vec4<f32>,
  @location(4) i2: vec4<f32>,
  @location(5) i3: vec4<f32>,
  @location(6) icolor: vec3<f32>,
  @location(7) iselected: f32,
};

struct InstOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) nrm: vec3<f32>,
  @location(1) world: vec3<f32>,
  @location(2) sel: f32,
  @location(3) icolor: vec3<f32>,
};

@vertex
fn vs_inst(input: InstIn) -> InstOut {
  let inst = mat4x4<f32>(input.i0, input.i1, input.i2, input.i3);
  let world_pos = (model_u.model * inst * vec4<f32>(input.pos, 1.0)).xyz;
  var out: InstOut;
  out.world = world_pos;
  out.nrm = normalize((model_u.model * inst * vec4<f32>(input.nrm, 0.0)).xyz);
  out.pos = globals.view_proj * vec4<f32>(world_pos, 1.0);
  out.sel = input.iselected;
  out.icolor = input.icolor;
  return out;
}

@fragment
fn fs_inst(in: InstOut) -> @location(0) vec4<f32> {
  let light_dir = normalize(vec3<f32>(0.3, 1.0, 0.4));
  let ndl = max(dot(in.nrm, light_dir), 0.0);
  var base = in.icolor * (0.2 + 0.8 * ndl) + model_u.emissive;
  if (in.sel > 0.5) {
    base = vec3<f32>(1.0, 1.0, 0.1);
  }
  return vec4<f32>(base, 1.0);
}
"#;
