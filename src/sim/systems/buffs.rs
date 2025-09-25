//! Apply buff-type spells without attack/save (e.g., Bless aura).

use crate::sim::state::SimState;

pub fn run(state: &mut SimState) {
    let completed = state.cast_completed.clone();
    for (actor_idx, ability_id) in completed {
        // Identify Bless
        let is_bless = ability_id.contains("bless") || state.spells.get(&ability_id).map(|s| s.name.to_ascii_lowercase() == "bless").unwrap_or(false);
        if is_bless {
            // Apply Bless to all non-boss allies for 10s
            for i in 0..state.actors.len() {
                if i == actor_idx { continue; }
                if state.actors[i].role != "boss" { state.actors[i].blessed_ms = 10_000; }
            }
            state.log(format!("bless_applied by={} dur_ms=10000", state.actors[actor_idx].id));
        }
    }
}

