//! Apply buff-type spells without attack/save (e.g., Bless aura).

use crate::sim::state::SimState;
use crate::sim::events::SimEvent;

pub fn run(state: &mut SimState) {
    let completed = state.cast_completed.clone();
    for (actor_idx, ability_id) in completed {
        // Identify Bless
        let is_bless = ability_id.contains("bless")
            || state
                .spells
                .get(&ability_id)
                .map(|s| s.name.eq_ignore_ascii_case("bless"))
                .unwrap_or(false);
        if is_bless {
            // Apply Bless to all non-boss allies for 10s
            for i in 0..state.actors.len() {
                if i == actor_idx {
                    continue;
                }
                if state.actors[i].role != "boss" {
                    state.actors[i].blessed_ms = 10_000;
                }
            }
            state.events.push(SimEvent::BlessApplied { caster: state.actors[actor_idx].id.clone(), duration_ms: 10_000 });
            // Bless is a Concentration spell: starting it ends any existing concentration
            let prev = state.actors[actor_idx]
                .concentration
                .replace(ability_id.clone());
            let _ = prev; // typed events for start/end are covered by ConcentrationBroken + subsequent start events as needed
        }

        // Identify Heroism (grant THP; also Concentration)
        let is_heroism = ability_id.contains("heroism")
            || state
                .spells
                .get(&ability_id)
                .map(|s| s.name.eq_ignore_ascii_case("heroism"))
                .unwrap_or(false);
        if is_heroism {
            // Concentration handling
            let prev = state.actors[actor_idx]
                .concentration
                .replace(ability_id.clone());
            let _ = prev;
            // Grant temporary hit points (prototype amount = 3)
            let current = state.actors[actor_idx].temp_hp;
            let grant = 3;
            let new_thp = current.max(grant);
            state.actors[actor_idx].temp_hp = new_thp;
            // Covered implicitly by HP diff in downstream consumers; omit log
        }
    }
}
