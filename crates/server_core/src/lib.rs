//! In‑process NPC state and simple melee AI/collision avoidance.
//!
//! Also hosts simple voxel destructible helpers (see `destructible` module):
//! - Grid raycast via Amanatides & Woo DDA
//! - Carve impact sphere + spawn debris with seeded RNG

use ecs_core::components as ec;
use glam::Vec3;
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

/// Authoritative wizard model backed by the server.
///
/// Index 0 is reserved for the local PC in the demo; additional entries model
/// NPC-controlled wizards. This is a lightweight stand-in for a fuller ECS
/// until the sim integrates component storage for actors.
#[derive(Debug, Clone)]
pub struct Wizard {
    pub id: u32,
    pub pos: Vec3,
    pub yaw: f32,
    pub hp: i32,
    pub max_hp: i32,
    pub kind: u8, // 0=PC, 1=NPC wizard
    pub cast_timer: f32,
}

/// Projectile kind enum.
///
/// IMPORTANT: The server is authoritative over all projectile tuning
/// (speed, lifetime, AoE radius, damage). Clients must never supply
/// gameplay parameters — they only request a kind.
#[derive(Debug, Clone, Copy)]
pub enum ProjKind {
    Firebolt,
    Fireball,
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
    pub owner: Option<u32>,
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
    /// Wizards mirrored from client positions (index 0 is PC for demo).
    pub wizards: Vec<Wizard>,
    /// Live projectiles spawned by wizards.
    pub projectiles: Vec<Projectile>,
    next_proj_id: u32,
    /// When true, NPC wizards target the PC instead of zombies
    pub wizards_hostile_to_pc: bool,
}

