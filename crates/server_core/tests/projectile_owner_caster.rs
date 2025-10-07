use glam::vec3;

#[test]
fn projectile_owner_is_caster() {
    let mut s = server_core::ServerState::new();
    // Spawn a PC (not used as caster here)
    let _pc = s.spawn_pc_at(vec3(0.0, 0.6, -2.0));
    // Spawn a wizard NPC as the caster
    let wid = s.ecs.spawn(
        server_core::ActorKind::Wizard,
        server_core::Team::Wizards,
        server_core::Transform { pos: vec3(0.0, 0.6, 0.0), yaw: 0.0, radius: 0.7 },
        server_core::Health { hp: 100, max: 100 },
    );
    // Ensure casting components
    if let Some(w) = s.ecs.get_mut(wid) {
        use std::collections::HashMap;
        w.pool = Some(server_core::ecs::ResourcePool { mana: 10, max: 10, regen_per_s: 1.0 });
        w.cooldowns = Some(server_core::ecs::Cooldowns { gcd_s: 0.0, gcd_ready: 0.0, per_spell: HashMap::new() });
        w.spellbook = Some(server_core::ecs::Spellbook { known: vec![server_core::SpellId::Firebolt] });
    }
    // Enqueue a cast with explicit caster id
    s.pending_casts.push(server_core::CastCmd {
        pos: vec3(0.0, 0.6, 0.0),
        dir: vec3(0.0, 0.0, 1.0),
        spell: server_core::SpellId::Firebolt,
        caster: Some(wid),
    });
    // Step once to run cast_system and ingest projectiles
    s.step_authoritative(0.016, &[]);
    // Find projectile in ECS and verify owner
    let mut found = false;
    for c in s.ecs.iter() {
        if c.projectile.is_some() {
            assert_eq!(c.owner.map(|o| o.id), Some(wid), "projectile owner must be caster");
            found = true;
            break;
        }
    }
    assert!(found, "expected a projectile spawned by cast_system");
}

