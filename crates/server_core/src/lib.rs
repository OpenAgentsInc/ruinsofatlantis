//! In‑process NPC state and simple melee AI/collision avoidance.
//!
//! Also hosts simple voxel destructible helpers (see `destructible` module):
//! - Grid raycast via Amanatides & Woo DDA
//! - Carve impact sphere + spawn debris with seeded RNG

use ecs_core::components as ec;
mod actor;
mod combat;
pub use actor::*;
pub use combat::*;
use glam::Vec3;
pub mod destructible;
pub mod jobs;
pub mod scene_build;
pub mod systems;

// Legacy NPC types removed. Use ActorStore (Zombie/Boss kinds).

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

// Legacy Wizard removed. Use ActorStore (Wizard kind).

/// Projectile kind enum.
///
/// IMPORTANT: The server is authoritative over all projectile tuning
/// (speed, lifetime, AoE radius, damage). Clients must never supply
/// gameplay parameters — they only request a kind.
#[derive(Debug, Clone, Copy)]
pub enum ProjKind {
    Firebolt,
    Fireball,
    MagicMissile,
}

/// Server-side projectile state used for authoritative collision.
#[derive(Debug, Clone)]
pub struct Projectile {
    pub id: u32,
    pub pos: Vec3,
    pub vel: Vec3,
    pub kind: ProjKind,
    pub age: f32,
    pub life: f32,
    /// Optional owner wizard id (1=PC, >=2=NPC wizard)
    pub owner: Option<ActorId>,
}

/// Server-side resolved projectile parameters used for spawning and collision.
#[derive(Debug, Clone, Copy)]
struct ProjectileSpec {
    speed_mps: f32,
    life_s: f32,
    aoe_radius_m: f32,
    damage: i32,
}

#[inline]
fn segment_hits_circle_xz(p0: Vec3, p1: Vec3, center: Vec3, radius: f32) -> bool {
    let a = glam::Vec2::new(p0.x, p0.z);
    let b = glam::Vec2::new(p1.x, p1.z);
    let c = glam::Vec2::new(center.x, center.z);
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 <= 1e-12 {
        return (a - c).length_squared() <= radius * radius;
    }
    let t = ((c - a).dot(ab) / len2).clamp(0.0, 1.0);
    let closest = a + ab * t;
    (closest - c).length_squared() <= radius * radius
}

// Legacy NPC ctor removed.

// Legacy hit events removed.

#[derive(Debug, Default)]
pub struct ServerState {
    /// Unique boss handle if spawned (e.g., Nivita).
    pub nivita_actor_id: Option<ActorId>,
    /// Snapshot of Nivita's boss stats/components for replication/logging.
    pub nivita_stats: Option<NivitaStats>,
    /// Live projectiles spawned by wizards.
    pub projectiles: Vec<Projectile>,
    next_proj_id: u32,
    /// New authoritative actor store (bridge toward ECS)
    pub actors: ActorStore,
    pub factions: FactionState,
    /// Cached PC actor id (spawned during sync)
    pub pc_actor: Option<ActorId>,
}

impl ServerState {
    // Legacy AoE removed; use apply_aoe_at_actors.
    pub fn new() -> Self {
        Self {
            nivita_actor_id: None,
            nivita_stats: None,
            projectiles: Vec::new(),
            next_proj_id: 1,
            actors: ActorStore::default(),
            factions: FactionState::default(),
            pc_actor: None,
        }
    }
    // Legacy actor rebuild removed. Actors are authoritative.

