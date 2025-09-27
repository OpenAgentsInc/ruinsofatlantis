//! ux_hud: HUD logic/state with simple toggles.
//!
//! Owns runtime HUD switches and produces lightweight data that a renderer UI
//! module can consume.

#[derive(Debug, Clone)]
pub struct HudModel {
    perf_enabled: bool,
    hud_enabled: bool,
}

impl Default for HudModel {
    fn default() -> Self {
        Self {
            perf_enabled: false,
            hud_enabled: true,
        }
    }
}

impl HudModel {
    pub fn toggle_perf(&mut self) {
        self.perf_enabled = !self.perf_enabled;
    }
    pub fn toggle_hud(&mut self) {
        self.hud_enabled = !self.hud_enabled;
    }
    pub fn perf_enabled(&self) -> bool {
        self.perf_enabled
    }
    pub fn hud_enabled(&self) -> bool {
        self.hud_enabled
    }

    // Placeholder for deriving draw data from sim + renderer stats
    #[allow(unused_variables)]
    pub fn update_from<T, U>(&mut self, sim: &T, stats: &U) {
        let _ = (sim, stats);
    }
}

/// Placeholder flattened draw data for the renderer
pub struct HudDraw;

#[cfg(test)]
mod tests {
    use super::HudModel;

    #[test]
    fn toggles_default_and_flip() {
        let mut m = HudModel::default();
        assert!(m.hud_enabled());
        assert!(!m.perf_enabled());
        m.toggle_perf();
        assert!(m.perf_enabled());
        m.toggle_hud();
        assert!(!m.hud_enabled());
    }
}
