//! Resolve attack rolls for newly completed casts.

use crate::core::rules::attack::Advantage;
use crate::sim::state::SimState;

pub fn run(state: &mut SimState) {
    // For each completed cast, perform an attack roll if the spec defines one
    let completed = std::mem::take(&mut state.cast_completed);
    for (actor_idx, ability_id) in completed {
        // Gather immutable info before any mutable borrows of state
        let (has_attack, crit_on_nat20) = if let Some(spec) = state.spells.get(&ability_id) {
            if let Some(att) = &spec.attack {
                (true, att.crit_rule.as_deref() == Some("nat20_double_dice"))
            } else { (false, false) }
        } else { (false, false) };
        if has_attack {
            let target_ac = state.target_ac(actor_idx).unwrap_or(12);
            let (roll, nat20) = state.roll_d20(Advantage::Normal);
            let bonus = state.actors[actor_idx].spell_attack_bonus;
            let total = roll + bonus;
            let hit = total >= target_ac;
            let actor_id = state.actors[actor_idx].id.clone();
            state.log(format!("attack_resolved actor={} ability={} d20={} + {} = {} vs AC{} => {}", actor_id, ability_id, roll, bonus, total, target_ac, if hit {"HIT"} else {"MISS"}));
            if hit { state.pending_damage.push((actor_idx, ability_id.clone(), crit_on_nat20 && nat20)); }
        } else {
            state.pending_damage.push((actor_idx, ability_id.clone(), false));
        }
    }
}
