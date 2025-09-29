use std::collections::HashMap;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::combat::fsm::{ActionDone, ActionState, Gcd};
use crate::rules::attack::Advantage;
use crate::sim::events::SimEvent;
use data_runtime::loader::{load_class_spec, load_monster_spec};
use data_runtime::specdb::SpecDb;
use data_runtime::spell::SpellSpec;

#[derive(Debug, Clone)]
pub struct ActorSim {
    pub id: String,
    pub role: String,
    pub class: Option<String>,
    pub team: Option<String>,
    pub hp: i32,
    pub ac_base: i32,
    pub ac_temp_bonus: i32,
    pub ability_ids: Vec<String>,
    pub action: ActionState,
    pub gcd: Gcd,
    pub target: Option<usize>,
    // Character level for scaling (e.g., cantrip dice bands)
    pub char_level: u8,
    pub spell_attack_bonus: i32,
    pub spell_save_dc: i32,
    pub statuses: Vec<(crate::combat::conditions::Condition, u32)>,
    pub blessed_ms: u32,
    pub reaction_ready: bool,
    pub next_ability_idx: usize,
    // Temporary Hit Points (THP) applied before real HP. Non-stacking: take the higher value.
    pub temp_hp: i32,
    // Active concentration effect (by ability id). Starting a new one ends the old one.
    pub concentration: Option<String>,
    // Per-ability cooldown timers in milliseconds.
    pub ability_cooldowns: HashMap<String, u32>,
}

pub struct SimState {
    pub tick_ms: u32,
    pub rng: ChaCha8Rng,
    pub actors: Vec<ActorSim>,
    pub spells: HashMap<String, SpellSpec>,
    pub spec_db: SpecDb,
    pub cast_completed: Vec<(usize, String)>,
    pub pending_damage: Vec<(usize, String, bool)>,
    pub pending_status: Vec<(usize, crate::combat::conditions::Condition, u32)>,
    pub events: Vec<SimEvent>,
    pub underwater: bool,
}

impl SimState {
    pub fn new(tick_ms: u32, seed: u64) -> Self {
        Self {
            tick_ms,
            rng: ChaCha8Rng::seed_from_u64(seed),
            actors: Vec::new(),
            spells: HashMap::new(),
            spec_db: SpecDb::load_default(),
            cast_completed: Vec::new(),
            pending_damage: Vec::new(),
            pending_status: Vec::new(),
            events: Vec::new(),
            underwater: false,
        }
    }

    pub fn load_spell(&self, id: &str) -> anyhow::Result<SpellSpec> {
        if let Some(s) = self.spec_db.get_spell(id) {
            return Ok(s.clone());
        }
        anyhow::bail!("spell not found: {}", id)
    }

    pub fn load_class_defaults(&self, id: &str) -> anyhow::Result<(i32, i32, i32)> {
        if let Some(c) = self.spec_db.get_class(id) {
            return Ok((c.base_ac, c.spell_attack_bonus, c.spell_save_dc));
        }
        let rel = format!("classes/{}.json", id);
        let spec = load_class_spec(rel)?;
        Ok((spec.base_ac, spec.spell_attack_bonus, spec.spell_save_dc))
    }

    pub fn load_monster_defaults(&self, id: &str) -> anyhow::Result<(i32, i32)> {
        if let Some(m) = self.spec_db.get_monster(id) {
            return Ok((m.ac, m.hp));
        }
        let rel = format!("monsters/{}.json", id);
        let spec = load_monster_spec(rel)?;
        Ok((spec.ac, spec.hp))
    }

    pub fn tick(&mut self) {
        let dt = self.tick_ms;
        for idx in 0..self.actors.len() {
            let (done, actor_id) = {
                let a = &mut self.actors[idx];
                if a.hp <= 0 {
                    continue;
                }
                // Tick per-ability cooldowns
                for v in a.ability_cooldowns.values_mut() {
                    *v = v.saturating_sub(dt);
                }
                let (next, done) = a.action.clone().tick(dt);
                a.action = next;
                (done, a.id.clone())
            };
            if let Some(ActionDone::CastCompleted { ability }) = done {
                self.cast_completed.push((idx, ability.0));
                self.events.push(SimEvent::CastCompleted {
                    actor: actor_id,
                    ability: "..".into(),
                });
            }
        }
    }

    pub fn target_ac(&self, actor_idx: usize) -> Option<i32> {
        let tgt = self.actors[actor_idx].target?;
        Some(self.actors[tgt].ac_base + self.actors[tgt].ac_temp_bonus)
    }

