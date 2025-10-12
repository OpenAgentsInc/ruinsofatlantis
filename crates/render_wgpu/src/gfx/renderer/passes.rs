//! Renderer passes split out of the monolithic render() for readability.
//! These helpers are invoked from render() incrementally as we refactor.
//! Pass I/O overview (see `renderer::graph` for validation):
//! - sky: writes SceneColor
//! - main: reads Depth, writes SceneColor
//! - blit_scene_to_read: reads SceneColor, writes SceneRead (when not direct-present)
//! - ssr: reads linear Depth + SceneRead, writes SceneColor
//! - ssgi: reads Depth + SceneRead, writes SceneColor
//! - post_ao: reads Depth, writes SceneColor
//! - bloom: reads SceneRead, writes SceneColor
#![allow(dead_code)] // staged extraction; called progressively during renderer split

use crate::gfx::Renderer;

impl Renderer {
    pub(crate) fn pass_main(&mut self, encoder: &mut wgpu::CommandEncoder) {
        // Match legacy behavior: render to offscreen scene_view with optional depth
        let pc_debug = std::env::var("RA_PC_DEBUG")
            .map(|v| v == "1")
            .unwrap_or(false);
        let want_depth = if pc_debug {
            !self.is_picker_batches()
        } else {
            true
        };
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("main-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.attachments.scene_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: if want_depth {
                Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.attachments.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                })
            } else {
                None
            },
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        // Terrain (if enabled)
        if std::env::var("RA_DRAW_TERRAIN")
            .map(|v| v != "0")
            .unwrap_or(true)
            && !self.is_picker_batches()
        {
            rp.set_pipeline(&self.pipeline);
            rp.set_bind_group(0, &self.globals_bg, &[]);
            rp.set_bind_group(1, &self.terrain_model_bg, &[]);
            rp.set_vertex_buffer(0, self.terrain_vb.slice(..));
            rp.set_index_buffer(self.terrain_ib.slice(..), wgpu::IndexFormat::Uint16);
            rp.draw_indexed(0..self.terrain_index_count, 0, 0..1);
            self.draw_calls += 1;
        }
        // Ruins (instanced static)
        if self.ruins_count > 0 && !self.is_picker_batches() {
            let inst_pipe = if self.wire_enabled {
                self.wire_pipeline.as_ref().unwrap_or(&self.inst_pipeline)
            } else {
                &self.inst_pipeline
            };
            rp.set_pipeline(inst_pipe);
            rp.set_bind_group(0, &self.globals_bg, &[]);
            rp.set_bind_group(1, &self.shard_model_bg, &[]);
            rp.set_vertex_buffer(0, self.ruins_vb.slice(..));
            rp.set_vertex_buffer(1, self.ruins_instances.slice(..));
            rp.set_index_buffer(self.ruins_ib.slice(..), wgpu::IndexFormat::Uint16);
            rp.draw_indexed(0..self.ruins_index_count, 0, 0..self.ruins_count);
            self.draw_calls += 1;
        }
        // Voxel meshes
        if !self.voxel_meshes.is_empty() && !pc_debug {
            rp.set_pipeline(&self.pipeline);
            rp.set_bind_group(0, &self.globals_bg, &[]);
            rp.set_bind_group(1, &self.voxel_model_bg, &[]);
            for m in self.voxel_meshes.values() {
                rp.set_vertex_buffer(0, m.vb.slice(..));
                rp.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed(0..m.idx, 0, 0..1);
                self.draw_calls += 1;
            }
        }
        // Wizards and PC
        if self.is_vox_onepath() {
            // skip entirely in demo
        } else if self.has_zone_batches() && !self.is_picker_batches() {
            let pc_ready = self.pc_vb.is_some()
                && self.pc_ib.is_some()
                && self.pc_instances.is_some()
                && self.pc_mat_bg.is_some()
                && self.pc_palettes_bg.is_some()
                && self.pc_index_count > 0;
            if pc_ready {
                self.draw_pc_only(&mut rp);
                self.draw_calls += 1;
            }
        } else if !self.has_zone_batches()
            && !pc_debug
            && std::env::var("RA_DRAW_WIZARDS")
                .map(|v| v != "0")
                .unwrap_or(true)
        {
            self.draw_wizards(&mut rp);
            self.draw_calls += 1;
            if self.pc_vb.is_some() {
                self.draw_pc_only(&mut rp);
                self.draw_calls += 1;
            }
        }
        // DK, Sorceress, Zombies
        if self.dk_count > 0
            && !self.is_vox_onepath()
            && !self.has_zone_batches()
            && self.repl_buf.boss_status.is_some()
        {
            self.draw_deathknight(&mut rp);
            self.draw_calls += 1;
        }
        if self.sorc_count > 0 && !self.is_vox_onepath() && !self.has_zone_batches() {
            self.draw_sorceress(&mut rp);
            self.draw_calls += 1;
        }
        if !self.is_vox_onepath()
            && !self.has_zone_batches()
            && std::env::var("RA_DRAW_ZOMBIES")
                .map(|v| v != "0")
                .unwrap_or(true)
        {
            self.draw_zombies(&mut rp);
            self.draw_calls += 1;
        }
        drop(rp);
        // SceneRead copy for bloom/ssgi if needed
        if !self.direct_present {
            let mut blit = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit-scene-read"),
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
            self.draw_calls += 1;
        }
    }
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
