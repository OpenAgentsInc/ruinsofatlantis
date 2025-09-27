//! Renderer resize split from gfx/mod.rs for readability.

use winit::dpi::PhysicalSize;

use crate::gfx::{Renderer, gbuffer, hiz, util};

pub fn resize_impl(r: &mut Renderer, new_size: PhysicalSize<u32>) {
    if new_size.width == 0 || new_size.height == 0 {
        return;
    }
    let (w, h) = util::scale_to_max((new_size.width, new_size.height), r.max_dim);
    if (w, h) != (new_size.width, new_size.height) {
        log::debug!(
            "Resized {}x{} exceeds max {}, clamped to {}x{} (aspect kept)",
            new_size.width,
            new_size.height,
            r.max_dim,
            w,
            h
        );
    }
    r.size = PhysicalSize::new(w, h);
    r.config.width = w;
    r.config.height = h;
    r.surface.configure(&r.device, &r.config);
    r.depth = util::create_depth_view(&r.device, r.config.width, r.config.height, r.config.format);

    // Recreate SceneColor + SceneRead
    r.scene_color = r.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scene-color"),
        size: wgpu::Extent3d {
            width: r.config.width,
            height: r.config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    r.scene_view = r
        .scene_color
        .create_view(&wgpu::TextureViewDescriptor::default());
    r.scene_read = r.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scene-read"),
        size: wgpu::Extent3d {
            width: r.config.width,
            height: r.config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    r.scene_read_view = r
        .scene_read
        .create_view(&wgpu::TextureViewDescriptor::default());

    // Rebuild bind groups referencing resized textures
    r.present_bg = r.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("present-bg"),
        layout: &r.present_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&r.scene_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&r._post_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&r.depth),
            },
        ],
    });
    r.post_ao_bg = r.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("post-ao-bg"),
        layout: &r.post_ao_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&r.depth),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&r._post_sampler),
            },
        ],
    });
    r.ssgi_depth_bg = r.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ssgi-depth-bg"),
        layout: &r.ssgi_depth_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&r.depth),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&r._post_sampler),
            },
        ],
    });
    r.ssgi_scene_bg = r.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ssgi-scene-bg"),
        layout: &r.ssgi_scene_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&r.scene_read_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&r._post_sampler),
            },
        ],
    });
    // Lighting M1 resources
    r.gbuffer = Some(gbuffer::GBuffer::create(
        &r.device,
        r.config.width,
        r.config.height,
    ));
    r.hiz = Some(hiz::HiZPyramid::create(
        &r.device,
        r.config.width,
        r.config.height,
    ));
    if let Some(h) = &r.hiz {
        r.ssr_depth_bg = r.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ssr-depth-bg"),
            layout: &r.ssr_depth_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&h.linear_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&r.point_sampler),
                },
            ],
        });
    }
    r.ssr_scene_bg = r.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ssr-scene-bg"),
        layout: &r.ssr_scene_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&r.scene_read_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&r._post_sampler),
            },
        ],
    });
}
