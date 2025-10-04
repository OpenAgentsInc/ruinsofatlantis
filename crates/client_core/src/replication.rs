//! Client replication scaffolding.
//!
//! Responsibilities
//! - Buffer incoming snapshot deltas
//! - Apply to client ECS/state
//! - Invalidate GPU uploads for changed chunks
//!
//! Filled in later when net_core types are finalized.

/// Opaque replication buffer (placeholder).
use net_core::snapshot::SnapshotDecode;

#[derive(Default, Debug)]
pub struct ReplicationBuffer {
    pub updated_chunks: usize,
    pending_mesh: Vec<(u64, (u32, u32, u32), crate::upload::ChunkMeshEntry)>,
}

impl ReplicationBuffer {
    /// Apply a raw message. Returns whether any state changed.
    pub fn apply_message(&mut self, bytes: &[u8]) -> bool {
        let mut slice: &[u8] = bytes;
        if let Ok(delta) = net_core::snapshot::ChunkMeshDelta::decode(&mut slice) {
            let entry = crate::upload::ChunkMeshEntry {
                positions: delta.positions,
                normals: delta.normals,
                indices: delta.indices,
            };
            self.pending_mesh.push((delta.did, delta.chunk, entry));
            self.updated_chunks += 1;
            true
        } else {
            false
        }
    }

    /// Drain pending mesh updates accumulated from replication into a vector
    /// of (did, chunk, entry). Renderer or host applies uploads via `MeshUpload`.
    pub fn drain_mesh_updates(
        &mut self,
    ) -> Vec<(u64, (u32, u32, u32), crate::upload::ChunkMeshEntry)> {
        let mut v = Vec::new();
        std::mem::swap(&mut v, &mut self.pending_mesh);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn buffer_default_is_empty() {
        let b = ReplicationBuffer::default();
        assert_eq!(b.updated_chunks, 0);
    }
}
