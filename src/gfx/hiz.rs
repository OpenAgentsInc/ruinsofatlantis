//! Hi-Z (Z-MAX) pyramid over linearized depth.
//!
//! This module declares the structures and shader entry names to build a Z-MAX
//! mip chain from an R32F linear depth texture. The actual compute dispatch is
//! wired by the renderer when the pass executes.

use wgpu::util::DeviceExt;
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

    /// Build the Z-MAX mip chain using compute shaders over a linear R32F texture.
    ///
    /// - `depth_view` is the current frame's depth attachment view (Depth32Float)
    /// - `sampler` is any basic sampler (nearest is fine here)
    /// - `znear`/`zfar` are used to linearize the depth into mip0
    pub fn build_mips(
        &self,
        device: &Device,
        encoder: &mut wgpu::CommandEncoder,
        depth_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        znear: f32,
        zfar: f32,
    ) {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hiz-comp"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "hiz.comp.wgsl"
            ))),
        });
        // Layouts
        let params_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hiz-params-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    // depth
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Depth,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    // sampler
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    // params UBO
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    // dst mip0 (write)
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let reduce_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hiz-reduce-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    // src mip (sampled)
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    // dst mip (write)
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let params_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hiz-params-pl"),
            bind_group_layouts: &[&params_bgl],
            push_constant_ranges: &[],
        });
        let reduce_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hiz-reduce-pl"),
            bind_group_layouts: &[&reduce_bgl],
            push_constant_ranges: &[],
        });
        let p_linear = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("hiz-linearize"),
            layout: Some(&params_pl),
            module: &shader,
            entry_point: Some("cs_linearize_mip0"),
            compilation_options: Default::default(),
            cache: None,
        });
        let p_reduce = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("hiz-reduce-max"),
            layout: Some(&reduce_pl),
            module: &shader,
            entry_point: Some("cs_downsample_max"),
            compilation_options: Default::default(),
            cache: None,
        });
        // Params UBO
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct Params {
            znear: f32,
            zfar: f32,
            _pad: [f32; 2],
        }
        let params = Params {
            znear,
            zfar,
            _pad: [0.0; 2],
        };
        let params_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("hiz-params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        // Mip0: linearize depth
        let bg0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hiz-linearize-bg"),
            layout: &params_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&self.mip_views[0]),
                },
            ],
        });
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("hiz-linearize-pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&p_linear);
            cpass.set_bind_group(0, &bg0, &[]);
            let gx = self.width.div_ceil(8);
            let gy = self.height.div_ceil(8);
            cpass.dispatch_workgroups(gx, gy, 1);
        }
        // Downsample chain: 2x2 max from mip N-1 to mip N
        for mip in 1..self.mip_views.len() {
            let src = &self.mip_views[mip - 1];
            let dst = &self.mip_views[mip];
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("hiz-reduce-bg"),
                layout: &reduce_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(src),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(dst),
                    },
                ],
            });
            let w = (self.width >> mip).max(1);
            let h = (self.height >> mip).max(1);
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("hiz-reduce-pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&p_reduce);
            cpass.set_bind_group(0, &bg, &[]);
            let half_w = w.div_ceil(2);
            let half_h = h.div_ceil(2);
            let gx = half_w.div_ceil(8); // each thread covers 2x2 of src
            let gy = half_h.div_ceil(8);
            cpass.dispatch_workgroups(gx, gy, 1);
        }
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
