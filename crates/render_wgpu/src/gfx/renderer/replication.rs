//! Replication bridge helpers: apply deltas and upload chunk meshes.
//!
//! This module is CPU-only and does not depend on wgpu, so we can unit test
//! the delta->upload logic independent of the renderer's GPU state.

use client_core::replication::ReplicationBuffer;
use client_core::upload::MeshUpload;

/// Apply a batch of serialized messages and issue mesh uploads for any decoded
/// chunk mesh deltas.
#[allow(dead_code)]
pub fn apply_deltas_and_upload<T: MeshUpload>(
    target: &mut T,
    repl: &mut ReplicationBuffer,
    msgs: &[Vec<u8>],
) {
    for b in msgs {
        let _ = repl.apply_message(b);
    }
    for (did, chunk, entry) in repl.drain_mesh_updates() {
        target.upload_chunk_mesh(did, chunk, &entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use client_core::upload::ChunkMeshEntry;

    struct DummyUploader {
        uploads: usize,
        last: Option<(u64, (u32, u32, u32), ChunkMeshEntry)>,
    }
    impl MeshUpload for DummyUploader {
        fn upload_chunk_mesh(&mut self, did: u64, chunk: (u32, u32, u32), mesh: &ChunkMeshEntry) {
            self.uploads += 1;
            self.last = Some((did, chunk, mesh.clone()));
        }
        fn remove_chunk_mesh(&mut self, _did: u64, _chunk: (u32, u32, u32)) {}
    }

    #[test]
    fn bridge_uploads_from_decoded_messages() {
        use net_core::snapshot::{ChunkMeshDelta, SnapshotEncode};
        let delta = ChunkMeshDelta {
            did: 9,
            chunk: (4, 5, 6),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            indices: vec![0, 1, 2],
        };
        let mut buf = Vec::new();
        delta.encode(&mut buf);
        let mut repl = ReplicationBuffer::default();
        let mut up = DummyUploader {
            uploads: 0,
            last: None,
        };
        apply_deltas_and_upload(&mut up, &mut repl, &[buf]);
        assert_eq!(up.uploads, 1);
        let (did, chunk, entry) = up.last.unwrap();
        assert_eq!(did, 9);
        assert_eq!(chunk, (4, 5, 6));
        assert_eq!(entry.indices, vec![0, 1, 2]);
    }
}
