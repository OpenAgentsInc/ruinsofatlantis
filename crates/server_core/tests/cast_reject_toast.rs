use glam::vec3;

#[test]
fn fireball_reject_shows_toast_and_spawns_no_projectile() {
    let mut s = server_core::ServerState::new();
    let pc = s.spawn_pc_at(vec3(0.0, 0.6, 0.0));
    // Drain mana to 4 so Fireball (cost ~5) cannot be cast
    {
        let a = s.ecs.get_mut(pc).unwrap();
        let pool = a.pool.as_mut().unwrap();
        pool.mana = 4;
    }
    // Try to cast Fireball
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::SpellId::Fireball,
    );
    s.step_authoritative(0.016);

    // No projectile should be spawned
    let proj_ct = s.ecs.iter().filter(|e| e.projectile.is_some()).count();
    assert_eq!(proj_ct, 0, "cast should be rejected; no projectiles");

    // HUD toast code 1 = Not enough mana must be queued for platform
    assert!(
        s.hud_toasts.iter().any(|&c| c == 1),
        "expected 'Not enough mana' toast (code 1)"
    );
}

