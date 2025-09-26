//! Begin cast validation for simple spellcasts.
//! For the prototype, attempt to start a cast for actors that are Idle and
//! have a known ability. Cast times and GCD are pulled from loaded SpellSpecs.

use crate::core::combat::fsm::ActionState;
use crate::core::data::ids::Id;
use crate::sim::state::{ActorSim, SimState};

pub fn run(state: &mut SimState) {
    let dt_ms = state.tick_ms;
    // Tick GCDs first
    for a in &mut state.actors {
        a.gcd.tick(dt_ms);
    }
    // Try to start casts on idle actors
    for idx in 0..state.actors.len() {
        let start_ability: Option<(String, usize)> = {
            let a = &state.actors[idx];
            if !matches!(a.action, ActionState::Idle) || a.ability_ids.is_empty() {
                None
            } else {
                let i = a.next_ability_idx % a.ability_ids.len();
                Some((a.ability_ids[i].clone(), i))
            }
        };
        let Some((first, sel_idx)) = start_ability else {
            continue;
        };
        // Load spec if needed
        if !state.spells.contains_key(&first) {
            if let Ok(spec) = state.load_spell(&first) {
                state.spells.insert(first.clone(), spec);
            } else if first == "basic_attack" {
                let spec = SimState::builtin_basic_attack_spec();
                state.spells.insert(first.clone(), spec);
            } else if first == "boss.tentacle" {
                let spec = SimState::builtin_boss_tentacle_spec();
                state.spells.insert(first.clone(), spec);
            } else {
                continue;
            }
        }
        let spec = state.spells.get(&first).unwrap();
        // Cast time / GCD in ms
        let cast_ms = (spec.cast_time_s * 1000.0) as u32;
        let gcd_ms = (spec.gcd_s * 1000.0) as u32;
        // Mutate actor in a dedicated scope, then log
        let mut started = false;
        let actor_id = {
            let a: &mut ActorSim = &mut state.actors[idx];
            let id = Id(first.clone());
            if let Ok(new_state) = a
                .action
                .clone()
                .try_start_cast(id, cast_ms, &mut a.gcd, gcd_ms)
            {
                a.action = new_state;
                started = true;
                a.next_ability_idx = sel_idx.wrapping_add(1);
            }
            a.id.clone()
        };
        if started {
            state.log(format!(
                "cast_started actor={} ability={} cast_ms={} gcd_ms={}",
                actor_id, first, cast_ms, gcd_ms
            ));
        }
    }
}
