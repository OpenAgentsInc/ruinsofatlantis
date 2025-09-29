//! Renderer passes split out of the monolithic render() for readability.
//! These helpers are invoked from render() incrementally as we refactor.
#![allow(dead_code)] // staged extraction; called progressively during renderer split

use crate::gfx::Renderer;

impl Renderer {
    pub(crate) fn pass_build_hiz(&self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(hiz) = &self.hiz {
            let znear = 0.1f32;
            let zfar = 1000.0f32;
            hiz.build_mips(
                &self.device,
                encoder,
                &self.attachments.depth_view,
                &self._post_sampler,
                znear,
                zfar,
            );
        }
    }

    pub(crate) fn pass_blit_scene_read(&self, encoder: &mut wgpu::CommandEncoder) {
        if !(self.enable_ssgi || self.enable_ssr) {
            return;
        }
        let mut blit = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("blit-scene-to-read"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.attachments.scene_read_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        blit.set_pipeline(&self.blit_scene_read_pipeline);
        blit.set_bind_group(0, &self.present_bg, &[]);
        blit.draw(0..3, 0..1);
    }

    pub(crate) fn pass_ssr(&self, encoder: &mut wgpu::CommandEncoder) {
        if !self.enable_ssr {
            return;
        }
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ssr-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.attachments.scene_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        rp.set_pipeline(&self.ssr_pipeline);
        rp.set_bind_group(0, &self.ssr_depth_bg, &[]);
        rp.set_bind_group(1, &self.ssr_scene_bg, &[]);
        rp.draw(0..3, 0..1);
    }

    pub(crate) fn pass_ssgi(&self, encoder: &mut wgpu::CommandEncoder) {
        if !self.enable_ssgi {
            return;
        }
        let mut gi = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ssgi-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.attachments.scene_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        gi.set_pipeline(&self.ssgi_pipeline);
        gi.set_bind_group(0, &self.ssgi_globals_bg, &[]);
        gi.set_bind_group(1, &self.ssgi_depth_bg, &[]);
        gi.set_bind_group(2, &self.ssgi_scene_bg, &[]);
        gi.draw(0..3, 0..1);
    }

    pub(crate) fn pass_ao(&self, encoder: &mut wgpu::CommandEncoder) {
        if !self.enable_post_ao {
            return;
        }
        let mut post = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("post-ao-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.attachments.scene_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        post.set_pipeline(&self.post_ao_pipeline);
        post.set_bind_group(0, &self.globals_bg, &[]);
        post.set_bind_group(1, &self.post_ao_bg, &[]);
        post.draw(0..3, 0..1);
    }

    pub(crate) fn pass_present(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        swap_view: &wgpu::TextureView,
    ) {
        let mut present = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("present-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: swap_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        present.set_pipeline(&self.present_pipeline);
        present.set_bind_group(0, &self.present_bg, &[]);
        present.draw(0..3, 0..1);
    }
}
