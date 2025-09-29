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
    // Rebuild attachments in one place
    let sc_fmt = r.config.format;
    let offscreen = wgpu::TextureFormat::Rgba16Float;
    r.attachments.swapchain_format = sc_fmt;
    r.attachments.offscreen_format = offscreen;
    r.attachments
        .rebuild(&r.device, r.config.width, r.config.height);

    // Rebuild bind groups referencing resized textures
    r.present_bg = r.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("present-bg"),
        layout: &r.present_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&r.attachments.scene_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&r._post_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&r.attachments.depth_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&r.point_sampler),
            },
        ],
    });
    r.post_ao_bg = r.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("post-ao-bg"),
        layout: &r.post_ao_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&r.attachments.depth_view),
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
                resource: wgpu::BindingResource::TextureView(&r.attachments.depth_view),
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
                resource: wgpu::BindingResource::TextureView(&r.attachments.scene_read_view),
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
                resource: wgpu::BindingResource::TextureView(&r.attachments.scene_read_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&r._post_sampler),
            },
        ],
    });
}
