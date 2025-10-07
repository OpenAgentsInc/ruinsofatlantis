use glam::Vec3;

#[test]
fn corpse_with_despawn_after_is_removed_when_timer_elapses() {
    let mut s = server_core::ServerState::new();
    s.sync_wizards(&[Vec3::new(0.0, 0.6, 0.0)]);
    let z = s.spawn_undead(Vec3::new(0.5, 0.6, 0.5), 0.9, 10);

    // Kill via burning; server sets DespawnAfter { seconds: 2.0 } on death
    {
        let a = s.ecs.get_mut(z).unwrap();
        a.apply_burning(100, 0.1, None); // die next tick
    }

    // First step should apply damage and set despawn timer
    let wiz0: Vec<Vec3> = s
        .ecs
        .iter()
        .filter(|a| matches!(a.kind, server_core::ActorKind::Wizard) && a.team == server_core::Team::Pc)
        .map(|a| a.tr.pos)
        .collect();
    s.step_authoritative(0.1, &wiz0);
    assert!(s.ecs.get(z).is_some(), "present during despawn delay");

    // Step until just before 2.0s
    for _ in 0..18 {
        let wiz: Vec<Vec3> = s
            .ecs
            .iter()
            .filter(|a| matches!(a.kind, server_core::ActorKind::Wizard) && a.team == server_core::Team::Pc)
            .map(|a| a.tr.pos)
            .collect();
        s.step_authoritative(0.1, &wiz);
    }
    assert!(s.ecs.get(z).is_some(), "still present before timer elapses");

    // Step past the timer
    for _ in 0..3 {
        let wiz: Vec<Vec3> = s
            .ecs
            .iter()
            .filter(|a| matches!(a.kind, server_core::ActorKind::Wizard) && a.team == server_core::Team::Pc)
            .map(|a| a.tr.pos)
            .collect();
        s.step_authoritative(0.1, &wiz);
    }
    assert!(s.ecs.get(z).is_none(), "removed after timer elapses");
}