    pub fn are_allies(&self, a_idx: usize, b_idx: usize) -> bool {
        match (&self.actors[a_idx].team, &self.actors[b_idx].team) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    pub fn roll_d20(&mut self, _adv: Advantage) -> (i32, bool) {
        let v: i32 = (self.rng.random::<u32>() % 20 + 1) as i32;
        (v, v == 20)
    }

    pub fn roll_dice_str(&mut self, dice: &str) -> i32 {
        // Parser for NdM, optionally with +K/-K (e.g., 3d4+3, 2d6-1). Whitespace not expected.
        // Fallbacks default to 1d1.
        let mut base = dice.trim();
        let mut flat: i32 = 0;
        // Handle +K or -K suffix
        if let Some(pos) = base.rfind(['+', '-'])
            && pos > 0
        {
            let (left, right) = base.split_at(pos);
            base = left;
            let sign = &right[..1];
            let k = right[1..].parse::<i32>().unwrap_or(0);
            flat = if sign == "+" { k } else { -k };
        }
        let (n, m) = if let Some((n, m)) = base.split_once('d') {
            (n.parse::<i32>().unwrap_or(1), m.parse::<i32>().unwrap_or(1))
        } else {
            (1, 1)
        };
        let mut sum = 0;
        for _ in 0..n.max(0) {
            sum += (self.rng.random::<u32>() % (m.max(1) as u32) + 1) as i32;
        }
        sum + flat
    }

    pub fn actor_alive(&self, idx: usize) -> bool {
        self.actors.get(idx).map(|a| a.hp > 0).unwrap_or(false)
    }
}

// Built-in fallback specs
impl SimState {
    pub fn builtin_basic_attack_spec() -> data_runtime::spell::SpellSpec {
        use data_runtime::spell::{AttackSpec, DamageSpec, SpellSpec};
        use std::collections::HashMap;
        let mut dice = HashMap::new();
        dice.insert("1-4".to_string(), "1d6".to_string());
        SpellSpec {
            id: "core.basic_attack".into(),
            name: "Basic Attack".into(),
            version: None,
            source: None,
            school: "weapon".into(),
            level: 0,
            classes: vec![],
            tags: vec!["weapon".into()],
            cast_time_s: 1.0,
            gcd_s: 1.0,
            cooldown_s: 0.0,
            resource_cost: None,
            can_move_while_casting: false,
            targeting: "unit".into(),
            requires_line_of_sight: true,
            range_ft: 5,
            minimum_range_ft: 0,
            firing_arc_deg: 180,
            attack: Some(AttackSpec {
                kind: "melee_weapon_attack".into(),
                rng_stream: Some("attack".into()),
                crit_rule: Some("nat20_double_dice".into()),
            }),
            damage: Some(DamageSpec {
                damage_type: "slashing".into(),
                add_spell_mod_to_damage: false,
                dice_by_level_band: Some(dice),
            }),
            projectile: None,
            secondary: None,
            latency: None,
            events: vec![],
            metrics: None,
            policy: None,
            save: None,
        }
    }

    pub fn builtin_boss_tentacle_spec() -> data_runtime::spell::SpellSpec {
        use data_runtime::spell::{AttackSpec, DamageSpec, SpellSpec};
        use std::collections::HashMap;
        let mut dice = HashMap::new();
        dice.insert("1-4".to_string(), "3d10".to_string());
        SpellSpec {
            id: "boss.tentacle".into(),
            name: "Tentacle".into(),
            version: None,
            source: None,
            school: "natural".into(),
            level: 0,
            classes: vec![],
            tags: vec!["melee".into(), "boss".into()],
            cast_time_s: 1.0,
            gcd_s: 1.0,
            cooldown_s: 0.0,
            resource_cost: None,
            can_move_while_casting: false,
            targeting: "unit".into(),
            requires_line_of_sight: true,
            range_ft: 10,
            minimum_range_ft: 0,
            firing_arc_deg: 180,
            attack: Some(AttackSpec {
                kind: "melee_attack".into(),
                rng_stream: Some("attack".into()),
                crit_rule: Some("nat20_double_dice".into()),
            }),
            damage: Some(DamageSpec {
                damage_type: "bludgeoning".into(),
                add_spell_mod_to_damage: false,
                dice_by_level_band: Some(dice),
            }),
            projectile: None,
            secondary: None,
            latency: None,
            events: vec![],
            metrics: None,
            policy: None,
            save: None,
        }
    }
}
