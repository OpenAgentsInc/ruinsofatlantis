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

#[inline]
fn ptr_id<T>(r: &T) -> u32 {
    (r as *const T as usize as u64) as u32
}

impl Renderer {
    fn batch_begin(&mut self) {
        self.main_batch_prev_key = None;
        self.main_batch_count_curr = 0;
    }

    fn batch_add_key_ids(&mut self, pid: u32, mid: u32, mesh_ref_id: u32) {
        let key = [pid, mid, mesh_ref_id];
        match self.main_batch_prev_key {
            Some(prev) if prev == key => {}
            _ => {
                self.main_batch_prev_key = Some(key);
                self.main_batch_count_curr = self.main_batch_count_curr.saturating_add(1);
            }
        }
    }

    fn batch_end(&mut self) {
        self.main_batch_count_last = self.main_batch_count_curr;
    }
    pub(crate) fn set_pending_frame(&mut self, frame: wgpu::SurfaceTexture) {
        self.pending_frame = Some(frame);
    }

    pub(crate) fn take_pending_frame(&mut self) -> Option<wgpu::SurfaceTexture> {
        self.pending_frame.take()
    }
    /// Main scene pass that renders into the provided color/depth views instead of attachments.
    /// Behavior matches `pass_main` aside from not performing the SceneRead blit.
    pub(crate) fn pass_main_to_views(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        depth_view: Option<&wgpu::TextureView>,
    ) {
        self.batch_begin();
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("main-pass(graph)"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: depth_view.map(|dv| wgpu::RenderPassDepthStencilAttachment {
                view: dv,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.main_draw_into(&mut rp);
        // pass ends when rp goes out of scope
        self.batch_end();
    }

    /// Draw the main scene content into an already-open render pass.
    pub(crate) fn main_draw_into<'rp>(&mut self, rp: &mut wgpu::RenderPass<'rp>) {
        let pc_debug = std::env::var("RA_PC_DEBUG")
            .map(|v| v == "1")
            .unwrap_or(false);
        #[cfg(not(target_arch = "wasm32"))]
        let mut pop_scope = |label: &str, dev: &wgpu::Device| -> bool {
            if let Some(e) = pollster::block_on(dev.pop_error_scope()) {
                log::error!("main pass: {}: {:?}", label, e);
                return true;
            }
            false
        };
        #[cfg(target_arch = "wasm32")]
        let mut pop_scope = |_label: &str, _dev: &wgpu::Device| -> bool { false };
        // Terrain (if enabled)
        if std::env::var("RA_DRAW_TERRAIN")
            .map(|v| v != "0")
            .unwrap_or(true)
            && !self.is_picker_batches()
        {
            #[cfg(not(target_arch = "wasm32"))]
            self.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let pid = ptr_id(&self.pipeline);
            let mid = 0;
            let mesh = ptr_id(&self.terrain_ib);
            rp.set_pipeline(&self.pipeline);
            self.pipeline_binds_count = self.pipeline_binds_count.saturating_add(1);
            rp.set_bind_group(0, &self.globals_bg, &[]);
            self.bg_binds_count = self.bg_binds_count.saturating_add(1);
            rp.set_bind_group(1, &self.terrain_model_bg, &[]);
            self.bg_binds_count = self.bg_binds_count.saturating_add(1);
            rp.set_vertex_buffer(0, self.terrain_vb.slice(..));
            rp.set_index_buffer(self.terrain_ib.slice(..), wgpu::IndexFormat::Uint16);
            self.vb_ib_sets_count = self.vb_ib_sets_count.saturating_add(1);
            rp.draw_indexed(0..self.terrain_index_count, 0, 0..1);
            self.draw_calls += 1;
            self.batch_add_key_ids(pid, mid, mesh);
            if pop_scope("terrain", &self.device) {
                return;
            }
        }
        // Ruins (instanced static)
        if self.ruins_count > 0 && !self.is_picker_batches() {
            #[cfg(not(target_arch = "wasm32"))]
            self.device.push_error_scope(wgpu::ErrorFilter::Validation);
            {
                let inst_pipe = if self.wire_enabled {
                    self.wire_pipeline.as_ref().unwrap_or(&self.inst_pipeline)
                } else {
                    &self.inst_pipeline
                };
                let pid = ptr_id(inst_pipe);
                rp.set_pipeline(inst_pipe);
                self.pipeline_binds_count = self.pipeline_binds_count.saturating_add(1);
                // bind and draw
                rp.set_bind_group(0, &self.globals_bg, &[]);
                self.bg_binds_count = self.bg_binds_count.saturating_add(1);
                rp.set_bind_group(1, &self.shard_model_bg, &[]);
                self.bg_binds_count = self.bg_binds_count.saturating_add(1);
                rp.set_vertex_buffer(0, self.ruins_vb.slice(..));
                rp.set_vertex_buffer(1, self.ruins_instances.slice(..));
                rp.set_index_buffer(self.ruins_ib.slice(..), wgpu::IndexFormat::Uint16);
                self.vb_ib_sets_count = self.vb_ib_sets_count.saturating_add(1);
                rp.draw_indexed(0..self.ruins_index_count, 0, 0..self.ruins_count);
                self.draw_calls += 1;
                let mid = 0;
                let mesh = ptr_id(&self.ruins_ib);
                self.batch_add_key_ids(pid, mid, mesh);
            }
            if pop_scope("ruins", &self.device) {
                return;
            }
        }
        // Voxel meshes
        if !self.voxel_meshes.is_empty() && !pc_debug {
            #[cfg(not(target_arch = "wasm32"))]
            self.device.push_error_scope(wgpu::ErrorFilter::Validation);
            rp.set_pipeline(&self.pipeline);
            self.pipeline_binds_count = self.pipeline_binds_count.saturating_add(1);
            rp.set_bind_group(0, &self.globals_bg, &[]);
            self.bg_binds_count = self.bg_binds_count.saturating_add(1);
            rp.set_bind_group(1, &self.voxel_model_bg, &[]);
            self.bg_binds_count = self.bg_binds_count.saturating_add(1);
            let mut voxel_keys: Vec<[u32; 3]> = Vec::new();
            for m in self.voxel_meshes.values() {
                let pid = ptr_id(&self.pipeline);
                let mid = ptr_id(&self.voxel_model_bg);
                let mesh = ptr_id(&m.ib);
                voxel_keys.push([pid, mid, mesh]);
                rp.set_vertex_buffer(0, m.vb.slice(..));
                rp.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                self.vb_ib_sets_count = self.vb_ib_sets_count.saturating_add(1);
                rp.draw_indexed(0..m.idx, 0, 0..1);
                self.draw_calls += 1;
            }
            for [pid, mid, mesh] in voxel_keys.into_iter() {
                self.batch_add_key_ids(pid, mid, mesh);
            }
            if pop_scope("voxels", &self.device) {
                return;
            }
        }
        // Trees (instanced static mesh; textured pipeline for UV support)
        if self.trees_count > 0 && !self.is_picker_batches() {
            #[cfg(not(target_arch = "wasm32"))]
            self.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let inst_pipe = &self.inst_tex_pipeline;
            let pid = ptr_id(inst_pipe);
            rp.set_pipeline(inst_pipe);
            self.pipeline_binds_count = self.pipeline_binds_count.saturating_add(1);
            rp.set_bind_group(0, &self.globals_bg, &[]);
            self.bg_binds_count = self.bg_binds_count.saturating_add(1);
            rp.set_bind_group(1, &self.shard_model_bg, &[]);
            self.bg_binds_count = self.bg_binds_count.saturating_add(1);
            // Palettes group (unused by this pipeline, but layout expects it)
            rp.set_bind_group(2, &self.palettes_bg, &[]);
            // Material group (base_tex/base_sam); fall back to default white
            // Material group is only valid for the textured pipeline; skip here
            rp.set_vertex_buffer(0, self.trees_vb.slice(..));
            rp.set_vertex_buffer(1, self.trees_instances.slice(..));
            rp.set_index_buffer(self.trees_ib.slice(..), wgpu::IndexFormat::Uint16);
            self.vb_ib_sets_count = self.vb_ib_sets_count.saturating_add(1);
            rp.draw_indexed(0..self.trees_index_count, 0, 0..self.trees_count);
            self.draw_calls += 1;
            let mid = ptr_id(&self.shard_model_bg);
            let mesh = ptr_id(&self.trees_ib);
            self.batch_add_key_ids(pid, mid, mesh);
            if pop_scope("trees", &self.device) {
                return;
            }
        }
        // Rocks (instanced static mesh)
        if self.rocks_count > 0 && !self.is_picker_batches() {
            #[cfg(not(target_arch = "wasm32"))]
            self.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let inst_pipe = if self.wire_enabled {
                self.wire_pipeline.as_ref().unwrap_or(&self.inst_pipeline)
            } else {
                &self.inst_pipeline
            };
            let pid = ptr_id(inst_pipe);
            rp.set_pipeline(inst_pipe);
            self.pipeline_binds_count = self.pipeline_binds_count.saturating_add(1);
            rp.set_bind_group(0, &self.globals_bg, &[]);
            self.bg_binds_count = self.bg_binds_count.saturating_add(1);
            rp.set_bind_group(1, &self.shard_model_bg, &[]);
            self.bg_binds_count = self.bg_binds_count.saturating_add(1);
            // Material group is only valid for the textured pipeline; skip here
            rp.set_vertex_buffer(0, self.rocks_vb.slice(..));
            rp.set_vertex_buffer(1, self.rocks_instances.slice(..));
            rp.set_index_buffer(self.rocks_ib.slice(..), wgpu::IndexFormat::Uint16);
            self.vb_ib_sets_count = self.vb_ib_sets_count.saturating_add(1);
            rp.draw_indexed(0..self.rocks_index_count, 0, 0..self.rocks_count);
            self.draw_calls += 1;
            let mid = ptr_id(&self.shard_model_bg);
            let mesh = ptr_id(&self.rocks_ib);
            self.batch_add_key_ids(pid, mid, mesh);
            if pop_scope("rocks", &self.device) {
                return;
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
                let pid = ptr_id(&self.wizard_pipeline);
                let mid = ptr_id(&self.wizard_mat_bg);
                let mesh = ptr_id(&self.wizard_ib);
                self.draw_pc_only(rp);
                self.draw_calls += 1;
                self.batch_add_key_ids(pid, mid, mesh);
            }
        } else if !self.has_zone_batches()
            && !pc_debug
            && std::env::var("RA_DRAW_WIZARDS")
                .map(|v| v != "0")
                .unwrap_or(true)
        {
            #[cfg(not(target_arch = "wasm32"))]
            self.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let pid = ptr_id(&self.wizard_pipeline);
            let mid = ptr_id(&self.wizard_mat_bg);
            let mesh = ptr_id(&self.wizard_ib);
            self.draw_wizards(rp);
            self.draw_calls += 1;
            self.batch_add_key_ids(pid, mid, mesh);
            if self.pc_vb.is_some() {
                let pid = ptr_id(&self.wizard_pipeline);
                let mid = ptr_id(&self.wizard_mat_bg);
                let mesh = ptr_id(&self.wizard_ib);
                self.draw_pc_only(rp);
                self.draw_calls += 1;
                self.batch_add_key_ids(pid, mid, mesh);
            }
            if pop_scope("wizards", &self.device) {
                return;
            }
        }
        // DK, Sorceress, Zombies
        if self.dk_count > 0
            && !self.is_vox_onepath()
            && !self.has_zone_batches()
            && self.repl_buf.boss_status.is_some()
        {
            let pid = ptr_id(&self.wizard_pipeline);
            let mid = ptr_id(&self.dk_mat_bg);
            let mesh = ptr_id(&self.dk_ib);
            self.draw_deathknight(rp);
            self.draw_calls += 1;
            self.batch_add_key_ids(pid, mid, mesh);
        }
        if self.sorc_count > 0 && !self.is_vox_onepath() && !self.has_zone_batches() {
            let pid = ptr_id(&self.wizard_pipeline);
            let mid = ptr_id(&self.sorc_mat_bg);
            let mesh = ptr_id(&self.sorc_ib);
            self.draw_sorceress(rp);
            self.draw_calls += 1;
            self.batch_add_key_ids(pid, mid, mesh);
        }
        if !self.is_vox_onepath()
            && !self.has_zone_batches()
            && std::env::var("RA_DRAW_ZOMBIES")
                .map(|v| v != "0")
                .unwrap_or(true)
        {
            let pid = ptr_id(&self.wizard_pipeline);
            let mid = ptr_id(&self.zombie_mat_bg);
            let mesh = ptr_id(&self.zombie_ib);
            self.draw_zombies(rp);
            self.draw_calls += 1;
            self.batch_add_key_ids(pid, mid, mesh);
        }
        // pass ends when rp goes out of scope
        self.batch_end();
    }
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
        // Track batches conservatively across draws (pipeline, material, mesh)
        self.batch_begin();
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
            let pid = ptr_id(&self.pipeline);
            let mid = 0;
            let mesh = ptr_id(&self.terrain_ib);
            rp.set_pipeline(&self.pipeline);
            self.pipeline_binds_count = self.pipeline_binds_count.saturating_add(1);
            rp.set_bind_group(0, &self.globals_bg, &[]);
            self.bg_binds_count = self.bg_binds_count.saturating_add(1);
            rp.set_bind_group(1, &self.terrain_model_bg, &[]);
            self.bg_binds_count = self.bg_binds_count.saturating_add(1);
            rp.set_vertex_buffer(0, self.terrain_vb.slice(..));
            rp.set_index_buffer(self.terrain_ib.slice(..), wgpu::IndexFormat::Uint16);
            self.vb_ib_sets_count = self.vb_ib_sets_count.saturating_add(1);
            rp.draw_indexed(0..self.terrain_index_count, 0, 0..1);
            self.draw_calls += 1;
            self.batch_add_key_ids(pid, mid, mesh);
        }
        // Ruins (instanced static)
        if self.ruins_count > 0 && !self.is_picker_batches() {
            {
                let inst_pipe = if self.wire_enabled {
                    self.wire_pipeline.as_ref().unwrap_or(&self.inst_pipeline)
                } else {
                    &self.inst_pipeline
                };
                let pid = ptr_id(inst_pipe);
                rp.set_pipeline(inst_pipe);
                self.pipeline_binds_count = self.pipeline_binds_count.saturating_add(1);
                // bind and draw
                rp.set_bind_group(0, &self.globals_bg, &[]);
                self.bg_binds_count = self.bg_binds_count.saturating_add(1);
                rp.set_bind_group(1, &self.shard_model_bg, &[]);
                self.bg_binds_count = self.bg_binds_count.saturating_add(1);
                rp.set_vertex_buffer(0, self.ruins_vb.slice(..));
                rp.set_vertex_buffer(1, self.ruins_instances.slice(..));
                rp.set_index_buffer(self.ruins_ib.slice(..), wgpu::IndexFormat::Uint16);
                self.vb_ib_sets_count = self.vb_ib_sets_count.saturating_add(1);
                rp.draw_indexed(0..self.ruins_index_count, 0, 0..self.ruins_count);
                self.draw_calls += 1;
                let mid = 0;
                let mesh = ptr_id(&self.ruins_ib);
                self.batch_add_key_ids(pid, mid, mesh);
            }
        }
        // Voxel meshes
        if !self.voxel_meshes.is_empty() && !pc_debug {
            rp.set_pipeline(&self.pipeline);
            self.pipeline_binds_count = self.pipeline_binds_count.saturating_add(1);
            rp.set_bind_group(0, &self.globals_bg, &[]);
            self.bg_binds_count = self.bg_binds_count.saturating_add(1);
            rp.set_bind_group(1, &self.voxel_model_bg, &[]);
            self.bg_binds_count = self.bg_binds_count.saturating_add(1);
            let mut voxel_keys: Vec<[u32; 3]> = Vec::new();
            for m in self.voxel_meshes.values() {
                let pid = ptr_id(&self.pipeline);
                let mid = ptr_id(&self.voxel_model_bg);
                let mesh = ptr_id(&m.ib);
                voxel_keys.push([pid, mid, mesh]);
                rp.set_vertex_buffer(0, m.vb.slice(..));
                rp.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                self.vb_ib_sets_count = self.vb_ib_sets_count.saturating_add(1);
                rp.draw_indexed(0..m.idx, 0, 0..1);
                self.draw_calls += 1;
            }
            for [pid, mid, mesh] in voxel_keys.into_iter() {
                self.batch_add_key_ids(pid, mid, mesh);
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
                let pid = ptr_id(&self.wizard_pipeline);
                let mid = ptr_id(&self.wizard_mat_bg);
                let mesh = ptr_id(&self.wizard_ib);
                self.draw_pc_only(&mut rp);
                self.draw_calls += 1;
                self.batch_add_key_ids(pid, mid, mesh);
            }
        } else if !self.has_zone_batches()
            && !pc_debug
            && std::env::var("RA_DRAW_WIZARDS")
                .map(|v| v != "0")
                .unwrap_or(true)
        {
            let pid = ptr_id(&self.wizard_pipeline);
            let mid = ptr_id(&self.wizard_mat_bg);
            let mesh = ptr_id(&self.wizard_ib);
            self.draw_wizards(&mut rp);
            self.draw_calls += 1;
            self.batch_add_key_ids(pid, mid, mesh);
            if self.pc_vb.is_some() {
                let pid = ptr_id(&self.wizard_pipeline);
                let mid = ptr_id(&self.wizard_mat_bg);
                let mesh = ptr_id(&self.wizard_ib);
                self.draw_pc_only(&mut rp);
                self.draw_calls += 1;
                self.batch_add_key_ids(pid, mid, mesh);
            }
        }
        // DK, Sorceress, Zombies
        if self.dk_count > 0
            && !self.is_vox_onepath()
            && !self.has_zone_batches()
            && self.repl_buf.boss_status.is_some()
        {
            let pid = ptr_id(&self.wizard_pipeline);
            let mid = ptr_id(&self.dk_mat_bg);
            let mesh = ptr_id(&self.dk_ib);
            self.draw_deathknight(&mut rp);
            self.draw_calls += 1;
            self.batch_add_key_ids(pid, mid, mesh);
        }
        if self.sorc_count > 0 && !self.is_vox_onepath() && !self.has_zone_batches() {
            let pid = ptr_id(&self.wizard_pipeline);
            let mid = ptr_id(&self.sorc_mat_bg);
            let mesh = ptr_id(&self.sorc_ib);
            self.draw_sorceress(&mut rp);
            self.draw_calls += 1;
            self.batch_add_key_ids(pid, mid, mesh);
        }
        if !self.is_vox_onepath()
            && !self.has_zone_batches()
            && std::env::var("RA_DRAW_ZOMBIES")
                .map(|v| v != "0")
                .unwrap_or(true)
        {
            let pid = ptr_id(&self.wizard_pipeline);
            let mid = ptr_id(&self.zombie_mat_bg);
            let mesh = ptr_id(&self.zombie_ib);
            self.draw_zombies(&mut rp);
            self.draw_calls += 1;
            self.batch_add_key_ids(pid, mid, mesh);
        }
        // Legacy blit removed; post chain handles blit/copies
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

    pub(crate) fn pass_ssr(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        if !self.enable_ssr {
            return;
        }
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ssr-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
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

    pub(crate) fn pass_ssgi(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        if !self.enable_ssgi {
            return;
        }
        let mut gi = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ssgi-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
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

    pub(crate) fn pass_ao(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        if !self.enable_post_ao {
            return;
        }
        let mut post = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("post-ao-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
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

    pub(crate) fn pass_bloom(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
    ) {
        if !self.enable_bloom {
            return;
        }
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("bloom-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
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
        rp.set_pipeline(&self.bloom_pipeline);
        rp.set_bind_group(0, &self.bloom_bg, &[]);
        rp.draw(0..3, 0..1);
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

    /// Copy current HDR (scene_view) into history_color via a fullscreen blit.
    /// Uses the existing blit pipeline and present bind group that samples HDR.
    pub(crate) fn pass_copy_hdr_to_history(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("copy-hdr-to-history"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.attachments.history_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        rp.set_pipeline(&self.blit_scene_read_pipeline);
        rp.set_bind_group(0, &self.present_bg, &[]);
        rp.draw(0..3, 0..1);
    }
}