    /// Apply AoE to actors, skipping self-damage for PC-owned sources and flipping factions when PC hits Wizards.
    fn apply_aoe_at_actors(
        &mut self,
        x: f32,
        z: f32,
        r2: f32,
        damage: i32,
        source: Option<ActorId>,
    ) -> usize {
        let src_team = source.and_then(|id| self.actors.get(id).map(|a| a.team));
        let mut hits = 0usize;
        for a in self.actors.iter_mut() {
            // Skip self-damage for PC-owned AoE
            if let (Some(Team::Pc), Some(src)) = (src_team, source)
                && a.id == src
            {
                continue;
            }
            let dx = a.tr.pos.x - x;
            let dz = a.tr.pos.z - z;
            if dx * dx + dz * dz <= r2 && a.hp.alive() {
                a.hp.hp = (a.hp.hp - damage).max(0);
                hits += 1;
                if let Some(Team::Pc) = src_team
                    && a.team == Team::Wizards
                {
                    self.factions.pc_vs_wizards_hostile = true;
                }
            }
        }
        hits
    }
    /// Mirror wizard positions into ActorStore and ensure PC/NPC wizard actors exist.
    pub fn sync_wizards(&mut self, wiz_pos: &[Vec3]) {
        // PC (index 0)
        if let Some(p0) = wiz_pos.first().copied() {
            if self.pc_actor.is_none() {
                let id = self.actors.spawn(
                    ActorKind::Wizard,
                    Team::Pc,
                    Transform {
                        pos: p0,
                        yaw: 0.0,
                        radius: 0.7,
                    },
                    Health { hp: 100, max: 100 },
                );
                self.pc_actor = Some(id);
            }
            if let Some(id) = self.pc_actor
                && let Some(a) = self.actors.get_mut(id)
            {
                a.tr.pos = p0;
            }
        }
        // Extra wizard positions correspond to NPC wizards (Team::Wizards)
        let need = wiz_pos.len().saturating_sub(1);
        let mut npc_ids: Vec<ActorId> = self
            .actors
            .actors
            .iter()
            .filter(|a| a.kind == ActorKind::Wizard && a.team == Team::Wizards)
            .map(|a| a.id)
            .collect();
        while npc_ids.len() < need {
            let id = self.actors.spawn(
                ActorKind::Wizard,
                Team::Wizards,
                Transform {
                    pos: Vec3::ZERO,
                    yaw: 0.0,
                    radius: 0.7,
                },
                Health { hp: 100, max: 100 },
            );
            npc_ids.push(id);
        }
        for (i, id) in npc_ids.into_iter().enumerate() {
            if let Some(p) = wiz_pos.get(i + 1).copied()
                && let Some(a) = self.actors.get_mut(id)
            {
                a.tr.pos = p;
            }
        }
    }
    pub fn spawn_projectile(&mut self, pos: Vec3, vel: Vec3, kind: ProjKind) {
        let id = self.next_proj_id;
        self.next_proj_id = self.next_proj_id.wrapping_add(1);
        let spec = self.projectile_spec(kind);
        let (age, life) = (0.0, spec.life_s);
        self.projectiles.push(Projectile {
            id,
            pos,
            vel,
            kind,
            age,
            life,
            owner: None,
        });
    }
    /// Spawn a projectile by unit direction; velocity is scaled using server specs.
    pub fn spawn_projectile_from_dir(&mut self, pos: Vec3, dir: Vec3, kind: ProjKind) {
        let d = dir.normalize_or_zero();
        let spec = self.projectile_spec(kind);
        if std::env::var("RA_LOG_FIREBALL")
            .map(|v| v == "1")
            .unwrap_or(false)
            && matches!(kind, ProjKind::Fireball)
        {
            log::info!(
                "server: Fireball spawn speed={:.1} life={:.2}s r={:.1} dmg={}",
                spec.speed_mps,
                spec.life_s,
                spec.aoe_radius_m,
                spec.damage
            );
        }
        // Default owner is None; platform/NPC-cast may override via helper below
        let id = self.next_proj_id;
        self.next_proj_id = self.next_proj_id.wrapping_add(1);
        let (age, life) = (0.0, spec.life_s);
        self.projectiles.push(Projectile {
            id,
            pos,
            vel: d * spec.speed_mps,
            kind,
            age,
            life,
            owner: None,
        });
    }

