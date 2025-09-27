//! G-Buffer attachments and pass scaffolding.
//!
//! This module defines formats and textures for a deferred G-Buffer and exposes
//! helpers to create/resize the attachments. The actual draw integration will
//! be wired in the renderer once materials/pipelines opt-in.

use wgpu::{Device, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor};

/// Formats for the initial G-Buffer (all linear).
pub mod formats {
    use wgpu::TextureFormat;
    pub const ALBEDO: TextureFormat = TextureFormat::Rgba8Unorm; // linear
    // Oct-encoded normal in snorm keeps [-1,1] compact.
    pub const NORMAL_OCT: TextureFormat = TextureFormat::Rg16Snorm;
    // Packed roughness/metalness
    pub const ROUGH_METAL: TextureFormat = TextureFormat::Rg8Unorm;
    // Optional HDR emissive target
    pub const EMISSIVE_HDR: TextureFormat = TextureFormat::Rgba16Float;
    // Motion vectors (signed, may exceed [-1,1])
    pub const MOTION: TextureFormat = TextureFormat::Rg16Float;
}

/// G-Buffer attachments created per-frame resolution.
pub struct GBuffer {
    size: (u32, u32),
    pub albedo: Texture,
    pub albedo_view: TextureView,
    pub normal_oct: Texture,
    pub normal_view: TextureView,
    pub rough_metal: Texture,
    pub rough_metal_view: TextureView,
    pub emissive: Texture,
    pub emissive_view: TextureView,
    pub motion: Texture,
    pub motion_view: TextureView,
}

impl GBuffer {
    pub fn create(device: &Device, width: u32, height: u32) -> Self {
        fn make(device: &Device, w: u32, h: u32, fmt: TextureFormat, label: &str) -> (Texture, TextureView) {
            let tex = device.create_texture(&TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: fmt,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = tex.create_view(&TextureViewDescriptor::default());
            (tex, view)
        }
        let (albedo, albedo_view) = make(device, width, height, formats::ALBEDO, "gbuf-albedo");
        let (normal_oct, normal_view) = make(device, width, height, formats::NORMAL_OCT, "gbuf-normal");
        let (rough_metal, rough_metal_view) = make(device, width, height, formats::ROUGH_METAL, "gbuf-rough-metal");
        let (emissive, emissive_view) = make(device, width, height, formats::EMISSIVE_HDR, "gbuf-emissive");
        let (motion, motion_view) = make(device, width, height, formats::MOTION, "gbuf-motion");
        Self { size: (width, height), albedo, albedo_view, normal_oct, normal_view, rough_metal, rough_metal_view, emissive, emissive_view, motion, motion_view }
    }

    pub fn size(&self) -> (u32, u32) { self.size }
}

