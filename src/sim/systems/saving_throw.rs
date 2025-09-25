//! Resolve saving throws for completed casts that specify a save.

use crate::core::combat::conditions::Condition;
use crate::core::rules::saves::SaveKind;
use crate::sim::state::SimState;

fn parse_save_kind(s: &str) -> SaveKind {
    match s.to_ascii_lowercase().as_str() {
        "str" | "strength" => SaveKind::Str,
        "dex" | "dexterity" => SaveKind::Dex,
        "con" | "constitution" => SaveKind::Con,
        "int" | "intelligence" => SaveKind::Int,
        "wis" | "wisdom" => SaveKind::Wis,
        "cha" | "charisma" => SaveKind::Cha,
        _ => SaveKind::Dex,
    }
}

fn parse_condition(s: &str) -> Option<Condition> {
    match s.to_ascii_lowercase().as_str() {
        "prone" => Some(Condition::Prone),
        "restrained" => Some(Condition::Restrained),
        "stunned" => Some(Condition::Stunned),
        _ => None,
    }
}

pub fn run(state: &mut SimState) {
    let completed = state.cast_completed.clone();
    for (actor_idx, ability_id) in completed {
        // Copy save info before any mutable borrows
        let (save_kind_s, save_dc_opt, on_fail) = if let Some(spec) = state.spells.get(&ability_id) {
            if let Some(save) = &spec.save {
                (save.kind.clone(), save.dc, save.on_fail.clone())
            } else { continue }
        } else { continue };
        let Some(tgt_idx) = state.actors[actor_idx].target else { continue };
        if !state.actor_alive(actor_idx) || !state.actor_alive(tgt_idx) { continue; }
        if state.are_allies(actor_idx, tgt_idx) { continue; }
        let dc = save_dc_opt.unwrap_or(state.actors[actor_idx].spell_save_dc);
        let kind = parse_save_kind(&save_kind_s);
        let mod_bonus = actor_save_mod(state, tgt_idx, kind);
        let (roll, _nat20) = state.roll_d20(crate::core::rules::attack::Advantage::Normal);
        let total = roll + mod_bonus;
        let ok = total >= dc;
        let caster_id = state.actors[actor_idx].id.clone();
        let tgt_id = state.actors[tgt_idx].id.clone();
        state.log(format!("save_resolved src={} tgt={} ability={} save={} total={} vs DC{} => {}", caster_id, tgt_id, ability_id, save_kind_s, total, dc, if ok {"SUCCEED"} else {"FAIL"}));
        if !ok {
            if let Some(of) = on_fail {
                if let Some(name) = of.apply_condition {
                    if let Some(cond) = parse_condition(&name) {
                        let dur = of.duration_ms.unwrap_or(6000);
                        state.pending_status.push((tgt_idx, cond, dur));
                    }
                }
            }
        }
    }
}

fn actor_save_mod(state: &mut SimState, idx: usize, kind: SaveKind) -> i32 {
    let mut bonus = 0;
    // Use simple defaults: Dex+1 for non-boss, +3 for boss
    match kind { SaveKind::Dex => { bonus += if state.actors[idx].role == "boss" { 3 } else { 1 }; } _ => {} }
    // Bless adds 1d4 to saves if active
    if state.actors[idx].blessed_ms > 0 { bonus += state.roll_dice_str("1d4"); }
    bonus
}
