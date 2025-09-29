//! Begin cast validation for simple spellcasts.
//! For the prototype, attempt to start a cast for actors that are Idle and
//! have a known ability. Cast times and GCD are pulled from loaded SpellSpecs.

use crate::combat::fsm::ActionState;
use crate::sim::events::SimEvent;
use crate::sim::state::{ActorSim, SimState};
use data_runtime::ids::Id;

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
        // Respect per-ability cooldowns
        if state.actors[idx]
            .ability_cooldowns
            .get(&first)
            .copied()
            .unwrap_or(0)
            > 0
        {
            continue;
        }
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
                // Begin per-ability cooldown immediately upon cast start
                let cd_ms = (spec.cooldown_s * 1000.0) as u32;
                if cd_ms > 0 {
                    a.ability_cooldowns.insert(first.clone(), cd_ms);
                }
            }
            a.id.clone()
        };
        if started {
            state.events.push(SimEvent::CastStarted {
                actor: actor_id,
                ability: first.clone(),
                cast_ms,
                gcd_ms,
            });
        }
    }
}
