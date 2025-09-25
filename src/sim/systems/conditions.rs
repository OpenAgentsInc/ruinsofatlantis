//! Apply pending statuses and tick durations.

use crate::sim::state::SimState;

pub fn run(state: &mut SimState) {
    // Apply pending
    let add = std::mem::take(&mut state.pending_status);
    for (idx, cond, dur) in add {
        state.actors[idx].statuses.push((cond, dur));
        state.log(format!("condition_applied tgt={} cond={:?} dur_ms={}", state.actors[idx].id, cond, dur));
    }
    // Tick durations and drop expired
    for a in &mut state.actors {
        for s in &mut a.statuses { s.1 = s.1.saturating_sub(state.tick_ms); }
        a.statuses.retain(|s| s.1 > 0);
    }
}