impl ServerState {
    #[inline]
    fn apply_aoe_at(&mut self, cx: f32, cz: f32, r2: f32, damage: i32) -> (u32, u32) {
        let mut hit_npc = 0u32;
        for m in &mut self.npcs {
            if !m.alive {
                continue;
            }
            let dx = m.pos.x - cx;
            let dz = m.pos.z - cz;
            if dx * dx + dz * dz <= r2 {
                let before = m.hp;
                m.hp = (m.hp - damage).max(0);
                if m.hp == 0 {
                    m.alive = false;
                }
                hit_npc += 1;
                if std::env::var("RA_LOG_COMBAT")
                    .map(|v| v == "1")
                    .unwrap_or(false)
                {
                    log::info!(
                        "combat: npc {:?} hp {} -> {} by {} at ({:.2},{:.2})",
                        m.id,
                        before,
                        m.hp,
                        damage,
                        cx,
                        cz
                    );
                }
            }
        }
        let mut hit_wiz = 0u32;
        for m in &mut self.wizards {
            if m.hp <= 0 {
                continue;
            }
            let dx = m.pos.x - cx;
            let dz = m.pos.z - cz;
            if dx * dx + dz * dz <= r2 {
                let before = m.hp;
                m.hp = (m.hp - damage).max(0);
                hit_wiz += 1;
                if std::env::var("RA_LOG_COMBAT")
                    .map(|v| v == "1")
                    .unwrap_or(false)
                {
                    log::info!(
                        "combat: wizard id={} hp {} -> {} by {} at ({:.2},{:.2})",
                        m.id,
                        before,
                        m.hp,
                        damage,
                        cx,
                        cz
                    );
                }
            }
        }
        (hit_npc, hit_wiz)
    }
    pub fn new() -> Self {
        Self {
            next_id: 1,
            npcs: Vec::new(),
            nivita_id: None,
            nivita_stats: None,
            wizards: Vec::new(),
            projectiles: Vec::new(),
            next_proj_id: 1,
            wizards_hostile_to_pc: false,
        }
    }
    /// Mirror wizard positions from the client into authoritative state; create entries as needed.
    pub fn sync_wizards(&mut self, wiz_pos: &[Vec3]) {
        // Resize preserving existing HP/yaw/kind where possible
        if self.wizards.len() < wiz_pos.len() {
            let start = self.wizards.len();
            for (i, p) in wiz_pos.iter().copied().enumerate().skip(start) {
                let kind = if i == 0 { 0u8 } else { 1u8 };
                self.wizards.push(Wizard {
                    id: (i as u32) + 1,
                    pos: p,
                    yaw: 0.0,
                    hp: 100,
                    max_hp: 100,
                    kind,
                    cast_timer: 0.0,
                });
            }
        }
        for (i, p) in wiz_pos.iter().copied().enumerate() {
            if let Some(w) = self.wizards.get_mut(i) {
                w.pos = p;
            }
        }
        // Drop extra entries if fewer wizards now
        if self.wizards.len() > wiz_pos.len() {
            self.wizards.truncate(wiz_pos.len());
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
        owner: Option<u32>,
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
        }
    }
    /// Step server-authoritative systems: NPC AI/melee, wizard casts, projectile
    /// integration/collision. Collisions reduce HP for both NPCs and wizards.
    pub fn step_authoritative(&mut self, dt: f32, wizard_positions: &[Vec3]) {
        // Ensure we mirror wizard positions
        self.sync_wizards(wizard_positions);
        // 1) NPC AI (melee hits against wizards)
        let hits = self.step_npc_ai(dt, wizard_positions);
        for (wiz_idx, dmg) in hits {
            if let Some(w) = self.wizards.get_mut(wiz_idx) {
                w.hp = (w.hp - dmg).max(0);
            }
        }
        // 2) Wizard simple casting: non-PC wizards shoot Fire Bolts
        let wiz_len = self.wizards.len();
        for i in 0..wiz_len {
            if i == 0 {
                continue;
            }
            let (pos, hp);
            {
                let w = &mut self.wizards[i];
                pos = w.pos;
                hp = w.hp;
            }
            let mut yaw_local = 0.0f32;
            if hp <= 0 {
                continue;
            }
            // Choose target: PC if hostile_to_pc, else nearest NPC
            let target = if self.wizards_hostile_to_pc && !self.wizards.is_empty() {
                self.wizards[0].pos
            } else {
                let mut best = None::<(f32, Vec3)>;
                for n in &self.npcs {
                    if !n.alive {
                        continue;
                    }
                    let dx = n.pos.x - pos.x;
                    let dz = n.pos.z - pos.z;
                    let d2 = dx * dx + dz * dz;
                    if best.as_ref().map(|(b, _)| d2 < *b).unwrap_or(true) {
                        best = Some((d2, n.pos));
                    }
                }
                best.map(|(_, p)| p).unwrap_or(pos)
            };
            if target != pos {
                let dir = Vec3::new(target.x - pos.x, 0.0, target.z - pos.z);
                if dir.length_squared() > 1e-6 {
                    yaw_local = dir.x.atan2(dir.z);
                }
                let mut fire_now = false;
                {
                    let w = &mut self.wizards[i];
                    w.yaw = yaw_local;
                    w.cast_timer -= dt;
                    if w.cast_timer <= 0.0 {
                        fire_now = true;
                        w.cast_timer = 1.5;
                    }
                }
                if fire_now {
                    // Fire a bolt using projectile DB speed
                    let speed = data_runtime::specs::projectiles::ProjectileSpecDb::load_default()
                        .ok()
                        .and_then(|db| db.actions.get("AtWillLMB").cloned())
                        .map(|s| s.speed_mps)
                        .unwrap_or(40.0);
                    let vel = dir.normalize_or_zero() * speed;
                    let owner = Some(self.wizards[i].id);
                    // Push projectile with owner id so future hostility toggles can reason about source
                    let id = self.next_proj_id;
                    self.next_proj_id = self.next_proj_id.wrapping_add(1);
                    self.projectiles.push(Projectile {
                        id,
                        pos: pos + vel.normalize_or_zero() * 0.3,
                        vel,
                        kind: ProjKind::Firebolt,
                        age: 0.0,
                        life: 1.5,
                        owner,
                    });
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
            // Collide vs NPCs (direct hit)
            for n in &mut self.npcs {
                if !n.alive {
                    continue;
                }
                if segment_hits_circle_xz(p0, p1, n.pos, n.radius) {
                    let spec = spec_kind;
                    if matches!(kind, ProjKind::Fireball) {
                        // AoE explode on impact
                        let r2 = spec.aoe_radius_m * spec.aoe_radius_m;
                        let cx = p1.x;
                        let cz = p1.z;
                        let pc_hp_before = self.wizards.first().map(|w| w.hp);
                        let (hit_npc, hit_wiz) = self.apply_aoe_at(cx, cz, r2, spec.damage);
                        if owner_id == Some(1)
                            && let Some(hp0) = pc_hp_before
                            && let Some(w0) = self.wizards.get_mut(0)
                        {
                            w0.hp = hp0;
                        }
                        if std::env::var("RA_LOG_FIREBALL")
                            .map(|v| v == "1")
                            .unwrap_or(false)
                        {
                            log::info!(
                                "server: Fireball impact explode at ({:.2},{:.2},{:.2}) r={:.1} dmg={} hits(NPCs={},Wiz={})",
                                p1.x,
                                p1.y,
                                p1.z,
                                spec.aoe_radius_m,
                                spec.damage,
                                hit_npc,
                                hit_wiz
                            );
                        }
                        if owner_id == Some(1) {
                            // Flip hostility if any non-PC wizard within AoE
                            let hit_non_pc = self.wizards.iter().skip(1).any(|w| {
                                let dx = w.pos.x - cx;
                                let dz = w.pos.z - cz;
                                dx * dx + dz * dz <= r2
                            });
                            if hit_non_pc {
                                self.wizards_hostile_to_pc = true;
                            }
                            if std::env::var("RA_LOG_COMBAT")
                                .map(|v| v == "1")
                                .unwrap_or(false)
                            {
                                log::info!(
                                    "server: hostility -> PC (impact AoE) hits_wiz={}",
                                    hit_wiz
                                );
                            }
                        }
                    } else {
                        let dmg = spec.damage;
                        n.hp = (n.hp - dmg).max(0);
                        if n.hp == 0 {
                            n.alive = false;
                        }
                    }
                    removed = true;
                    break;
                }
            }
            if !removed {
                // Collide vs wizards (direct hit)
                for w in &mut self.wizards {
                    if w.hp <= 0 {
                        continue;
                    }
                    let r = 0.7f32;
                    if segment_hits_circle_xz(p0, p1, w.pos, r) {
                        let spec = spec_kind;
                        if matches!(kind, ProjKind::Fireball) {
                            // AoE explode on impact
                            let r2 = spec.aoe_radius_m * spec.aoe_radius_m;
                            let cx = p1.x;
                            let cz = p1.z;
                            let pc_hp_before = self.wizards.first().map(|w| w.hp);
                            let (hit_npc, hit_wiz) = self.apply_aoe_at(cx, cz, r2, spec.damage);
                            if owner_id == Some(1)
                                && let Some(hp0) = pc_hp_before
                                && let Some(w0) = self.wizards.get_mut(0)
                            {
                                w0.hp = hp0;
                            }
                            if std::env::var("RA_LOG_FIREBALL")
                                .map(|v| v == "1")
                                .unwrap_or(false)
                            {
                                log::info!(
                                    "server: Fireball impact explode at ({:.2},{:.2},{:.2}) r={:.1} dmg={} hits(NPCs={},Wiz={})",
                                    p1.x,
                                    p1.y,
                                    p1.z,
                                    spec.aoe_radius_m,
                                    spec.damage,
                                    hit_npc,
                                    hit_wiz
                                );
                            }
                            if owner_id == Some(1) {
                                // Flip hostility if any non-PC wizard within AoE
                                let hit_non_pc = self.wizards.iter().skip(1).any(|w| {
                                    let dx = w.pos.x - cx;
                                    let dz = w.pos.z - cz;
                                    dx * dx + dz * dz <= r2
                                });
                                if hit_non_pc {
                                    self.wizards_hostile_to_pc = true;
                                }
                                if std::env::var("RA_LOG_COMBAT")
                                    .map(|v| v == "1")
                                    .unwrap_or(false)
                                {
                                    log::info!(
                                        "server: hostility -> PC (impact wiz) hits_wiz={}",
                                        hit_wiz
                                    );
                                }
                            }
                        } else {
                            let dmg = spec.damage;
                            w.hp = (w.hp - dmg).max(0);
                            if owner_id == Some(1) {
                                self.wizards_hostile_to_pc = true;
                                if std::env::var("RA_LOG_COMBAT")
                                    .map(|v| v == "1")
                                    .unwrap_or(false)
                                {
                                    log::info!("server: hostility -> PC (direct hit wiz)");
                                }
                            }
                        }
                        removed = true;
                        break;
                    }
                }
            }
            // Fireball proximity explode: if we passed within AoE radius of any NPC or wizard this step
            if !removed && matches!(kind, ProjKind::Fireball) {
                let spec = spec_kind;
                let r2 = spec.aoe_radius_m * spec.aoe_radius_m;
                let a = glam::Vec2::new(p0.x, p0.z);
                let b = glam::Vec2::new(p1.x, p1.z);
                let ab = b - a;
                let len2 = ab.length_squared();
                let mut best_d2 = f32::INFINITY;
                let mut best_center = b;
                for m in &self.npcs {
                    if !m.alive {
                        continue;
                    }
                    let c = glam::Vec2::new(m.pos.x, m.pos.z);
                    let t = if len2 <= 1e-12 {
                        0.0
                    } else {
                        ((c - a).dot(ab) / len2).clamp(0.0, 1.0)
                    };
                    let closest = a + ab * t;
                    let d2 = (closest - c).length_squared();
                    if d2 < best_d2 {
                        best_d2 = d2;
                        best_center = closest;
                    }
                }
                // Also consider proximity to wizards (alive)
                for w in &self.wizards {
                    if w.hp <= 0 {
                        continue;
                    }
                    let c = glam::Vec2::new(w.pos.x, w.pos.z);
                    let t = if len2 <= 1e-12 {
                        0.0
                    } else {
                        ((c - a).dot(ab) / len2).clamp(0.0, 1.0)
                    };
                    let closest = a + ab * t;
                    let d2 = (closest - c).length_squared();
                    if d2 < best_d2 {
                        best_d2 = d2;
                        best_center = closest;
                    }
                }
                if best_d2 <= r2 {
                    let cx = best_center.x;
                    let cz = best_center.y;
                    // Apply AoE, but do not damage the PC with their own Fireball
                    let pc_hp_before = self.wizards.first().map(|w| w.hp);
                    let (hit_npc, hit_wiz) = self.apply_aoe_at(cx, cz, r2, spec.damage);
                    if owner_id == Some(1)
                        && let Some(hp0) = pc_hp_before
                        && let Some(w0) = self.wizards.get_mut(0)
                    {
                        w0.hp = hp0;
                    }
                    if std::env::var("RA_LOG_FIREBALL")
                        .map(|v| v == "1")
                        .unwrap_or(false)
                    {
                        log::info!(
                            "server: Fireball proximity explode at ({:.2},{:.2},{:.2}) r={:.1} dmg={} hits(NPCs={},Wiz={})",
                            cx,
                            p1.y,
                            cz,
                            spec.aoe_radius_m,
                            spec.damage,
                            hit_npc,
                            hit_wiz
                        );
                    }
                    if owner_id == Some(1) {
                        // Flip hostility only if any non-PC wizard is within AoE
                        let hit_non_pc = self.wizards.iter().skip(1).any(|w| {
                            let dx = w.pos.x - cx;
                            let dz = w.pos.z - cz;
                            dx * dx + dz * dz <= r2
                        });
                        if hit_non_pc {
                            self.wizards_hostile_to_pc = true;
                        }
                        if std::env::var("RA_LOG_COMBAT")
                            .map(|v| v == "1")
                            .unwrap_or(false)
                        {
                            log::info!("server: hostility -> PC (proximity) hits_wiz={}", hit_wiz);
                        }
                    }
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
                    let (hit_npc, hit_wiz) = self.apply_aoe_at(cx, cz, r2, spec.damage);
                    if std::env::var("RA_LOG_FIREBALL")
                        .map(|v| v == "1")
                        .unwrap_or(false)
                    {
                        log::info!(
                            "server: Fireball timeout explode at ({:.2},{:.2},{:.2}) r={:.1} dmg={} hits(NPCs={},Wiz={})",
                            cx,
                            p1.y,
                            cz,
                            spec.aoe_radius_m,
                            spec.damage,
                            hit_npc,
                            hit_wiz
                        );
                    }
                    if owner_id == Some(1) && hit_wiz > 0 {
                        self.wizards_hostile_to_pc = true;
                        if std::env::var("RA_LOG_COMBAT")
                            .map(|v| v == "1")
                            .unwrap_or(false)
                        {
                            log::info!("server: hostility -> PC (ttl) hits_wiz={}", hit_wiz);
                        }
                    }
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
    /// Build a consolidated `TickSnapshot` for clients. Until wizard/projectile state
    /// lives here, we include wizard positions from the caller and compute NPC yaw toward
    /// the nearest wizard.
    pub fn tick_snapshot(&self, tick: u32) -> net_core::snapshot::TickSnapshot {
        let mut npcs: Vec<net_core::snapshot::NpcRep> = Vec::with_capacity(self.npcs.len());
        for n in &self.npcs {
            // Compute yaw toward nearest wizard if available
            let mut yaw = 0.0f32;
            let mut best_d2 = f32::INFINITY;
            for w in &self.wizards {
                let dx = w.pos.x - n.pos.x;
                let dz = w.pos.z - n.pos.z;
                let d2 = dx * dx + dz * dz;
                if d2 < best_d2 {
                    best_d2 = d2;
                    yaw = dx.atan2(dz);
                }
            }
            npcs.push(net_core::snapshot::NpcRep {
                id: n.id.0,
                archetype: 0,
                pos: [n.pos.x, n.pos.y, n.pos.z],
                yaw,
                radius: n.radius,
                hp: n.hp,
                max: n.max_hp,
                alive: n.alive,
            });
        }
        let wizards: Vec<net_core::snapshot::WizardRep> = self
            .wizards
            .iter()
            .map(|w| net_core::snapshot::WizardRep {
                id: w.id,
                kind: w.kind,
                pos: [w.pos.x, w.pos.y, w.pos.z],
                yaw: w.yaw,
                hp: w.hp,
                max: w.max_hp,
            })
            .collect();
        let boss = self.nivita_status().map(|st| net_core::snapshot::BossRep {
            id: self.nivita_id.map(|i| i.0).unwrap_or(0),
            name: st.name,
            pos: [st.pos.x, st.pos.y, st.pos.z],
            hp: st.hp,
            max: st.max,
            ac: st.ac,
        });
        let projectiles: Vec<net_core::snapshot::ProjectileRep> = self
            .projectiles
            .iter()
            .map(|p| net_core::snapshot::ProjectileRep {
                id: p.id,
                kind: match p.kind {
                    ProjKind::Firebolt => 0,
                    ProjKind::Fireball => 1,
                },
                pos: [p.pos.x, p.pos.y, p.pos.z],
                vel: [p.vel.x, p.vel.y, p.vel.z],
            })
            .collect();
        net_core::snapshot::TickSnapshot {
            v: 1,
            tick,
            wizards,
            npcs,
            projectiles,
            boss,
        }
    }
    /// Move toward nearest wizard and attack when in range. Returns (wizard_idx, damage) per hit.
    pub fn step_npc_ai(&mut self, dt: f32, wizards: &[Vec3]) -> Vec<(usize, i32)> {
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

// ============================================================================
// Tests – keep these at the bottom of crates/server_core/src/lib.rs
// ============================================================================

#[cfg(test)]
mod tests_aoe {
    use super::*;
    use glam::{Vec3, vec3};

    /// Ensure the server has at least the requested number of wizards/NPCs.
    /// If missing, spawn minimal entries. Then normalize HP/max/alive.
    fn ensure_min_entities(s: &mut ServerState, min_wiz: usize, min_npc: usize) {
        // Wizards: use sync_wizards to create entries as needed
        if s.wizards.len() < min_wiz {
            let mut pos: Vec<Vec3> = Vec::with_capacity(min_wiz);
            for _ in 0..min_wiz {
                pos.push(Vec3::ZERO);
            }
            s.sync_wizards(&pos);
        }
        // NPCs
        while s.npcs.len() < min_npc {
            let id = s.spawn_npc(Vec3::ZERO, 0.9, 30);
            let _ = id;
        }
        // Normalize fields relied on by tests
        for w in &mut s.wizards {
            w.hp = 100;
            w.max_hp = 100;
        }
        for n in &mut s.npcs {
            n.hp = 30;
            n.max_hp = 30;
            n.alive = true;
        }
    }

    #[test]
    fn apply_aoe_hits_wizards_and_npcs_and_clamps_hp_and_kills_npcs() {
        let mut s = ServerState::new();
        ensure_min_entities(&mut s, 3, 3);

        // Explosion at origin with radius 6.0 (r2 = 36.0), damage = 28
        let (cx, cz) = (0.0f32, 0.0f32);
        let r = 6.0f32;
        let r2 = r * r;
        let dmg = 28i32;

        // Wizards:
        // - w0 at (1, 1) -> inside -> 100 -> 72
        // - w1 at (5.9, 0) -> inside -> 100 -> 72
        // - w2 at (6.1, 0) -> outside -> stays 100
        s.wizards[0].pos = vec3(1.0, 0.0, 1.0);
        s.wizards[1].pos = vec3(5.9, 0.0, 0.0);
        s.wizards[2].pos = vec3(6.1, 0.0, 0.0);

        // NPCs:
        // - n0 at (0, 2) -> inside; set to 20 hp so it dies
        // - n1 at (7, 0) -> outside -> stays 30 (alive)
        // - n2 at (0, -5.9) -> inside -> 30 -> 2 (alive)
        s.npcs[0].pos = vec3(0.0, 0.0, 2.0);
        s.npcs[1].pos = vec3(7.0, 0.0, 0.0);
        s.npcs[2].pos = vec3(0.0, 0.0, -5.9);
        s.npcs[0].hp = 20; // 20 - 28 => 0 (and alive=false)

        let (hit_npc, hit_wiz) = s.apply_aoe_at(cx, cz, r2, dmg);
        assert_eq!(hit_wiz, 2, "expected to hit two wizards inside r=6.0");
        assert_eq!(hit_npc, 2, "expected to hit two NPCs inside r=6.0");

        // Wizards
        assert_eq!(s.wizards[0].hp, 72);
        assert_eq!(s.wizards[1].hp, 72);
        assert_eq!(s.wizards[2].hp, 100);

        // NPCs
        assert_eq!(s.npcs[0].hp, 0, "n0 should be clamped to 0");
        assert!(
            !s.npcs[0].alive,
            "n0 should be flagged dead when hp reaches 0"
        );

        assert_eq!(s.npcs[2].hp, 2, "n2 should be reduced but remain alive");
        assert!(s.npcs[2].alive);

        assert_eq!(s.npcs[1].hp, 30, "n1 should be untouched outside radius");
        assert!(s.npcs[1].alive);
    }

    #[test]
    fn apply_aoe_respects_inclusive_radius_boundary() {
        let mut s = ServerState::new();
        ensure_min_entities(&mut s, 2, 0);

        // Explosion radius: r = 6.0 (r2 = 36.0)
        let r2 = 36.0f32;

        // Place w0 exactly on boundary (6.0, 0) -> must be hit
        // Place w1 just outside (6.01, 0) -> must not be hit
        s.wizards[0].hp = 50;
        s.wizards[1].hp = 50;
        s.wizards[0].pos = vec3(6.0, 0.0, 0.0);
        s.wizards[1].pos = vec3(6.01, 0.0, 0.0);

        let (_hit_npc, hit_wiz) = s.apply_aoe_at(0.0, 0.0, r2, 10);
        assert_eq!(hit_wiz, 1, "inclusive boundary should count exactly r");
        assert_eq!(s.wizards[0].hp, 40);
        assert_eq!(s.wizards[1].hp, 50);
    }

    #[test]
    fn apply_aoe_skips_dead_targets() {
        let mut s = ServerState::new();
        ensure_min_entities(&mut s, 1, 1);

        // Mark wizard[0] dead-ish (hp=0), place inside radius. Should not count / not go negative.
        s.wizards[0].hp = 0;
        s.wizards[0].pos = vec3(0.5, 0.0, 0.5);

        // NPC[0] alive and in range; verifies we still count the other type
        s.npcs[0].hp = 10;
        s.npcs[0].alive = true;
        s.npcs[0].pos = vec3(0.5, 0.0, -0.5);

        let (hit_npc, hit_wiz) = s.apply_aoe_at(0.0, 0.0, 4.0, 5);
        assert_eq!(hit_wiz, 0, "dead wizards must be skipped");
        assert_eq!(hit_npc, 1, "alive NPC in range must be hit");
        assert_eq!(s.wizards[0].hp, 0, "wizard hp must remain clamped at 0");
        assert_eq!(s.npcs[0].hp, 5, "npc took damage");
        assert!(s.npcs[0].alive, "still alive since hp>0");
    }

    #[test]
    fn spawn_projectile_owned_sets_owner_and_velocity_direction() {
        let mut s = ServerState::new();

        let pos = vec3(0.0, 0.0, 0.0);
        let dir = vec3(1.0, 0.0, 0.0);
        let before = s.projectiles.len();

        s.spawn_projectile_from_dir_owned(pos, dir, ProjKind::Fireball, Some(1));

        assert_eq!(
            s.projectiles.len(),
            before + 1,
            "one projectile should be spawned"
        );
        let p = s.projectiles.last().expect("projectile exists");

        assert!(matches!(p.kind, ProjKind::Fireball));
        assert_eq!(p.owner, Some(1), "owner must be tagged as PC");

        // Velocity should align with input dir (within tight tolerance)
        let v_norm = p.vel.normalize_or_zero();
        let d_norm = dir.normalize_or_zero();
        let dot = v_norm.dot(d_norm);
        assert!(
            dot > 0.999,
            "projectile velocity should align with dir; dot={dot}, v_norm={v_norm:?}, d_norm={d_norm:?}"
        );

        // And magnitude should match spec.speed_mps (within small epsilon)
        let spec = s.projectile_spec(ProjKind::Fireball);
        let speed = p.vel.length();
        assert!(
            (speed - spec.speed_mps).abs() < 1e-3,
            "projectile speed mismatch; got {speed}, expected {}",
            spec.speed_mps
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firebolt_hits_npc_and_reduces_hp() {
        let mut srv = ServerState::new();
        // Spawn one zombie at z=1 in front of origin (large radius so easy hit)
        let id = srv.spawn_npc(Vec3::new(0.0, 0.6, 1.0), 0.95, 20);
        assert_eq!(id.0, 1);
        // Mirror two wizards (PC far away; NPC wizard at origin)
        let wiz_pos = vec![Vec3::new(10.0, 0.6, 10.0), Vec3::new(0.0, 0.6, 0.0)];
        srv.sync_wizards(&wiz_pos);
        if let Some(w) = srv.wizards.get_mut(1) {
            w.cast_timer = 0.0;
        }
        srv.step_authoritative(0.1, &wiz_pos);
        // NPC hp should be reduced
        let n = srv.npcs.iter().find(|n| n.id == id).unwrap();
        assert!(n.hp < n.max_hp, "expected damage applied");
    }

    #[test]
    fn spawn_from_dir_scales_speed() {
        let mut srv = ServerState::new();
        srv.spawn_projectile_from_dir(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), ProjKind::Firebolt);
        let p = &srv.projectiles[0];
        assert!(p.vel.z > 20.0, "vel was not scaled: {}", p.vel.z);
    }

    #[test]
    fn fireball_aoe_damages_ring() {
        let mut srv = ServerState::new();
        // Simple ring around origin within ~3m radius
        for a in [
            0.0,
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::PI,
            3.0 * std::f32::consts::FRAC_PI_2,
        ] {
            srv.spawn_npc(Vec3::new(a.cos() * 3.0, 0.6, a.sin() * 3.0), 0.75, 50);
        }
        // Cast fireball grazing the ring
        srv.spawn_projectile_from_dir(
            Vec3::new(-6.0, 0.6, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            ProjKind::Fireball,
        );
        // Step forward a bit to ensure proximity explode triggers
        for _ in 0..20 {
            srv.step_authoritative(0.05, &[]);
        }
        let any_damaged = srv.npcs.iter().any(|n| n.hp < n.max_hp);
        assert!(any_damaged, "expected at least one NPC to take damage");
    }

    #[test]
    fn fireball_ttl_explodes_and_damages() {
        let mut srv = ServerState::new();
        // Put a target near the end of the projectile path
        let target = srv.spawn_npc(Vec3::new(3.0, 0.6, 0.0), 0.9, 40);
        srv.spawn_projectile_from_dir(
            Vec3::new(0.0, 0.6, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            ProjKind::Fireball,
        );
        // Run simulation long enough for TTL explosion
        for _ in 0..60 {
            srv.step_authoritative(0.05, &[]);
        }
        let n = srv.npcs.iter().find(|n| n.id == target).unwrap();
        assert!(
            n.hp < n.max_hp,
            "target should have taken damage from TTL explode"
        );
    }
}

// Additional focused Fireball authoritative tests covering impact/proximity AoE and hostility flip
#[cfg(test)]
mod tests_fireball {
    use super::*;
    use glam::{Vec3, vec3};

    fn ensure_min_entities(s: &mut ServerState, min_wiz: usize, min_npc: usize) {
        if s.wizards.len() < min_wiz {
            let mut pos = Vec::with_capacity(min_wiz);
            for _ in 0..min_wiz {
                pos.push(Vec3::ZERO);
            }
            s.sync_wizards(&pos);
        }
        while s.npcs.len() < min_npc {
            let _ = s.spawn_npc(Vec3::ZERO, 0.9, 30);
        }
        for w in &mut s.wizards {
            w.hp = 100;
            w.max_hp = 100;
        }
        for n in &mut s.npcs {
            n.hp = 30;
            n.max_hp = 30;
            n.alive = true;
        }
    }

    #[test]
    fn fireball_aoe_hits_wizards_and_npcs_on_impact_and_removes_projectile() {
        let mut s = ServerState::new();
        ensure_min_entities(&mut s, 2, 1);

        // Place wizard[1] and npc[0] near origin so an impact AoE catches both
        s.wizards[1].pos = vec3(0.6, 0.6, 0.6);
        s.npcs[0].pos = vec3(-0.6, 0.6, -0.6);

        // Spawn PC-owned fireball that crosses near (0,0) to impact
        s.spawn_projectile_from_dir_owned(
            vec3(-0.8, 0.6, -0.8),
            vec3(1.0, 0.0, 1.0),
            ProjKind::Fireball,
            Some(1),
        );

        // Authoritative step using mirrored wizard positions
        let wiz_pos: Vec<Vec3> = s.wizards.iter().map(|w| w.pos).collect();
        s.step_authoritative(0.1, &wiz_pos);

        assert!(
            s.wizards[1].hp < 100,
            "wizard should take AoE damage on impact"
        );
        assert!(s.npcs[0].hp < 30, "npc should take AoE damage on impact");
        assert!(
            s.projectiles.is_empty(),
            "fireball must be removed after detonation"
        );
    }

    #[test]
    fn fireball_proximity_aoe_hits_targets_and_removes_projectile() {
        let mut s = ServerState::new();
        ensure_min_entities(&mut s, 2, 1);

        // Targets offset from the centerline; proximity should trigger
        s.wizards[1].pos = vec3(2.5, 0.6, 0.0);
        s.npcs[0].pos = vec3(-2.5, 0.6, 0.0);

        s.spawn_projectile_from_dir_owned(
            vec3(-6.0, 0.6, 0.0),
            vec3(1.0, 0.0, 0.0),
            ProjKind::Fireball,
            Some(1),
        );
        let wiz_pos: Vec<Vec3> = s.wizards.iter().map(|w| w.pos).collect();
        s.step_authoritative(0.2, &wiz_pos);

        assert!(
            s.wizards[1].hp < 100 || s.npcs[0].hp < 30,
            "at least one target must take proximity damage"
        );
        assert!(
            s.projectiles.is_empty(),
            "fireball must be removed after proximity detonation"
        );
    }

    #[test]
    fn owner_pc_flip_hostility_on_wizard_damage() {
        let mut s = ServerState::new();
        ensure_min_entities(&mut s, 2, 0);

        // Put wizard[1] in the path for direct impact -> AoE
        s.wizards[1].pos = vec3(0.2, 0.6, 0.0);
        s.spawn_projectile_from_dir_owned(
            vec3(-0.2, 0.6, 0.0),
            vec3(1.0, 0.0, 0.0),
            ProjKind::Fireball,
            Some(1),
        );

        let wiz_pos: Vec<Vec3> = s.wizards.iter().map(|w| w.pos).collect();
        s.step_authoritative(0.05, &wiz_pos);

        assert!(
            s.wizards_hostile_to_pc,
            "wizard damage by PC should flip hostility to PC"
        );
    }
}
