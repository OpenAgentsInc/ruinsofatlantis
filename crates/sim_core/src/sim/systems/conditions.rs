//! Apply pending statuses and tick durations.

use crate::sim::state::SimState;
use crate::sim::events::SimEvent;

pub fn run(state: &mut SimState) {
    // Apply pending
    let add = std::mem::take(&mut state.pending_status);
    for (idx, cond, dur) in add {
        state.actors[idx].statuses.push((cond, dur));
        state.events.push(SimEvent::ConditionApplied { target: state.actors[idx].id.clone(), condition: format!("{:?}", cond), duration_ms: dur });
    }
    // Tick durations and drop expired
    for a in &mut state.actors {
        for s in &mut a.statuses {
            s.1 = s.1.saturating_sub(state.tick_ms);
        }
        a.statuses.retain(|s| s.1 > 0);
        if a.blessed_ms > 0 {
            a.blessed_ms = a.blessed_ms.saturating_sub(state.tick_ms);
        }
    }
}
