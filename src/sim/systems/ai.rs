//! Simple AI: ensure bosses target an alive player; ensure players target boss.

use crate::sim::state::SimState;

pub fn run(state: &mut SimState) {
    // Find an alive boss index and an alive player index
    let boss_idxs: Vec<usize> = state.actors.iter().enumerate().filter(|(_, a)| a.role == "boss" && a.hp > 0).map(|(i,_)| i).collect();
    let player_idxs: Vec<usize> = state.actors.iter().enumerate().filter(|(_, a)| a.team.as_deref() == Some("players") && a.hp > 0).map(|(i,_)| i).collect();

    for &b in &boss_idxs {
        if state.actors[b].target.map(|t| state.actor_alive(t)).unwrap_or(false) { continue; }
        if let Some(&p) = player_idxs.first() { state.actors[b].target = Some(p); }
    }
    // Ensure players target the boss
    let boss_target = boss_idxs.first().copied();
    for (_i, a) in state.actors.iter_mut().enumerate() {
        if a.team.as_deref() == Some("players") && a.hp > 0 {
            if let Some(bi) = boss_target { a.target = Some(bi); }
        }
    }
}
