//! In‑process NPC state and simple melee AI/collision avoidance.
//!
//! Also hosts simple voxel destructible helpers (see `destructible` module):
//! - Grid raycast via Amanatides & Woo DDA
//! - Carve impact sphere + spawn debris with seeded RNG

use glam::Vec3;
use ecs_core::components as ec;
pub mod destructible;
pub mod jobs;
pub mod scene_build;
pub mod systems;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NpcId(pub u32);

#[derive(Debug, Clone)]
pub struct Npc {
    pub id: NpcId,
    pub pos: Vec3,
    pub radius: f32,
    pub hp: i32,
    pub max_hp: i32,
    pub alive: bool,
    pub attack_cooldown: f32,
    pub attack_anim: f32,
    /// Damage dealt per melee hit
    pub damage: i32,
    /// Movement speed in m/s
    pub speed: f32,
}

/// Stored boss stats built from data_runtime config (no ECS world yet).
#[derive(Debug, Clone)]
pub struct NivitaStats {
    pub name: String,
    pub ac: i32,
    pub abilities: ec::Abilities,
    pub saves: ec::SavingThrows,
    pub defenses: ec::Defenses,
    pub legendary: ec::LegendaryResist,
    pub spellbook: ec::Spellbook,
    pub radius: f32,
    pub height: f32,
    pub team: Option<String>,
    pub team_id: Option<u32>,
}

/// Minimal boss status used by clients.
#[derive(Debug, Clone)]
pub struct BossStatus {
    pub name: String,
    pub ac: i32,
    pub hp: i32,
    pub max: i32,
    pub pos: Vec3,
}

impl Npc {
    pub fn new(id: NpcId, pos: Vec3, radius: f32, hp: i32) -> Self {
        Self {
            id,
            pos,
            radius,
            hp,
            max_hp: hp,
            alive: true,
            attack_cooldown: 0.0,
            attack_anim: 0.0,
            damage: 5,  // default zombie hit
            speed: 2.0, // default zombie speed
        }
    }
}

#[derive(Debug, Clone)]
pub struct HitEvent {
    pub npc: NpcId,
    pub pos: Vec3,
    pub damage: i32,
    pub hp_before: i32,
    pub hp_after: i32,
    pub fatal: bool,
}

