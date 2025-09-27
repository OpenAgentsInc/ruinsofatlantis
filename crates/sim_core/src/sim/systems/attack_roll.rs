//! Resolve attack rolls for newly completed casts.

use crate::rules::attack::Advantage;
use crate::sim::state::SimState;

pub fn run(state: &mut SimState) {
    // For each completed cast, perform an attack roll if the spec defines one
    let completed = std::mem::take(&mut state.cast_completed);
    for (actor_idx, ability_id) in completed {
        // Gather immutable info before any mutable borrows of state
        let (has_attack, crit_on_nat20) = if let Some(spec) = state.spells.get(&ability_id) {
            if let Some(att) = &spec.attack {
                (true, att.crit_rule.as_deref() == Some("nat20_double_dice"))
            } else {
                (false, false)
            }
        } else {
            (false, false)
        };
        if has_attack {
            if !state.actor_alive(actor_idx) {
                continue;
            }
            // Skip hostile resolution against allies
            if let Some(tgt_idx) = state.actors[actor_idx].target {
                if state.are_allies(actor_idx, tgt_idx) {
                    let actor_id = state.actors[actor_idx].id.clone();
                    let tgt_id = state.actors[tgt_idx].id.clone();
                    state.log(format!(
                        "ally_immunity actor={} -> tgt={} ability={} (skipped)",
                        actor_id, tgt_id, ability_id
                    ));
                    continue;
                }
                if !state.actor_alive(tgt_idx) {
                    continue;
                }
            }
            let target_ac_initial = state.target_ac(actor_idx).unwrap_or(12);
            let (roll, nat20) = state.roll_d20(Advantage::Normal);
            let mut bonus = state.actors[actor_idx].spell_attack_bonus;
            // Bless adds 1d4 to attacks if active
            if state.actors[actor_idx].blessed_ms > 0 {
                bonus += state.roll_dice_str("1d4");
            }
            let total = roll + bonus;
            let mut target_ac = target_ac_initial;
            let would_hit = total >= target_ac;
            // Reaction: Shield (+5 AC) if target has shield and reaction ready and would be hit
            if let Some(tgt_idx) = state.actors[actor_idx].target
                && would_hit
                && state.actors[tgt_idx].reaction_ready
                && state.actors[tgt_idx]
                    .ability_ids
                    .iter()
                    .any(|s| s.contains("shield"))
            {
                state.actors[tgt_idx].ac_temp_bonus += 5;
                state.actors[tgt_idx].reaction_ready = false;
                target_ac += 5;
                state.log(format!(
                    "shield_reaction tgt={} +5 AC -> {}",
                    state.actors[tgt_idx].id, target_ac
                ));
            }
            let hit = total >= target_ac;
            let actor_id = state.actors[actor_idx].id.clone();
            state.log(format!(
                "attack_resolved actor={} ability={} d20={} + {} = {} vs AC{} => {}",
                actor_id,
                ability_id,
                roll,
                bonus,
                total,
                target_ac,
                if hit { "HIT" } else { "MISS" }
            ));
            if hit {
                state
                    .pending_damage
                    .push((actor_idx, ability_id.clone(), crit_on_nat20 && nat20));
            }
        } else {
            state
                .pending_damage
                .push((actor_idx, ability_id.clone(), false));
        }
    }
}
