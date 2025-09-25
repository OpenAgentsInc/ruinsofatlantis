//! Apply damage for pending hits.

use crate::sim::state::SimState;

pub fn run(state: &mut SimState) {
    let pending = std::mem::take(&mut state.pending_damage);
    for (actor_idx, ability_id, crit) in pending {
        let Some(spec) = state.spells.get(&ability_id) else { continue };
        let Some(dmg) = &spec.damage else { continue };
        // Copy fields we need before mutably borrowing state
        let dice: String = dmg
            .dice_by_level_band
            .as_ref()
            .and_then(|m| m.get("1-4").cloned())
            .unwrap_or_else(|| "1d10".to_string());
        let dmg_type = dmg.damage_type.to_ascii_lowercase();
        drop(spec);
        // Roll
        let mut total = state.roll_dice_str(&dice);
        if crit { total += state.roll_dice_str(&dice); }
        if let Some(tgt_idx) = state.actors[actor_idx].target {
            if !state.actor_alive(actor_idx) || !state.actor_alive(tgt_idx) { continue; }
            if state.are_allies(actor_idx, tgt_idx) { continue; }
            let hp_before = state.actors[tgt_idx].hp;
            // Underwater fire resistance (prototype): halve fire damage
            if state.underwater && dmg_type == "fire" { total = (total / 2).max(0); }
            state.actors[tgt_idx].hp -= total as i32;
            state.log(format!(
                "damage_applied src={} tgt={} ability={} dmg={} hp: {} -> {}",
                state.actors[actor_idx].id,
                state.actors[tgt_idx].id,
                ability_id,
                total,
                hp_before,
                state.actors[tgt_idx].hp
            ));
        }
    }
}
