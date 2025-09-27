//! ux_hud: HUD logic/state (placeholder)
//!
//! For now, this crate is a stub to establish the API boundary.
//! Future work will move HUD state here; renderer-only draw code stays in render_wgpu.

/// Placeholder HUD model
pub struct HudModel;
/// Placeholder flattened draw data for the renderer
pub struct HudDraw;

impl HudModel {
    #[allow(unused_variables)]
    pub fn update_from<T, U>(sim: &T, stats: &U) {
        // no-op placeholder
    }
}
