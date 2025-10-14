#[test]
fn bg_cache_capacity_is_reasonable() {
    // Ensure default BG cache capacity stays large enough to avoid churn.
    assert!(render_wgpu::gfx::config::BG_CACHE_CAP >= 1024);
}
