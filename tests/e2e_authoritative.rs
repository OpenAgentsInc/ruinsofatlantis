use glam::vec3;
use server_core::{ProjKind, ServerState};

/// End-to-end authoritative loopback:
/// - Spawn a PC-owned Fireball toward an NPC wizard
/// - Step the server once (authoritative)
/// - Assert wizard HP drops, projectile is removed, hostility flips
/// - Build a TickSnapshot and assert it reflects the HP drop and no Fireball remains
#[test]
fn e2e_pc_fireball_damages_wizard_and_removes_projectile() {
    let mut s = ServerState::new();

    // Ensure at least two wizards exist via sync (PC + one NPC wizard)
    let wiz_pos = vec![vec3(0.0, 0.6, 0.0), vec3(2.0, 0.6, 0.0)];
    s.sync_wizards(&wiz_pos);
    // Normalize HP and adjust target wizard
    s.wizards[0].hp = 100;
    s.wizards[1].hp = 80;

    // Spawn Fireball from PC aimed at the NPC wizard and step once
    s.spawn_projectile_from_dir_owned(
        vec3(-1.5, 0.6, 0.0),
        vec3(1.0, 0.0, 0.0),
        ProjKind::Fireball,
        Some(1),
    );
    s.step_authoritative(0.1, &wiz_pos);

    // Fireball should detonate and be removed
    assert!(
        s.projectiles.is_empty(),
        "Fireball must be removed after detonation"
    );
    // Wizard took damage
    assert!(s.wizards[1].hp < 80, "Wizard should lose HP from Fireball");
    // NPC wizards flip hostile after PC damages a wizard
    assert!(
        s.wizards_hostile_to_pc,
        "NPC wizards should flip hostile to PC after PC damages a wizard"
    );

    // Snapshot reflects HP drop and carries current state
    let snap = s.tick_snapshot(123);
    assert!(snap.wizards.len() >= 2);
    let w1 = snap
        .wizards
        .iter()
        .find(|w| w.id == 2)
        .expect("wizard id=2 present");
    assert!(w1.hp < 80, "Snapshot must carry updated wizard HP");
    // After detonation, there must be no lingering Fireball in the snapshot
    assert!(
        snap.projectiles.iter().all(|p| p.kind != 1),
        "No lingering Fireball in snapshot"
    );
}
