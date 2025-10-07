use net_core::snapshot::{ActorRep, ActorSnapshot, SnapshotEncode, TAG_ACTOR_SNAPSHOT};

#[test]
fn apply_actor_snapshot_with_sparse_id_does_not_panic() {
    let snap = ActorSnapshot {
        v: 2,
        tick: 1,
        actors: vec![ActorRep {
            id: 100,
            kind: 1,
            team: 2,
            pos: [1.0, 0.6, 2.0],
            yaw: 0.0,
            radius: 0.9,
            hp: 30,
            max: 30,
            alive: true,
        }],
        projectiles: vec![],
    };
    // Encode to bytes
    let mut buf = Vec::new();
    snap.encode(&mut buf);
    assert_eq!(buf.first().copied(), Some(TAG_ACTOR_SNAPSHOT));

    let mut repl = client_core::replication::ReplicationBuffer::default();
    assert!(repl.apply_message(&buf));
    // At least one actor present, with matching id, no panic occurred
    assert!(repl.actors.iter().any(|a| a.id == 100));
}
