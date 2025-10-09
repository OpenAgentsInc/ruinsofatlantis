use render_wgpu::gfx::policy::should_draw_actor_lists;

#[test]
fn policy_only_zone_instances_no_actors_is_valid() {
    assert!(should_draw_actor_lists(true, 0));
}

#[test]
fn policy_no_zone_instances_but_one_actor_is_valid() {
    assert!(should_draw_actor_lists(false, 1));
}
