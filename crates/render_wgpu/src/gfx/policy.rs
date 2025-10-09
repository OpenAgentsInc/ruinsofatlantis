/// Simple draw policy helpers kept GPU-free for unit testing.
#[inline]
pub fn should_draw_actor_lists(zone_has_instances: bool, replicated_actor_count: usize) -> bool {
    replicated_actor_count > 0 || zone_has_instances
}
