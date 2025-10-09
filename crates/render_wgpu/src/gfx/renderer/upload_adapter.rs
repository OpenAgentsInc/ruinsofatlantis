//! Adapter to implement client_core::upload::MeshUpload for the Renderer.
//!
//! This bridges replication-applied CPU chunk meshes into GPU buffers using
//! the local `voxel_upload` helper.

use crate::gfx::Renderer;

impl client_core::upload::MeshUpload for Renderer {
    fn upload_chunk_mesh(
        &mut self,
        did: u64,
        chunk: (u32, u32, u32),
        mesh: &client_core::upload::ChunkMeshEntry,
    ) {
        // Compute/deserialization is in world space. Adjust Y so the chunk rests on terrain.
        let id = crate::gfx::DestructibleId(did as usize);
        let y_off = *self.destruct_y_offset.get(&id).unwrap_or(&f32::NAN);
        let offset = if y_off.is_nan() {
            // Sample terrain at chunk center; compute min Y of incoming positions
            let mut min_y = f32::INFINITY;
            let mut avg_x = 0.0f32;
            let mut avg_z = 0.0f32;
            let n = mesh.positions.len().max(1) as f32;
            for p in &mesh.positions {
                min_y = min_y.min(p[1]);
                avg_x += p[0] / n;
                avg_z += p[2] / n;
            }
            let (h, _n) = crate::gfx::terrain::height_at(&self.terrain_cpu, avg_x, avg_z);
            let off = (h - min_y).max(-10.0).min(10.0); // clamp to sane range
            self.destruct_y_offset.insert(id, off);
            off
        } else {
            y_off
        };
        let mut positions = mesh.positions.clone();
        if offset.abs() > 1e-5 {
            for p in &mut positions {
                p[1] += offset;
            }
        }
        let cpu = ecs_core::components::MeshCpu {
            positions,
            normals: mesh.normals.clone(),
            indices: mesh.indices.clone(),
        };
        let _ = crate::gfx::renderer::voxel_upload::upload_chunk_mesh(
            &self.device,
            id,
            chunk,
            &cpu,
            &mut self.voxel_meshes,
            &mut self.voxel_hashes,
        );
        // Hide static ruins once we begin receiving voxel chunks for any DID,
        // unless explicitly forced on via RA_SHOW_STATIC_RUINS.
        let keep_statics = std::env::var("RA_SHOW_STATIC_RUINS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if !keep_statics {
            self.ruins_count = 0;
        }
    }
    fn remove_chunk_mesh(&mut self, did: u64, chunk: (u32, u32, u32)) {
        crate::gfx::renderer::voxel_upload::remove_chunk_mesh(
            crate::gfx::DestructibleId(did as usize),
            chunk,
            &mut self.voxel_meshes,
            &mut self.voxel_hashes,
        );
    }
}
