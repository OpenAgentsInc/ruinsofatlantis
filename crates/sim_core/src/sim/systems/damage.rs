//! Apply damage for pending hits.

use crate::sim::state::SimState;

fn level_band(lvl: u8) -> &'static str {
    if lvl <= 4 {
        "1-4"
    } else if lvl <= 10 {
        "5-10"
    } else if lvl <= 16 {
        "11-16"
    } else {
        "17-20"
    }
}

fn pick_dice_for_level(dmg: &data_runtime::spell::DamageSpec, lvl: u8) -> String {
    if let Some(map) = &dmg.dice_by_level_band {
        let key = level_band(lvl);
        if let Some(s) = map.get(key) {
            return s.clone();
        }
        // Fallback to highest available band if exact key is missing
        for k in ["17-20", "11-16", "5-10", "1-4"] {
            if let Some(s) = map.get(k) {
                return s.clone();
            }
        }
    }
    "1d10".to_string()
}

pub fn run(state: &mut SimState) {
    let pending = std::mem::take(&mut state.pending_damage);
    for (actor_idx, ability_id, crit) in pending {
        let Some(spec) = state.spells.get(&ability_id) else {
            continue;
        };
        let Some(dmg) = &spec.damage else { continue };
        // Copy fields we need before mutably borrowing state
        let lvl = state.actors.get(actor_idx).map(|a| a.char_level).unwrap_or(1);
        let dice: String = pick_dice_for_level(dmg, lvl);
        let dmg_type = dmg.damage_type.to_ascii_lowercase();
        let _ = spec;
        // Roll
        let mut total = state.roll_dice_str(&dice);
        if crit {
            total += state.roll_dice_str(&dice);
        }
        if let Some(tgt_idx) = state.actors[actor_idx].target {
            if !state.actor_alive(actor_idx) || !state.actor_alive(tgt_idx) {
                continue;
            }
            if state.are_allies(actor_idx, tgt_idx) {
                continue;
            }
            let hp_before = state.actors[tgt_idx].hp;
            let original_total = total;
            // Underwater fire resistance (prototype): halve fire damage
            if state.underwater && dmg_type == "fire" {
                total = (total / 2).max(0);
            }
            // Apply Temporary Hit Points before HP
            if state.actors[tgt_idx].temp_hp > 0 && total > 0 {
                let absorbed = total.min(state.actors[tgt_idx].temp_hp);
                state.actors[tgt_idx].temp_hp -= absorbed;
                total -= absorbed;
                state.log(format!(
                    "temp_hp_absorb tgt={} absorbed={} thp_now={}",
                    state.actors[tgt_idx].id, absorbed, state.actors[tgt_idx].temp_hp
                ));
            }
            state.actors[tgt_idx].hp -= total;
            state.log(format!(
                "damage_applied src={} tgt={} ability={} dmg={} hp: {} -> {}",
                state.actors[actor_idx].id,
                state.actors[tgt_idx].id,
                ability_id,
                total,
                hp_before,
                state.actors[tgt_idx].hp
            ));
            // Concentration check (SRD): DC = max(10, floor(damage/2)), cap 30
            if state.actors[tgt_idx].concentration.is_some() && original_total > 0 {
                let mut dc = (original_total / 2).max(10);
                if dc > 30 {
                    dc = 30;
                }
                let (roll, _nat20) = state.roll_d20(crate::rules::attack::Advantage::Normal);
                // Simple Con save modifier: 0 for now; Bless adds 1d4 via existing logic if we reused it, but keep simple here.
                let total_save = roll; // + con_mod (0)
                let ok = total_save >= dc;
                state.log(format!(
                    "concentration_check tgt={} roll={} vs DC{} => {}",
                    state.actors[tgt_idx].id,
                    total_save,
                    dc,
                    if ok { "KEEP" } else { "BREAK" }
                ));
                if !ok {
                    let ended = state.actors[tgt_idx].concentration.take();
                    if let Some(old) = ended {
                        state.log(format!(
                            "concentration_broken tgt={} ability={}",
                            state.actors[tgt_idx].id, old
                        ));
                    }
                }
            }
        }
    }
}
