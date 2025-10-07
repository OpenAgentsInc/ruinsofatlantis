use glam::vec3;

#[test]
fn npc_wizard_chooses_fireball_when_clustered_targets() {
    let mut s = server_core::ServerState::new();
    // Spawn NPC wizard at origin with casting resources
    let wid = s.spawn_wizard_npc(vec3(0.0, 0.6, 0.0));
    // Spawn two undead clustered around z=14..15m
    let _u1 = s.spawn_undead(vec3(0.2, 0.6, 14.5), 0.9, 30);
    let _u2 = s.spawn_undead(vec3(-0.3, 0.6, 15.2), 0.9, 30);
    // Run one tick to allow AI to decide and cast
    s.step_authoritative(0.2, &[]);
    // Ingest happens in same frame; assert at least one projectile of kind Fireball exists
    let mut saw_fb = false;
    for c in s.ecs.iter() {
        if let (Some(p), Some(_)) = (c.projectile.as_ref(), c.velocity.as_ref()) {
            if matches!(p.kind, server_core::ProjKind::Fireball) {
                saw_fb = true;
                break;
            }
        }
    }
    assert!(
        saw_fb,
        "expected a Fireball projectile when clustered targets present"
    );
}
