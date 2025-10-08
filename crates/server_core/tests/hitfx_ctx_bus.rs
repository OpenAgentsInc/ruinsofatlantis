use glam::vec3;

#[test]
fn hitfx_flows_through_ctx_and_accumulates() {
    let mut s = server_core::ServerState::new();
    let _pc = s.spawn_pc_at(vec3(0.0, 0.6, 0.0));
    let wid = s.spawn_wizard_npc(vec3(2.0, 0.6, 0.0));
    // Fire a Firebolt toward the NPC wizard
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(1.0, 0.0, 0.0),
        server_core::SpellId::Firebolt,
    );
    // Step a few frames; collision should generate a HitFx
    for _ in 0..8 {
        s.step_authoritative(0.05);
    }
    assert!(
        !s.fx_hits.is_empty(),
        "expected server_state.fx_hits to be populated after a direct hit"
    );
    // At least one hit at roughly the target area
    let ok = s.fx_hits.iter().any(|h| (h.pos[0] - 2.0).abs() < 2.0);
    assert!(ok);
    let _ = wid; // silence unused if optimized out
}
