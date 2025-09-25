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
mod util;

use anyhow::Context;
use camera::Camera;
use types::{Globals, Instance, Model};
use util::scale_to_max;

use std::time::Instant;
use wgpu::{rwh::HasDisplayHandle, rwh::HasWindowHandle, util::DeviceExt, SurfaceError, SurfaceTargetUnsafe};
use winit::dpi::PhysicalSize;
use winit::window::Window;

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

    // Geometry
    cube_vb: wgpu::Buffer,
    cube_ib: wgpu::Buffer,
    cube_index_count: u32,
    plane_vb: wgpu::Buffer,
    plane_ib: wgpu::Buffer,
    plane_index_count: u32,

    // Instancing
    instance_buf: wgpu::Buffer,
    grid_cols: u32,
    grid_rows: u32,
    culling_enabled: bool,
    wire_enabled: bool,

    // Time base for animation
    start: Instant,
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
        let (pipeline, inst_pipeline, wire_pipeline) =
            pipeline::create_pipelines(&device, &shader, &globals_bgl, &model_bgl, config.format);

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

        // Geometry
        let (cube_vb, cube_ib, cube_index_count) = mesh::create_cube(&device);
        // Ground size covers the grid and then some
        let grid_cols = 100u32;
        let grid_rows = 100u32;
        let spacing = 2.5f32;
        // Plane size: previously 2.0x the grid footprint; shrink by 25% → 1.5x
        let plane_extent = spacing * (grid_cols.max(grid_rows) as f32) * 1.5;
        let (plane_vb, plane_ib, plane_index_count) = mesh::create_plane(&device, plane_extent);

        // Instances
        let mut instances = Vec::with_capacity((grid_cols * grid_rows) as usize);
        for r in 0..grid_rows {
            for c in 0..grid_cols {
                let x = (c as f32 - grid_cols as f32 * 0.5) * spacing;
                let z = (r as f32 - grid_rows as f32 * 0.5) * spacing;
                let model = glam::Mat4::from_translation(glam::vec3(x, 1.0, z));
                instances.push(Instance { model: model.to_cols_array_2d(), color: [0.85, 0.15, 0.15], selected: 0.0 });
            }
        }
        log::info!("instances: {} ({} x {})", (grid_cols as u64 * grid_rows as u64), grid_cols, grid_rows);
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
            globals_bg,
            plane_model_bg,
            shard_model_bg,

            globals_buf,
            _plane_model_buf: plane_model_buf,
            shard_model_buf,

            cube_vb,
            cube_ib,
            cube_index_count,
            plane_vb,
            plane_ib,
            plane_index_count,

            instance_buf,
            grid_cols,
            grid_rows,
            culling_enabled: true,
            wire_enabled: false,

            start: Instant::now(),
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
        // Slow orbit with elevated camera so the plaza reads clearly
        let cam = Camera::orbit(glam::vec3(0.0, 0.0, 0.0), 40.0, t * 0.1125, aspect);
        let globals = Globals { view_proj: cam.view_proj().to_cols_array_2d(), time_pad: [t, 0.0, 0.0, 0.0] };
        self.queue.write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));

        // Rotate the shard base model a bit for motion
        let shard_mtx = glam::Mat4::from_rotation_y(t) * glam::Mat4::from_translation(glam::vec3(0.0, 1.0, 0.0));
        let shard_model = Model { model: shard_mtx.to_cols_array_2d(), color: [0.85, 0.15, 0.15], emissive: 0.15, _pad: [0.0; 4] };
        self.queue.write_buffer(&self.shard_model_buf, 0, bytemuck::bytes_of(&shard_model));

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

            // Instanced shards
            let inst_pipe = if self.wire_enabled {
                self.wire_pipeline.as_ref().unwrap_or(&self.inst_pipeline)
            } else {
                &self.inst_pipeline
            };
            rpass.set_pipeline(inst_pipe);
            rpass.set_bind_group(0, &self.globals_bg, &[]);
            rpass.set_bind_group(1, &self.shard_model_bg, &[]);
            rpass.set_vertex_buffer(0, self.cube_vb.slice(..));
            rpass.set_vertex_buffer(1, self.instance_buf.slice(..));
            rpass.set_index_buffer(self.cube_ib.slice(..), IndexFormat::Uint16);

            // Very rough CPU culling per row (kept simple for demo)
            let cam_z = cam.eye.z;
            for r in 0..self.grid_rows {
                let z_world = (r as f32 - self.grid_rows as f32 * 0.5) * 2.5;
                let visible = !self.culling_enabled || (z_world - cam_z).abs() < 150.0;
                if visible {
                    let first = r * self.grid_cols;
                    rpass.draw_indexed(0..self.cube_index_count, 0, first..first + self.grid_cols);
                }
            }
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}
