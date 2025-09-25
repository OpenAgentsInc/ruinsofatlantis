use anyhow::Context;
use wgpu::SurfaceError;
use winit::dpi::PhysicalSize;
use winit::window::Window;

pub struct WgpuState<'w> {
    surface: wgpu::Surface<'w>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
}

impl<'w> WgpuState<'w> {
    pub async fn new(window: &'w Window) -> anyhow::Result<Self> {
        let size = window.inner_size();

        // Instance/Surface
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window)
            .context("create wgpu surface")?;

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
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("wgpu-device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::Performance,
                    trace: wgpu::Trace::default(),
                },
            )
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

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size,
        })
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
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

        // Clear to an "Atlantis blue" gradient-ish by two passes is overkill; just clear for now.
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("clear-encoder"),
            });
        {
            use wgpu::{Color, LoadOp, Operations, RenderPassColorAttachment, RenderPassDescriptor, StoreOp};
            let _rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("clear-pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color { r: 0.02, g: 0.08, b: 0.16, a: 1.0 }),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}
