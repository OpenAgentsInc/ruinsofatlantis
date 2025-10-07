use glam::vec3;

#[test]
fn magic_missile_from_wizard_damages_undead() {
    let mut s = server_core::ServerState::new();
    let wid = s.ecs.spawn(
        server_core::ActorKind::Wizard,
        server_core::Team::Wizards,
        server_core::Transform { pos: vec3(0.0, 0.6, 0.0), yaw: 0.0, radius: 0.7 },
        server_core::Health { hp: 100, max: 100 },
    );
    if let Some(w) = s.ecs.get_mut(wid) {
        use std::collections::HashMap;
        w.pool = Some(server_core::ecs::ResourcePool { mana: 50, max: 50, regen_per_s: 0.0 });
        w.cooldowns = Some(server_core::ecs::Cooldowns { gcd_s: 0.0, gcd_ready: 0.0, per_spell: HashMap::new() });
        w.spellbook = Some(server_core::ecs::Spellbook { known: vec![server_core::SpellId::MagicMissile] });
    }
    let uid = s.spawn_undead(vec3(0.0, 0.6, 8.0), 0.9, 40);
    let hp0 = s.ecs.get(uid).unwrap().hp.hp;
    s.pending_casts.push(server_core::CastCmd { pos: vec3(0.0, 0.6, 0.0), dir: vec3(0.0, 0.0, 1.0), spell: server_core::SpellId::MagicMissile, caster: Some(wid) });
    for _ in 0..8 { s.step_authoritative(0.05, &[]); }
    let hp1 = s.ecs.get(uid).unwrap().hp.hp;
    assert!(hp1 < hp0, "undead HP should drop after MagicMissile ({} -> {})", hp0, hp1);
}

