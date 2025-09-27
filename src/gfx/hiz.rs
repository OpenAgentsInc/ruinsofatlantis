//! Hi-Z (Z-MAX) pyramid over linearized depth.
//!
//! This module declares the structures and shader entry names to build a Z-MAX
//! mip chain from an R32F linear depth texture. The actual compute dispatch is
//! wired by the renderer when the pass executes.

use wgpu::{
    Device, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
};

/// Hi-Z resources: linear depth copy + mip chain views for Z-MAX reduction.
#[allow(dead_code)]
pub struct HiZPyramid {
    pub linear_depth: Texture,
    pub linear_view: TextureView,
    pub mip_views: Vec<TextureView>,
    pub width: u32,
    pub height: u32,
}

impl HiZPyramid {
    pub fn create(device: &Device, width: u32, height: u32) -> Self {
        let tex = device.create_texture(&TextureDescriptor {
            label: Some("linear-depth-r32f"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: Self::mip_count(width, height),
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R32Float,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::STORAGE_BINDING
                | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let linear_view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let mut mip_views = Vec::new();
        let mips = Self::mip_count(width, height);
        for i in 0..mips {
            mip_views.push(tex.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: i,
                mip_level_count: Some(1),
                ..Default::default()
            }));
        }
        Self {
            linear_depth: tex,
            linear_view,
            mip_views,
            width,
            height,
        }
    }

    #[inline]
    pub fn mip_count(w: u32, h: u32) -> u32 {
        let max_dim = w.max(h).max(1);
        32 - max_dim.leading_zeros()
    }
}

/// CPU reference downsample for tests: produces mip1 as max over 2x2 pixels from mip0.
#[allow(dead_code)]
pub fn zmax_downsample_2x2(src: &[f32], w: usize, h: usize) -> Vec<f32> {
    let mw = (w / 2).max(1);
    let mh = (h / 2).max(1);
    let mut dst = vec![0.0_f32; mw * mh];
    for y in 0..mh {
        for x in 0..mw {
            let x0 = (2 * x).min(w - 1);
            let y0 = (2 * y).min(h - 1);
            let ix = |xx: usize, yy: usize| -> usize { yy * w + xx };
            let a = src[ix(x0, y0)];
            let b = src[ix(x0.min(w - 1), (y0 + 1).min(h - 1))];
            let c = src[ix((x0 + 1).min(w - 1), y0)];
            let d = src[ix((x0 + 1).min(w - 1), (y0 + 1).min(h - 1))];
            dst[y * mw + x] = a.max(b).max(c).max(d);
        }
    }
    dst
}

#[cfg(test)]
mod tests {
    use super::zmax_downsample_2x2;

    #[test]
    fn zmax_4x4_two_planes() {
        // 4x4: top half depth=1.0, bottom half depth=5.0 → mip1 rows: [1, 5]
        let w = 4;
        let h = 4;
        let mut src = vec![1.0_f32; w * h];
        for y in 2..4 {
            for x in 0..4 {
                src[y * w + x] = 5.0;
            }
        }
        let mip1 = zmax_downsample_2x2(&src, w, h);
        assert_eq!(mip1.len(), (w / 2) * (h / 2));
        // y=0 row should be 1.0; y=1 row should be 5.0
        assert!((mip1[0] - 1.0).abs() < 1e-6);
        assert!((mip1[1] - 1.0).abs() < 1e-6);
        assert!((mip1[2] - 5.0).abs() < 1e-6);
        assert!((mip1[3] - 5.0).abs() < 1e-6);
    }
}
