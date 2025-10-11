//! Device/surface wrappers: typed shells over raw wgpu objects.
//!
//! Phase-one scaffolding: introduce `GpuCtx`, `SurfaceCtx`, and `Samplers`
//! as thin wrappers to improve cohesion and prepare for later refactors.
//! No behavior change; callers may continue to use raw fields during transition.

use std::sync::Arc;

/// GPU context: device/queue/adapter and reported capabilities.
pub struct GpuCtx {
    pub device: Arc<wgpu::Device>,
    pub queue: wgpu::Queue,
    pub adapter: wgpu::Adapter,
    pub limits: wgpu::Limits,
    pub features: wgpu::Features,
}

impl GpuCtx {
    /// Construct from existing parts. Transition helper (no probing logic here).
    pub fn from_parts(
        device: wgpu::Device,
        queue: wgpu::Queue,
        adapter: wgpu::Adapter,
        limits: wgpu::Limits,
        features: wgpu::Features,
    ) -> Self {
        Self {
            device: Arc::new(device),
            queue,
            adapter,
            limits,
            features,
        }
    }
}

/// Swapchain/surface configuration and current size.
pub struct SurfaceCtx {
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
}

impl SurfaceCtx {
    /// Construct from existing parts.
    pub fn from_parts(
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        size: winit::dpi::PhysicalSize<u32>,
    ) -> Self {
        Self {
            surface,
            config,
            size,
        }
    }
}

/// Common samplers owned once by the renderer.
pub struct Samplers {
    pub linear: wgpu::Sampler,
    pub nearest: wgpu::Sampler,
}

impl Samplers {
    pub fn new(device: &wgpu::Device) -> Self {
        let linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("nearest"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        Self { linear, nearest }
    }
}
