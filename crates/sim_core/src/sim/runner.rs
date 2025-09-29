use crate::sim::state::{ActorSim, SimState};
use crate::sim::systems;
use data_runtime::scenario::Scenario;

pub fn run_scenario(scn: &Scenario, result_only: bool) {
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
        // Load defaults
        let (ac_base, hp, atk, dc) = if a.role == "boss" {
            let (ac, hp) = state.load_monster_defaults(&a.id).unwrap_or((17, 120));
            (ac, hp, 8, 13)
        } else if let Some(class_id) = &a.class {
            let (ac, atk, dc) = state.load_class_defaults(class_id).unwrap_or((12, 0, 0));
            (ac, 30, atk, dc)
        } else {
            (12, 30, 0, 0)
        };
        // Team membership: use scenario team if present; else default to "players" for non-boss, "boss" for boss
        let team = a.team.clone().or_else(|| {
            if a.role == "boss" {
                Some("boss".into())
            } else {
                Some("players".into())
            }
        });
        let mut actor = ActorSim {
            id: a.id.clone(),
            role: a.role.clone(),
            class: a.class.clone(),
            team,
            hp,
            ac_base,
            ac_temp_bonus: 0,
            ability_ids: a.abilities.clone(),
            action: Default::default(),
            gcd: Default::default(),
            target: None,
            char_level: 1,
            spell_attack_bonus: atk,
            spell_save_dc: dc,
            statuses: Vec::new(),
            blessed_ms: 0,
            reaction_ready: true,
            next_ability_idx: 0,
            temp_hp: 0,
            concentration: None,
            ability_cooldowns: std::collections::HashMap::new(),
        };
        if actor.role == "boss" && actor.ability_ids.is_empty() {
            actor.ability_ids.push("boss.tentacle".into());
        }
        state.actors.push(actor);
    }
    for i in 0..state.actors.len() {
        if i != boss_idx {
            state.actors[i].target = Some(boss_idx);
        }
    }

    // Run until boss dies or party wipes, with a safety cap
    let max_steps = (300_000 / scn.tick_ms) as usize; // 5 minutes cap
    for step in 0..max_steps {
        // Reset per-tick temp AC and reaction
        for a in &mut state.actors {
            a.ac_temp_bonus = 0;
            a.reaction_ready = true;
        }
        systems::ai::run(&mut state);
        systems::cast_begin::run(&mut state);
        state.tick();
        systems::saving_throw::run(&mut state);
        systems::buffs::run(&mut state);
        systems::attack_roll::run(&mut state);
        systems::damage::run(&mut state);
        systems::conditions::run(&mut state);
        // Clear one-tick cast completion triggers
        state.cast_completed.clear();
        // Check win/loss
        let boss_alive = state.actors.iter().any(|a| a.role == "boss" && a.hp > 0);
        let party_alive = state
            .actors
            .iter()
            .any(|a| a.team.as_deref() == Some("players") && a.hp > 0);
        if !boss_alive {
            println!(
                "[sim] result: BOSS DEFEATED at t={} ms",
                (step as u32) * scn.tick_ms
            );
            break;
        }
        if !party_alive {
            println!(
                "[sim] result: PARTY WIPED at t={} ms",
                (step as u32) * scn.tick_ms
            );
            break;
        }
    }

    // Print summary
    if !result_only {
        for ev in &state.events {
            println!("[sim] {:?}", ev);
        }
    }
    for a in &state.actors {
        println!("[sim] final hp: {} => {}", a.id, a.hp);
    }
}
