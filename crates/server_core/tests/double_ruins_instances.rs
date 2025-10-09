#![allow(clippy::unwrap_used)]

use glam::vec3;

#[test]
fn adding_two_demo_ruins_registers_two_instances() {
    let mut s = server_core::ServerState::new();
    server_core::scene_build::add_demo_ruins_destructible(&mut s);
    server_core::scene_build::add_demo_ruins_destructible_at(&mut s, vec3(0.0, 0.0, 16.0), 2);

    assert!(
        s.destruct_instances.len() >= 2,
        "expected two destructible instances"
    );
    let ids: Vec<_> = s.destruct_instances.iter().map(|d| d.did).collect();
    assert!(ids.contains(&1) && ids.contains(&2));
}
