use net_core::snapshot::{
    ActorRep, ActorSnapshot, ProjectileRep, SnapshotEncode, TAG_ACTOR_SNAPSHOT,
};

#[test]
fn v2_snapshot_with_projectile_populates_replication() {
    let snap = ActorSnapshot {
        v: 2,
        tick: 1,
        actors: vec![ActorRep {
            id: 1,
            kind: 0,
            team: 0,
            pos: [0.0, 0.6, 0.0],
            yaw: 0.0,
            radius: 0.7,
            hp: 100,
            max: 100,
            alive: true,
        }],
        projectiles: vec![ProjectileRep {
            id: 42,
            kind: 0, // Firebolt
            pos: [1.0, 0.6, 2.0],
            vel: [0.0, 0.0, 10.0],
        }],
    };
    let mut buf = Vec::new();
    snap.encode(&mut buf);
    assert_eq!(buf.first().copied(), Some(TAG_ACTOR_SNAPSHOT));

    let mut repl = client_core::replication::ReplicationBuffer::default();
    assert!(repl.apply_message(&buf));
    assert_eq!(repl.projectiles.len(), 1);
    assert_eq!(repl.projectiles[0].id, 42);
    assert_eq!(repl.projectiles[0].kind, 0);
}
