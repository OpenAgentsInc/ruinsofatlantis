//! Small helpers used across the renderer.

use wgpu::TextureFormat;

/// Clamp `width`/`height` to `max_dim` while preserving aspect ratio.
pub fn scale_to_max((w0, h0): (u32, u32), max_dim: u32) -> (u32, u32) {
    let (mut w, mut h) = (w0.max(1), h0.max(1));
    if w > max_dim || h > max_dim {
        let scale = (w as f32 / max_dim as f32).max(h as f32 / max_dim as f32);
        w = ((w as f32 / scale).floor() as u32).clamp(1, max_dim);
        h = ((h as f32 / scale).floor() as u32).clamp(1, max_dim);
    }
    (w, h)
}

/// Create a depth texture view sized to the current surface.
pub fn create_depth_view(device: &wgpu::Device, width: u32, height: u32, _color_format: TextureFormat) -> wgpu::TextureView {
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

