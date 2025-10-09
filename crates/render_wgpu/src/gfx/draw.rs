//! Draw helpers: methods on Renderer for specific passes.

use wgpu::IndexFormat;

use super::Renderer;
use crate::gfx::mesh;
use crate::gfx::types::Model;

impl Renderer {
    /// Debug helper: draw a solid cube at a given model transform using the
    /// non‑skinned pipeline. Intended for visibility diagnostics.
    pub(crate) fn draw_debug_cube(&self, rpass: &mut wgpu::RenderPass<'_>, model_m: glam::Mat4) {
        // Create a tiny cube VB/IB on the fly (debug only)
        let (vb, ib, idx) = mesh::create_cube(&self.device);
        // Update the model uniform buffer used by shard_model_bg
        let m = Model {
            model: model_m.to_cols_array_2d(),
            color: [1.0, 0.1, 0.2],
            emissive: 0.0,
            _pad: [0.0; 4],
        };
        self.queue
            .write_buffer(&self.shard_model_buf, 0, bytemuck::bytes_of(&m));
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.globals_bg, &[]);
        rpass.set_bind_group(1, &self.shard_model_bg, &[]);
        rpass.set_vertex_buffer(0, vb.slice(..));
        rpass.set_index_buffer(ib.slice(..), IndexFormat::Uint16);
        rpass.draw_indexed(0..idx, 0, 0..1);
    }
    pub(crate) fn draw_pc_only(&self, rpass: &mut wgpu::RenderPass<'_>) {
        if self.wizard_index_count == 0 {
            return;
        }
        let use_debug = std::env::var("RA_PC_DEBUG").as_deref() == Ok("1");
        if use_debug {
            if let Some(p) = &self.wizard_pipeline_debug {
                rpass.set_pipeline(p);
            } else {
                rpass.set_pipeline(&self.wizard_pipeline);
            }
        } else {
            rpass.set_pipeline(&self.wizard_pipeline);
        }
        rpass.set_bind_group(0, &self.globals_bg, &[]);
        rpass.set_bind_group(1, &self.shard_model_bg, &[]);
        // Prefer PC (UBC) resources when available
        if let (Some(pc_pal_bg), Some(pc_mat_bg), Some(pc_vb), Some(pc_ib), Some(pc_inst)) = (
            self.pc_palettes_bg.as_ref(),
            self.pc_mat_bg.as_ref(),
            self.pc_vb.as_ref(),
            self.pc_ib.as_ref(),
            self.pc_instances.as_ref(),
        ) {
            rpass.set_bind_group(2, pc_pal_bg, &[]);
            rpass.set_bind_group(3, pc_mat_bg, &[]);
            rpass.set_vertex_buffer(0, pc_vb.slice(..));
            rpass.set_vertex_buffer(1, pc_inst.slice(..));
            // UBC male uses 32-bit indices; draw with Uint32 to ensure visibility
            rpass.set_index_buffer(pc_ib.slice(..), IndexFormat::Uint32);
            rpass.draw_indexed(0..self.pc_index_count, 0, 0..1);
            // Debug HUD line to prove the pass ran (will be queued by caller)
        } else {
            // Fallback: draw PC from wizard rig
            let stride = std::mem::size_of::<crate::gfx::types::InstanceSkin>() as u64;
            let offset = (self.pc_index as u64) * stride;
            rpass.set_bind_group(2, &self.palettes_bg, &[]);
            rpass.set_bind_group(3, &self.wizard_mat_bg, &[]);
            rpass.set_vertex_buffer(0, self.wizard_vb.slice(..));
            rpass.set_vertex_buffer(1, self.wizard_instances.slice(offset..offset + stride));
            rpass.set_index_buffer(self.wizard_ib.slice(..), IndexFormat::Uint16);
            rpass.draw_indexed(0..self.wizard_index_count, 0, 0..1);
        }
    }
    pub(crate) fn draw_wizards(&self, rpass: &mut wgpu::RenderPass<'_>) {
        rpass.set_pipeline(&self.wizard_pipeline);
        rpass.set_bind_group(0, &self.globals_bg, &[]);
        rpass.set_bind_group(1, &self.shard_model_bg, &[]);
        rpass.set_bind_group(2, &self.palettes_bg, &[]);
        rpass.set_bind_group(3, &self.wizard_mat_bg, &[]);
        rpass.set_vertex_buffer(0, self.wizard_vb.slice(..));
        rpass.set_index_buffer(self.wizard_ib.slice(..), IndexFormat::Uint16);
        // If PC uses a separate rig, skip the PC instance in the wizard draw by splitting draws
        if self.pc_vb.is_some() {
            let stride = std::mem::size_of::<crate::gfx::types::InstanceSkin>() as u64;
            // Draw [0, pc_index)
            if self.pc_index > 0 {
                let first_count = self.pc_index as u32;
                rpass.set_vertex_buffer(
                    1,
                    self.wizard_instances
                        .slice(..(self.pc_index as u64) * stride),
                );
                rpass.draw_indexed(0..self.wizard_index_count, 0, 0..first_count);
            }
            // Draw (pc_index, wizard_count]
            if (self.pc_index as u32) < self.wizard_count {
                let remain = self.wizard_count.saturating_sub(self.pc_index as u32 + 1);
                if remain > 0 {
                    let off = ((self.pc_index + 1) as u64) * stride;
                    rpass.set_vertex_buffer(1, self.wizard_instances.slice(off..));
                    rpass.draw_indexed(0..self.wizard_index_count, 0, 0..remain);
                }
            }
        } else {
            // No separate PC rig: draw all wizards together
            rpass.set_vertex_buffer(1, self.wizard_instances.slice(..));
            rpass.draw_indexed(0..self.wizard_index_count, 0, 0..self.wizard_count);
        }
    }

    pub(crate) fn draw_particles(&self, rpass: &mut wgpu::RenderPass<'_>) {
        if self.fx_count == 0 {
            return;
        }
        rpass.set_pipeline(&self.particle_pipeline);
        rpass.set_bind_group(0, &self.globals_bg, &[]);
        rpass.set_vertex_buffer(0, self.quad_vb.slice(..));
        rpass.set_vertex_buffer(1, self.fx_instances.slice(..));
        rpass.draw(0..4, 0..self.fx_count);
    }

    pub(crate) fn draw_zombies(&self, rpass: &mut wgpu::RenderPass<'_>) {
        if self.zombie_count == 0 {
            return;
        }
        rpass.set_pipeline(&self.wizard_pipeline);
        rpass.set_bind_group(0, &self.globals_bg, &[]);
        rpass.set_bind_group(1, &self.shard_model_bg, &[]);
        rpass.set_bind_group(2, &self.zombie_palettes_bg, &[]);
        rpass.set_bind_group(3, &self.zombie_mat_bg, &[]);
        rpass.set_vertex_buffer(0, self.zombie_vb.slice(..));
        rpass.set_vertex_buffer(1, self.zombie_instances.slice(..));
        rpass.set_index_buffer(self.zombie_ib.slice(..), IndexFormat::Uint16);
        rpass.draw_indexed(0..self.zombie_index_count, 0, 0..self.zombie_count);
    }

    pub(crate) fn draw_deathknight(&self, rpass: &mut wgpu::RenderPass<'_>) {
        if self.dk_count == 0 {
            return;
        }
        rpass.set_pipeline(&self.wizard_pipeline);
        rpass.set_bind_group(0, &self.globals_bg, &[]);
        rpass.set_bind_group(1, &self.shard_model_bg, &[]);
        rpass.set_bind_group(2, &self.dk_palettes_bg, &[]);
        rpass.set_bind_group(3, &self.dk_mat_bg, &[]);
        rpass.set_vertex_buffer(0, self.dk_vb.slice(..));
        rpass.set_vertex_buffer(1, self.dk_instances.slice(..));
        rpass.set_index_buffer(self.dk_ib.slice(..), IndexFormat::Uint16);
        rpass.draw_indexed(0..self.dk_index_count, 0, 0..self.dk_count);
    }

    pub(crate) fn draw_sorceress(&self, rpass: &mut wgpu::RenderPass<'_>) {
        if self.sorc_count == 0 {
            return;
        }
        rpass.set_pipeline(&self.wizard_pipeline);
        rpass.set_bind_group(0, &self.globals_bg, &[]);
        rpass.set_bind_group(1, &self.shard_model_bg, &[]);
        rpass.set_bind_group(2, &self.sorc_palettes_bg, &[]);
        rpass.set_bind_group(3, &self.sorc_mat_bg, &[]);
        rpass.set_vertex_buffer(0, self.sorc_vb.slice(..));
        rpass.set_vertex_buffer(1, self.sorc_instances.slice(..));
        rpass.set_index_buffer(self.sorc_ib.slice(..), IndexFormat::Uint16);
        rpass.draw_indexed(0..self.sorc_index_count, 0, 0..self.sorc_count);
    }
}