    /// Owned variant: attaches the owner wizard id (1=PC, >=2=NPC)
    pub fn spawn_projectile_from_dir_owned(
        &mut self,
        pos: Vec3,
        dir: Vec3,
        kind: ProjKind,
        owner: Option<ActorId>,
    ) {
        let d = dir.normalize_or_zero();
        let spec = self.projectile_spec(kind);
        let id = self.next_proj_id;
        self.next_proj_id = self.next_proj_id.wrapping_add(1);
        let (age, life) = (0.0, spec.life_s);
        self.projectiles.push(Projectile {
            id,
            pos,
            vel: d * spec.speed_mps,
            kind,
            age,
            life,
            owner,
        });
    }
    /// Convenience spawner for PC-owned projectiles.
    pub fn spawn_projectile_from_pc(&mut self, pos: Vec3, dir: Vec3, kind: ProjKind) {
        let owner = self.pc_actor;
        self.spawn_projectile_from_dir_owned(pos, dir, kind, owner);
    }
    /// Resolve server-authoritative projectile spec. Falls back to baked defaults
    /// when the DB cannot be loaded.
    fn projectile_spec(&self, kind: ProjKind) -> ProjectileSpec {
        let db = data_runtime::specs::projectiles::ProjectileSpecDb::load_default().ok();
        match kind {
            ProjKind::Firebolt => {
                let s = db
                    .as_ref()
                    .and_then(|db| db.actions.get("AtWillLMB"))
                    .cloned()
                    .unwrap_or(data_runtime::specs::projectiles::ProjectileSpec {
                        speed_mps: 40.0,
                        radius_m: 0.2,
                        damage: 10,
                        life_s: 1.5,
                    });
                ProjectileSpec {
                    speed_mps: s.speed_mps,
                    life_s: s.life_s,
                    aoe_radius_m: 0.0,
                    damage: s.damage,
                }
            }
            ProjKind::Fireball => {
                let s = db
                    .as_ref()
                    .and_then(|db| db.actions.get("EncounterQ"))
                    .cloned()
                    .unwrap_or(data_runtime::specs::projectiles::ProjectileSpec {
                        speed_mps: 30.0,
                        radius_m: 6.0,
                        damage: 28,
                        life_s: 1.5,
                    });
                ProjectileSpec {
                    speed_mps: s.speed_mps,
                    life_s: s.life_s,
                    aoe_radius_m: s.radius_m.max(0.0),
                    damage: s.damage.max(0),
                }
            }
            ProjKind::MagicMissile => {
                // Close-range, medium speed, short TTL; damage light per hit
                let s = data_runtime::specs::projectiles::ProjectileSpecDb::load_default()
                    .ok()
                    .and_then(|db| db.actions.get("MagicMissile").cloned())
                    .unwrap_or(data_runtime::specs::projectiles::ProjectileSpec {
                        speed_mps: 28.0,
                        radius_m: 0.5,
                        damage: 7,
                        life_s: 1.0,
                    });
                ProjectileSpec {
                    speed_mps: s.speed_mps,
                    life_s: s.life_s,
                    aoe_radius_m: s.radius_m.max(0.0),
                    damage: s.damage.max(0),
                }
            }
        }
    }
    /// Step server-authoritative systems: NPC AI/melee, wizard casts, projectile
    /// integration/collision. Collisions reduce HP for both NPCs and wizards.
    pub fn step_authoritative(&mut self, dt: f32, wizard_positions: &[Vec3]) {
        // Ensure we mirror wizard positions
        self.sync_wizards(wizard_positions);
        // 1) Minimal actor-based NPC AI (Undead seek nearest hostile Wizard/PC, melee on contact)
        {
            // Snapshot candidates to avoid borrow conflicts
            let wizard_targets: Vec<(ActorId, Vec3, f32)> = self
                .actors
                .actors
                .iter()
                .filter(|a| a.hp.alive() && matches!(a.kind, ActorKind::Wizard))
                .map(|a| (a.id, a.tr.pos, a.tr.radius))
                .collect();
            let undead_ids: Vec<ActorId> = self
                .actors
                .actors
                .iter()
                .filter(|a| a.hp.alive() && matches!(a.kind, ActorKind::Zombie))
                .map(|a| a.id)
                .collect();
            for uid in undead_ids {
                // Defaults until components carry these
                let speed = 2.0f32;
                let damage = 5i32;
                let (pos_u, rad_u) = if let Some(a) = self.actors.get(uid) {
                    (a.tr.pos, a.tr.radius)
                } else {
                    continue;
                };
                // Find nearest wizard target
                let mut best: Option<(f32, ActorId, Vec3, f32)> = None;
                for (tid, p, r) in &wizard_targets {
                    // Undead are hostile to both Pc and Wizards teams by default via FactionState
                    let dx = p.x - pos_u.x;
                    let dz = p.z - pos_u.z;
                    let d2 = dx * dx + dz * dz;
                    if best.as_ref().map(|(b, _, _, _)| d2 < *b).unwrap_or(true) {
                        best = Some((d2, *tid, *p, *r));
                    }
                }
                if let Some((_d2, tid, tp, tr)) = best {
                    let to = Vec3::new(tp.x - pos_u.x, 0.0, tp.z - pos_u.z);
                    let dist = to.length();
                    let contact = rad_u + tr + 0.35f32;
                    if dist > contact + 0.02 {
                        let step = (speed * dt).min(dist - contact);
                        if step > 1e-4
                            && let Some(a) = self.actors.get_mut(uid)
                        {
                            a.tr.pos += to.normalize_or_zero() * step;
                        }
                    } else {
                        // Apply melee damage when in range
                        if let Some(t) = self.actors.get_mut(tid) {
                            t.hp.hp = (t.hp.hp - damage).max(0);
                        }
                    }
                }
            }
        }
        // 3) Step projectiles and collide vs NPCs and wizards (friendly fire on)
        let mut i = 0usize;
        while i < self.projectiles.len() {
            let p0 = self.projectiles[i].pos;
            let kind = self.projectiles[i].kind; // copy
            let vel = self.projectiles[i].vel; // copy
            self.projectiles[i].pos = p0 + vel * dt;
            let p1 = self.projectiles[i].pos;
            self.projectiles[i].age += dt;
            let mut removed = false;
            // Resolve projectile spec once for this step to avoid borrow conflicts
            let spec_kind = self.projectile_spec(kind);
            let owner_id = self.projectiles[i].owner;
            // Collide vs actors (direct hit) with deferred AoE to avoid borrow conflicts
            if !removed {
                let mut pending_aoe: Option<(f32, f32, i32)> = None;
                let mut hit_any = false;
                for a in self.actors.iter_mut() {
                    if !a.hp.alive() {
                        continue;
                    }
                    // Skip self for PC-owned projectiles
                    if let Some(owner) = owner_id
                        && owner == a.id
                    {
                        continue;
                    }
                    if segment_hits_circle_xz(p0, p1, a.tr.pos, a.tr.radius) {
                        let spec = spec_kind;
                        if matches!(kind, ProjKind::Fireball) {
                            let cx = p1.x;
                            let cz = p1.z;
                            pending_aoe = Some((cx, cz, spec.damage));
                        } else {
                            let dmg = spec.damage;
                            a.hp.hp = (a.hp.hp - dmg).max(0);
                        }
                        hit_any = true;
                        break;
                    }
                }
                if hit_any {
                    removed = true;
                    if let Some((cx, cz, dmg)) = pending_aoe {
                        let r2 = spec_kind.aoe_radius_m * spec_kind.aoe_radius_m;
                        let _ = self.apply_aoe_at_actors(cx, cz, r2, dmg, owner_id);
                        if std::env::var("RA_LOG_FIREBALL")
                            .map(|v| v == "1")
                            .unwrap_or(false)
                        {
                            log::info!(
                                "server: Fireball impact explode at ({:.2},{:.2},{:.2}) r={:.1} dmg={}",
                                cx,
                                p1.y,
                                cz,
                                spec_kind.aoe_radius_m,
                                dmg
                            );
                        }
                    }
                }
            }
            // (renderer/client wizards are actors; handled in the actor loop above)
            // Fireball proximity explode: if we passed within AoE radius of any NPC or wizard this step
            if !removed && matches!(kind, ProjKind::Fireball) {
                let spec = spec_kind;
                let r2 = spec.aoe_radius_m * spec.aoe_radius_m;
                let seg_a = glam::Vec2::new(p0.x, p0.z);
                let seg_b = glam::Vec2::new(p1.x, p1.z);
                let ab = seg_b - seg_a;
                let len2 = ab.length_squared();
                let mut best_d2 = f32::INFINITY;
                let mut best_center = seg_b;
                for act in &self.actors.actors {
                    if !act.hp.alive() {
                        continue;
                    }
                    let c = glam::Vec2::new(act.tr.pos.x, act.tr.pos.z);
                    let t = if len2 <= 1e-12 {
                        0.0
                    } else {
                        ((c - seg_a).dot(ab) / len2).clamp(0.0, 1.0)
                    };
                    let closest = seg_a + ab * t;
                    let d2 = (closest - c).length_squared();
                    if d2 < best_d2 {
                        best_d2 = d2;
                        best_center = closest;
                    }
                }
                // (All actors already considered above)
                if best_d2 <= r2 {
                    let cx = best_center.x;
                    let cz = best_center.y;
                    // Apply AoE via actors
                    let _hits = self.apply_aoe_at_actors(cx, cz, r2, spec.damage, owner_id);
                    if std::env::var("RA_LOG_FIREBALL")
                        .map(|v| v == "1")
                        .unwrap_or(false)
                    {
                        log::info!(
                            "server: Fireball proximity explode at ({:.2},{:.2},{:.2}) r={:.1} dmg={}",
                            cx,
                            p1.y,
                            cz,
                            spec.aoe_radius_m,
                            spec.damage
                        );
                    }
                    // hostility flip handled in apply_aoe_at_actors
                    removed = true;
                }
            }
            // Fireball timeout explode at current position (server-authoritative)
            if !removed {
                let age = self.projectiles[i].age;
                let life = self.projectiles[i].life;
                if age >= life && matches!(kind, ProjKind::Fireball) {
                    let spec = spec_kind;
                    let r2 = spec.aoe_radius_m * spec.aoe_radius_m;
                    let cx = p1.x;
                    let cz = p1.z;
                    let _hits = self.apply_aoe_at_actors(cx, cz, r2, spec.damage, owner_id);
                    if std::env::var("RA_LOG_FIREBALL")
                        .map(|v| v == "1")
                        .unwrap_or(false)
                    {
                        log::info!(
                            "server: Fireball timeout explode at ({:.2},{:.2},{:.2}) r={:.1} dmg={}",
                            cx,
                            p1.y,
                            cz,
                            spec.aoe_radius_m,
                            spec.damage
                        );
                    }
                    // hostility flip handled in apply_aoe_at_actors
                    removed = true;
                }
            }
            if removed {
                self.projectiles.swap_remove(i);
                continue;
            }
            i += 1;
        }
    }
    /// Spawn an Undead actor (legacy NPC replacement)
    pub fn spawn_undead(&mut self, pos: Vec3, radius: f32, hp: i32) -> ActorId {
        self.actors.spawn(
            ActorKind::Zombie,
            Team::Undead,
            Transform {
                pos,
                yaw: 0.0,
                radius,
            },
            Health { hp, max: hp },
        )
    }
    /// Spawn the unique boss "Nivita of the Undertide" if not present.
    /// Returns the NPC id if spawned or already present.
    pub fn spawn_nivita_unique(&mut self, pos: Vec3) -> Option<ActorId> {
        if let Some(id) = self.nivita_actor_id {
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
        let id = self.actors.spawn(
            ActorKind::Boss,
            Team::Undead,
            Transform {
                pos,
                yaw: 0.0,
                radius,
            },
            Health {
                hp: hp_mid,
                max: hp_mid,
            },
        );
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
        self.nivita_actor_id = Some(id);
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
        let id = self.nivita_actor_id?;
        let n = self.actors.get(id)?;
        let stats = self.nivita_stats.as_ref()?;
        Some(BossStatus {
            name: stats.name.clone(),
            ac: stats.ac,
            hp: n.hp.hp,
            max: n.hp.max,
            pos: n.tr.pos,
        })
    }
    pub fn ring_spawn(&mut self, count: usize, radius: f32, hp: i32) {
        for i in 0..count {
            let a = (i as f32) / (count as f32) * std::f32::consts::TAU;
            let pos = Vec3::new(radius * a.cos(), 0.6, radius * a.sin());
            let _ = self.spawn_undead(pos, 0.95, hp);
        }
    }
    /// Build a consolidated `TickSnapshot` for clients. Until wizard/projectile state
    /// lives here, we include wizard positions from the caller and compute NPC yaw toward
    /// the nearest wizard.
    // Legacy TickSnapshot removed; actor-centric snapshot is canonical.
    pub fn tick_snapshot_actors(&self, tick: u64) -> net_core::snapshot::ActorSnapshot {
        let actors = self
            .actors
            .iter()
            .map(|a| net_core::snapshot::ActorRep {
                id: a.id.0,
                kind: match a.kind {
                    ActorKind::Wizard => 0,
                    ActorKind::Zombie => 1,
                    ActorKind::Boss => 2,
                },
                team: match a.team {
                    Team::Pc => 0,
                    Team::Wizards => 1,
                    Team::Undead => 2,
                    Team::Neutral => 3,
                },
                pos: [a.tr.pos.x, a.tr.pos.y, a.tr.pos.z],
                yaw: a.tr.yaw,
                radius: a.tr.radius,
                hp: a.hp.hp,
                max: a.hp.max,
                alive: a.hp.alive(),
            })
            .collect();
        let projectiles = self
            .projectiles
            .iter()
            .map(|p| net_core::snapshot::ProjectileRep {
                id: p.id,
                kind: match p.kind {
                    ProjKind::Firebolt => 0,
                    ProjKind::Fireball => 1,
                    ProjKind::MagicMissile => 2,
                },
                pos: [p.pos.x, p.pos.y, p.pos.z],
                vel: [p.vel.x, p.vel.y, p.vel.z],
            })
            .collect();
        net_core::snapshot::ActorSnapshot {
            v: 2,
            tick,
            actors,
            projectiles,
        }
    }

    // clone_for_snapshot removed
    // Move toward nearest wizard and attack when in range. Returns (wizard_idx, damage) per hit.
    /*
        pub fn step_npc_ai(&mut self, _dt: f32, _wizards: &[Vec3]) -> Vec<(usize, i32)> { return Vec::new(); }
    /*
            let _t0 = std::time::Instant::now();
            if wizards.is_empty() {
                let ms = _t0.elapsed().as_secs_f64() * 1000.0;
                metrics::histogram!("tick.ms").record(ms);
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
            let _c0 = std::time::Instant::now();
            self.resolve_collisions(wizards);
            let coll_ms = _c0.elapsed().as_secs_f64() * 1000.0;
            metrics::histogram!("collider.ms").record(coll_ms);
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
            let ms = _t0.elapsed().as_secs_f64() * 1000.0;
            metrics::histogram!("tick.ms").record(ms);
            hits
        }
        fn resolve_collisions(&mut self, _wizards: &[Vec3]) { }
    */
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
        */
}

// ============================================================================
// Tests – actors only
// ============================================================================

#[cfg(test)]
mod tests_actor {
    use super::*;
    use glam::Vec3;

    #[test]
    fn spawn_from_dir_scales_speed() {
        let mut srv = ServerState::new();
        srv.spawn_projectile_from_dir(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), ProjKind::Firebolt);
        let p = &srv.projectiles[0];
        assert!(p.vel.z > 20.0, "vel was not scaled: {}", p.vel.z);
    }
}
