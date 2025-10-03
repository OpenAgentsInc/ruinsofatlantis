#![cfg(not(any(
    feature = "legacy_client_carve",
    feature = "vox_onepath_demo",
    feature = "destruct_debug"
)))]
#[test]
fn default_build_has_no_mutation_features() {
    // 95A guarantee: default build should be non-mutating on the client.
    assert!(!cfg!(feature = "legacy_client_carve"));
    assert!(!cfg!(feature = "vox_onepath_demo"));
    // Logging for destructibles should be opt-in
    assert!(!cfg!(feature = "destruct_debug"));
}
