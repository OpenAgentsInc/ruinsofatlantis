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
pub fn create_depth_view(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    _color_format: TextureFormat,
) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth-texture"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}

/// Octahedral encode a unit normal (x,y,z) into 2D.
pub fn oct_encode(n: glam::Vec3) -> glam::Vec2 {
    let mut v = n / (n.x.abs() + n.y.abs() + n.z.abs()).max(1e-8);
    if v.z < 0.0 {
        let x = (1.0 - v.y.abs()) * (if v.x >= 0.0 { 1.0 } else { -1.0 });
        let y = (1.0 - v.x.abs()) * (if v.y >= 0.0 { 1.0 } else { -1.0 });
        v.x = x; v.y = y;
    }
    glam::Vec2::new(v.x, v.y)
}

/// Octahedral decode back to a unit normal.
pub fn oct_decode(e: glam::Vec2) -> glam::Vec3 {
    let mut v = glam::Vec3::new(e.x, e.y, 1.0 - e.x.abs() - e.y.abs());
    if v.z < 0.0 {
        let x = (1.0 - v.y.abs()) * (if v.x >= 0.0 { 1.0 } else { -1.0 });
        let y = (1.0 - v.x.abs()) * (if v.y >= 0.0 { 1.0 } else { -1.0 });
        v.x = x; v.y = y;
    }
    v.normalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn oct_roundtrip_error_bound() {
        let dirs = [
            Vec3::new(1.0, 0.0, 0.0).normalize(),
            Vec3::new(0.0, 1.0, 0.0).normalize(),
            Vec3::new(0.0, 0.0, 1.0).normalize(),
            Vec3::new(1.0, 1.0, 1.0).normalize(),
            Vec3::new(-0.3, 0.7, 0.64).normalize(),
        ];
        for n in dirs.iter() {
            let e = oct_encode(*n);
            let d = oct_decode(e);
            let err = (*n - d).length();
            assert!(err < 1e-3, "oct roundtrip error too high: {}", err);
        }
    }
}
