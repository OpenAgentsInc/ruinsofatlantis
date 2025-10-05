use client_core::replication::ReplicationBuffer;
use client_core::upload::{ChunkMeshEntry, MeshUpload};
use net_core::{
    channel,
    snapshot::{BossStatusMsg, ChunkMeshDelta, SnapshotEncode},
};

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
fn local_loop_sends_and_applies_chunk_mesh() {
    let (tx, rx) = channel::channel();
    // Server encodes a small delta
    let delta = ChunkMeshDelta {
        did: 7,
        chunk: (1, 2, 3),
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 1.0, 0.0]; 3],
        indices: vec![0, 1, 2],
    };
    let mut buf = Vec::new();
    delta.encode(&mut buf);
    assert!(tx.try_send(buf));

    // Client drains and applies
    let mut repl = ReplicationBuffer::default();
    for b in rx.drain() {
        let _ = repl.apply_message(&b);
    }
    assert_eq!(repl.updated_chunks, 1);

    // Renderer (or host) uploads
    let mut up = DummyUploader {
        uploads: 0,
        last: None,
    };
    for (did, chunk, entry) in repl.drain_mesh_updates() {
        up.upload_chunk_mesh(did, chunk, &entry);
    }
    assert_eq!(up.uploads, 1);
    let last = up.last.unwrap();
    assert_eq!(last.0, 7);
    assert_eq!(last.1, (1, 2, 3));
    assert_eq!(last.2.indices, vec![0, 1, 2]);

    // BossStatus message round-trip
    let bs = BossStatusMsg {
        name: "Nivita".into(),
        ac: 18,
        hp: 220,
        max: 250,
        pos: [0.0, 0.6, 35.0],
    };
    let mut b2 = Vec::new();
    bs.encode(&mut b2);
    let _ = repl.apply_message(&b2);
    assert!(repl.boss_status.is_some());
}
