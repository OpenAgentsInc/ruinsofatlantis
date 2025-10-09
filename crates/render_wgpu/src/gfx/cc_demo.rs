use super::Renderer;

/// Character Controller Demo: trim the scene to a single PC on flat green ground.
/// This keeps the client presentation-only and avoids modifying gameplay code paths.
pub fn apply_cc_demo(r: &mut Renderer) {
    // Hide everything except terrain + PC
    r.wizard_count = 1;
    r.zombie_count = 0;
    r.ruins_count = 0;
    r.trees_count = 0;
    r.rocks_count = 0;
    r.dk_count = 0;
    r.sorc_count = 0;
    // Ensure PC occupies slot 0 for simplicity
    r.pc_index = 0;
    // Zero out zombie buffers to avoid drawing
    // Palettes buffers can remain allocated; counts gate the draw.
    // Force greensward material (already used) and keep sky.
}
