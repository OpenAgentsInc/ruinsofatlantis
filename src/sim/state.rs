use std::collections::HashMap;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::core::combat::fsm::{ActionDone, ActionState, Gcd};
use crate::core::data::loader::load_spell_spec;
use crate::core::data::spell::SpellSpec;
use crate::core::rules::attack::Advantage;

#[derive(Debug, Clone)]
pub struct ActorSim {
    pub id: String,
    pub role: String,
    pub hp: i32,
    pub ability_ids: Vec<String>,
    pub action: ActionState,
    pub gcd: Gcd,
    pub target: Option<usize>,
}

pub struct SimState {
    pub tick_ms: u32,
    pub rng: ChaCha8Rng,
    pub actors: Vec<ActorSim>,
    pub spells: HashMap<String, SpellSpec>,
    pub cast_completed: Vec<(usize, String)>,
    pub pending_damage: Vec<(usize, String, bool)>,
    pub logs: Vec<String>,
}

impl SimState {
    pub fn new(tick_ms: u32, seed: u64) -> Self {
        Self {
            tick_ms,
            rng: ChaCha8Rng::seed_from_u64(seed),
            actors: Vec::new(),
            spells: HashMap::new(),
            cast_completed: Vec::new(),
            pending_damage: Vec::new(),
            logs: Vec::new(),
        }
    }

    pub fn load_spell(&self, id: &str) -> anyhow::Result<SpellSpec> {
        // Try exact id.json under spells/
        let primary = format!("spells/{}.json", id);
        if let Ok(spec) = load_spell_spec(&primary) { return Ok(spec); }
        // Try last segment after '.' (e.g., wiz.fire_bolt.srd521 -> srd521.json or fire_bolt.json)
        if let Some(last) = id.split('.').last() {
            let alt = format!("spells/{}.json", last);
            if let Ok(spec) = load_spell_spec(&alt) { return Ok(spec); }
        }
        // Heuristic: if the id contains "fire_bolt", fall back to fire_bolt.json
        if id.contains("fire_bolt") {
            if let Ok(spec) = load_spell_spec("spells/fire_bolt.json") { return Ok(spec); }
        }
        // Fallback: try the filename portion after a slash if present
        if let Some((_ns, tail)) = id.rsplit_once('/') {
            let alt = format!("spells/{}.json", tail);
            if let Ok(spec) = load_spell_spec(&alt) { return Ok(spec); }
        }
        load_spell_spec(&primary)
    }

    pub fn tick(&mut self) {
        let dt = self.tick_ms;
        for idx in 0..self.actors.len() {
            let (done, actor_id) = {
                let a = &mut self.actors[idx];
                let (next, done) = a.action.clone().tick(dt);
                a.action = next;
                (done, a.id.clone())
            };
            if let Some(ActionDone::CastCompleted { ability }) = done {
                self.cast_completed.push((idx, ability.0));
                self.log(format!("cast_completed actor={} ability=..", actor_id));
            }
        }
    }

    pub fn log(&mut self, s: String) {
        self.logs.push(s);
    }

    pub fn target_ac(&self, actor_idx: usize) -> Option<i32> {
        let tgt = self.actors[actor_idx].target?;
        // Prototype: fixed AC for targets
        Some(match self.actors[tgt].role.as_str() { "boss" => 17, _ => 12 })
    }

    pub fn roll_d20(&mut self, _adv: Advantage) -> (i32, bool) {
        let v: i32 = (self.rng.random::<u32>() % 20 + 1) as i32;
        (v, v == 20)
    }

    pub fn roll_dice_str(&mut self, dice: &str) -> i32 {
        // Very small parser for NdM
        let (n, m) = if let Some((n, m)) = dice.split_once('d') {
            (n.parse::<i32>().unwrap_or(1), m.parse::<i32>().unwrap_or(1))
        } else { (1, 1) };
        let mut sum = 0;
        for _ in 0..n { sum += (self.rng.random::<u32>() % (m as u32) + 1) as i32; }
        sum
    }
}
