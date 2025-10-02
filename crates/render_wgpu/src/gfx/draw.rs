//! Draw helpers: methods on Renderer for specific passes.

use wgpu::IndexFormat;

use super::Renderer;

impl Renderer {
    pub(crate) fn draw_pc_only(&self, rpass: &mut wgpu::RenderPass<'_>) {
        if self.wizard_index_count == 0 {
            return;
        }
        let stride = std::mem::size_of::<crate::gfx::types::InstanceSkin>() as u64;
        let offset = (self.pc_index as u64) * stride;
        rpass.set_pipeline(&self.wizard_pipeline);
        rpass.set_bind_group(0, &self.globals_bg, &[]);
        rpass.set_bind_group(1, &self.shard_model_bg, &[]);
        rpass.set_bind_group(2, &self.palettes_bg, &[]);
        rpass.set_bind_group(3, &self.wizard_mat_bg, &[]);
        rpass.set_vertex_buffer(0, self.wizard_vb.slice(..));
        rpass.set_vertex_buffer(1, self.wizard_instances.slice(offset..offset + stride));
        rpass.set_index_buffer(self.wizard_ib.slice(..), IndexFormat::Uint16);
        rpass.draw_indexed(0..self.wizard_index_count, 0, 0..1);
    }
    pub(crate) fn draw_wizards(&self, rpass: &mut wgpu::RenderPass<'_>) {
        rpass.set_pipeline(&self.wizard_pipeline);
        rpass.set_bind_group(0, &self.globals_bg, &[]);
        rpass.set_bind_group(1, &self.shard_model_bg, &[]);
        rpass.set_bind_group(2, &self.palettes_bg, &[]);
        rpass.set_bind_group(3, &self.wizard_mat_bg, &[]);
        rpass.set_vertex_buffer(0, self.wizard_vb.slice(..));
        rpass.set_vertex_buffer(1, self.wizard_instances.slice(..));
        rpass.set_index_buffer(self.wizard_ib.slice(..), IndexFormat::Uint16);
        rpass.draw_indexed(0..self.wizard_index_count, 0, 0..self.wizard_count);
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
}
