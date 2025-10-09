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
        let cpu = ecs_core::components::MeshCpu {
            positions: mesh.positions.clone(),
            normals: mesh.normals.clone(),
            indices: mesh.indices.clone(),
        };
        let _ = crate::gfx::renderer::voxel_upload::upload_chunk_mesh(
            &self.device,
            crate::gfx::DestructibleId(did as usize),
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
