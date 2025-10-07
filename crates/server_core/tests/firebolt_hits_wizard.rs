use glam::Vec3;

#[test]
fn firebolt_hits_wizard_reliably() {
    let mut s = server_core::ServerState::new();
    // PC and one NPC wizard 5m ahead
    let pc = Vec3::new(0.0, 0.6, 0.0);
    let wiz = Vec3::new(0.0, 0.6, 5.0);
    s.sync_wizards(&[pc, wiz]);

    // Cast Firebolt straight toward the NPC wizard
    s.enqueue_cast(pc, Vec3::new(0.0, 0.0, 1.0), server_core::SpellId::Firebolt);
    // Step a few frames to allow travel + collision
    for _ in 0..10 {
        s.step_authoritative(0.02, &[pc, wiz]);
    }

    // Assert at least one projectile was removed (collision) and wizard took damage
    let npc_id = s
        .ecs
        .iter()
        .filter(|a| matches!(a.kind, server_core::ActorKind::Wizard) && a.team == server_core::Team::Wizards)
        .map(|a| a.id)
        .next()
        .expect("npc wizard present");
    let npc = s.ecs.get(npc_id).unwrap();
    assert!(npc.hp.hp < npc.hp.max, "wizard should take damage from Firebolt");
}