#[derive(Debug, Default)]
pub struct ServerState {
    next_id: u32,
    pub npcs: Vec<Npc>,
    /// Unique boss handle if spawned (e.g., Nivita).
    pub nivita_id: Option<NpcId>,
    /// Snapshot of Nivita's boss stats/components for replication/logging.
    pub nivita_stats: Option<NivitaStats>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            npcs: Vec::new(),
            nivita_id: None,
            nivita_stats: None,
        }
    }
    pub fn spawn_npc(&mut self, pos: Vec3, radius: f32, hp: i32) -> NpcId {
        let id = NpcId(self.next_id);
        self.next_id += 1;
        self.npcs.push(Npc::new(id, pos, radius, hp));
        id
    }
    /// Spawn the unique boss "Nivita of the Undertide" if not present.
    /// Returns the NPC id if spawned or already present.
    pub fn spawn_nivita_unique(&mut self, pos: Vec3) -> Option<NpcId> {
        if let Some(id) = self.nivita_id {
            return Some(id);
        }
        let cfg = match data_runtime::configs::npc_unique::load_nivita() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("server: failed to load nivita config: {e:#}");
                return None;
            }
        };
        let hp_mid = (cfg.hp_range.0 + cfg.hp_range.1) / 2;
        let radius = cfg.radius_m.unwrap_or(0.9);
        let id = self.spawn_npc(pos, radius, hp_mid);
        // Patch NPC parameters
        if let Some(n) = self.npcs.iter_mut().find(|n| n.id == id) {
            n.speed = cfg.speed_mps.unwrap_or(1.2);
            // Keep default damage for now; spells will handle most boss damage.
        }
        // Build and store boss stats snapshot for replication/logging
        let ab = ec::Abilities {
            str: cfg.abilities.str,
            dex: cfg.abilities.dex,
            con: cfg.abilities.con,
            int: cfg.abilities.int,
            wis: cfg.abilities.wis,
            cha: cfg.abilities.cha,
            prof: cfg.abilities.proficiency,
        };
        let mod_of = |v: i8| ((v as i16 - 10) / 2) as i8;
        let saves = if let Some(s) = cfg.saves.as_ref() {
            ec::SavingThrows {
                str_mod: s.str,
                dex_mod: s.dex,
                con_mod: s.con,
                int_mod: s.int,
                wis_mod: s.wis,
                cha_mod: s.cha,
            }
        } else {
            ec::SavingThrows {
                str_mod: mod_of(ab.str),
                dex_mod: mod_of(ab.dex),
                con_mod: mod_of(ab.con),
                int_mod: mod_of(ab.int) + ab.prof,
                wis_mod: mod_of(ab.wis) + ab.prof,
                cha_mod: mod_of(ab.cha) + ab.prof,
            }
        };
        let resist: Vec<ec::DamageType> = cfg
            .defenses
            .resistances
            .iter()
            .filter_map(|s| ecs_core::parse::parse_damage_type(s))
            .collect();
        let immune: Vec<ec::Condition> = cfg
            .defenses
            .immunities
            .iter()
            .filter_map(|s| ecs_core::parse::parse_condition(s))
            .collect();
        let reset = match cfg.legendary.resets.as_deref() {
            Some("per_encounter") => ec::ResetRule::PerEncounter,
            _ => ec::ResetRule::LongRest,
        };
        let lres = ec::LegendaryResist::new(cfg.legendary.resist_per_day, reset);
        let spell_ids = |v: &[String]| v.iter().map(|s| ec::SpellId(s.clone())).collect();
        let book = ec::Spellbook {
            cantrips: spell_ids(&cfg.spellbook.cantrips),
            level_1_3: spell_ids(&cfg.spellbook.level_1_3),
            level_4_5: spell_ids(&cfg.spellbook.level_4_5),
            signature: spell_ids(&cfg.spellbook.signature),
        };
        let team_id = match cfg.team.as_deref() {
            Some("enemy_raid") => Some(2u32),
            Some("players") => Some(1u32),
            _ => None,
        };
        self.nivita_stats = Some(NivitaStats {
            name: cfg.name.clone(),
            ac: i32::from(cfg.defenses.ac),
            abilities: ab,
            saves,
            defenses: ec::Defenses { resist, immune },
            legendary: lres,
            spellbook: book,
            radius,
            height: cfg.height_m.unwrap_or(1.9),
            team: cfg.team.clone(),
            team_id,
        });
        self.nivita_id = Some(id);
        log::info!(
            "server: spawned unique boss '{}' (hp={}..{}, ac={}) as {:?}",
            cfg.name,
            cfg.hp_range.0,
            cfg.hp_range.1,
            cfg.defenses.ac,
            id
        );
        metrics::counter!("boss.nivita.spawns_total").increment(1);
        Some(id)
    }
    /// Lightweight status for UI/replication.
    pub fn nivita_status(&self) -> Option<BossStatus> {
        let id = self.nivita_id?;
        let n = self.npcs.iter().find(|n| n.id == id)?;
        let stats = self.nivita_stats.as_ref()?;
        Some(BossStatus {
            name: stats.name.clone(),
            ac: stats.ac,
            hp: n.hp,
            max: n.max_hp,
            pos: n.pos,
        })
    }
    pub fn ring_spawn(&mut self, count: usize, radius: f32, hp: i32) {
        for i in 0..count {
            let a = (i as f32) / (count as f32) * std::f32::consts::TAU;
            let pos = Vec3::new(radius * a.cos(), 0.6, radius * a.sin());
            self.spawn_npc(pos, 0.95, hp);
        }
    }
    /// Move toward nearest wizard and attack when in range. Returns (wizard_idx, damage) per hit.
    pub fn step_npc_ai(&mut self, dt: f32, wizards: &[Vec3]) -> Vec<(usize, i32)> {
        if wizards.is_empty() {
            return Vec::new();
        }
        let wizard_r = 0.7f32;
        let melee_pad = 0.35f32;
        let attack_cd = 1.5f32;
        let attack_anim_time = 0.8f32;
        let mut hits = Vec::new();
        let mut chosen: Vec<Option<usize>> = vec![None; self.npcs.len()];
        for (idx, n) in self.npcs.iter_mut().enumerate() {
            if !n.alive {
                continue;
            }
            n.attack_cooldown = (n.attack_cooldown - dt).max(0.0);
            n.attack_anim = (n.attack_anim - dt).max(0.0);
            let mut best_i = 0usize;
            let mut best_d2 = f32::INFINITY;
            for (i, w) in wizards.iter().enumerate() {
                let dx = w.x - n.pos.x;
                let dz = w.z - n.pos.z;
                let d2 = dx * dx + dz * dz;
                if d2 < best_d2 {
                    best_d2 = d2;
                    best_i = i;
                }
            }
            chosen[idx] = Some(best_i);
            let target = wizards[best_i];
            let to = Vec3::new(target.x - n.pos.x, 0.0, target.z - n.pos.z);
            let dist = to.length();
            let contact = n.radius + wizard_r + melee_pad;
            if dist > contact + 0.02 {
                let step = (n.speed * dt).min(dist - contact);
                if step > 1e-4 {
                    n.pos += to.normalize() * step;
                }
            }
        }
        self.resolve_collisions(wizards);
        for (idx, n) in self.npcs.iter_mut().enumerate() {
            if !n.alive {
                continue;
            }
            if let Some(best_i) = chosen[idx] {
                let target = wizards[best_i];
                let to = Vec3::new(target.x - n.pos.x, 0.0, target.z - n.pos.z);
                let dist = to.length();
                let contact = n.radius + wizard_r + melee_pad;
                if dist <= contact + 0.05 && n.attack_cooldown <= 0.0 {
                    hits.push((best_i, n.damage));
                    n.attack_cooldown = attack_cd;
                    n.attack_anim = attack_anim_time;
                }
            }
        }
        hits
    }
    fn resolve_collisions(&mut self, wizards: &[Vec3]) {
        let nlen = self.npcs.len();
        for i in 0..nlen {
            if !self.npcs[i].alive {
                continue;
            }
            for j in (i + 1)..nlen {
                if !self.npcs[j].alive {
                    continue;
                }
                let mut dx = self.npcs[j].pos.x - self.npcs[i].pos.x;
                let mut dz = self.npcs[j].pos.z - self.npcs[i].pos.z;
                let d2 = dx * dx + dz * dz;
                let min_d = self.npcs[i].radius + self.npcs[j].radius;
                if d2 < min_d * min_d {
                    let mut d = d2.sqrt();
                    if d < 1e-4 {
                        dx = 1.0;
                        dz = 0.0;
                        d = 1e-4;
                    }
                    dx /= d;
                    dz /= d;
                    let overlap = min_d - d;
                    let push = overlap * 0.5;
                    self.npcs[i].pos.x -= dx * push;
                    self.npcs[i].pos.z -= dz * push;
                    self.npcs[j].pos.x += dx * push;
                    self.npcs[j].pos.z += dz * push;
                }
            }
        }
        let wiz_r = 0.7f32;
        for n in &mut self.npcs {
            if !n.alive {
                continue;
            }
            for w in wizards {
                let mut dx = n.pos.x - w.x;
                let mut dz = n.pos.z - w.z;
                let d2 = dx * dx + dz * dz;
                let min_d = n.radius + wiz_r;
                if d2 < min_d * min_d {
                    let mut d = d2.sqrt();
                    if d < 1e-4 {
                        dx = 1.0;
                        dz = 0.0;
                        d = 1e-4;
                    }
                    dx /= d;
                    dz /= d;
                    let overlap = min_d - d;
                    n.pos.x += dx * overlap;
                    n.pos.z += dz * overlap;
                }
            }
        }
    }
}
