use crate::core::data::scenario::Scenario;
use crate::sim::state::{ActorSim, SimState};
use crate::sim::systems;

pub fn run_scenario(scn: &Scenario) {
    let seed = scn.seed.unwrap_or(42);
    let mut state = SimState::new(scn.tick_ms, seed);
    state.underwater = scn.underwater;

    // Create actors, set simple targets: everyone targets the first 'boss'
    let boss_idx = scn
        .actors
        .iter()
        .position(|a| a.role == "boss")
        .unwrap_or(0);
    for a in &scn.actors {
        let (atk, dc) = match a.class.as_deref() {
            Some("wizard") | Some("sorcerer") => (5, 13),
            Some("cleric") | Some("warlock") => (5, 13),
            _ => (4, 12),
        };
        state.actors.push(ActorSim {
            id: a.id.clone(),
            role: a.role.clone(),
            class: a.class.clone(),
            hp: if a.role == "boss" { 120 } else { 30 },
            ability_ids: a.abilities.clone(),
            action: Default::default(),
            gcd: Default::default(),
            target: None,
            spell_attack_bonus: atk,
            spell_save_dc: dc,
            statuses: Vec::new(),
        });
    }
    for i in 0..state.actors.len() { if i != boss_idx { state.actors[i].target = Some(boss_idx); } }

    // Run a few seconds of sim
    let steps = (3000 / scn.tick_ms) as usize; // 3 seconds
    for _ in 0..steps {
        systems::cast_begin::run(&mut state);
        state.tick();
        systems::saving_throw::run(&mut state);
        systems::attack_roll::run(&mut state);
        systems::damage::run(&mut state);
        systems::conditions::run(&mut state);
        // Clear one-tick cast completion triggers
        state.cast_completed.clear();
    }

    // Print summary
    for log in &state.logs { println!("[sim] {}", log); }
    for a in &state.actors { println!("[sim] final hp: {} => {}", a.id, a.hp); }
}
