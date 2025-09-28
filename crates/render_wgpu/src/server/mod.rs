//! In-process server scaffold (copied for renderer isolation).

use glam::Vec3;

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
}

impl Npc {
    pub fn new(id: NpcId, pos: Vec3, radius: f32, hp: i32) -> Self {
        Self { id, pos, radius, hp, max_hp: hp, alive: true, attack_cooldown: 0.0, attack_anim: 0.0 }
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
}

impl ServerState {
    pub fn new() -> Self { Self { next_id: 1, npcs: Vec::new() } }
    pub fn spawn_npc(&mut self, pos: Vec3, radius: f32, hp: i32) -> NpcId {
        let id = NpcId(self.next_id);
        self.next_id += 1;
        self.npcs.push(Npc::new(id, pos, radius, hp));
        id
    }
    pub fn ring_spawn(&mut self, count: usize, radius: f32, hp: i32) {
        for i in 0..count {
            let a = (i as f32) / (count as f32) * std::f32::consts::TAU;
            let pos = Vec3::new(radius * a.cos(), 0.6, radius * a.sin());
            self.spawn_npc(pos, 0.95, hp);
        }
    }

    /// Simple melee step: move NPCs toward nearest wizard and attack when in range.
    /// Returns list of (wizard_idx, damage) applied this step.
    pub fn step_npc_ai(&mut self, dt: f32, wizards: &[Vec3]) -> Vec<(usize, i32)> {
        if wizards.is_empty() {
            return Vec::new();
        }
        let speed = 2.0f32; // m/s
        let wizard_r = 0.7f32;
        let melee_pad = 0.35f32;
        let attack_cd = 1.5f32;
        let attack_anim_time = 0.8f32;
        let damage = 5i32;
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
                if d2 < best_d2 { best_d2 = d2; best_i = i; }
            }
            chosen[idx] = Some(best_i);
            let target = wizards[best_i];
            let to = Vec3::new(target.x - n.pos.x, 0.0, target.z - n.pos.z);
            let dist = to.length();
            let contact = n.radius + wizard_r + melee_pad;
            if dist > contact + 0.02 {
                let step = (speed * dt).min(dist - contact);
                if step > 1e-4 { n.pos += to.normalize() * step; }
            }
        }
        self.resolve_collisions(wizards);
        for (idx, n) in self.npcs.iter_mut().enumerate() {
            if !n.alive { continue; }
            if let Some(best_i) = chosen[idx] {
                let target = wizards[best_i];
                let to = Vec3::new(target.x - n.pos.x, 0.0, target.z - n.pos.z);
                let dist = to.length();
                let contact = n.radius + wizard_r + melee_pad;
                if dist <= contact + 0.05 && n.attack_cooldown <= 0.0 {
                    hits.push((best_i, damage));
                    n.attack_cooldown = attack_cd;
                    n.attack_anim = attack_anim_time;
                }
            }
        }
        hits
    }

    /// Push overlapping NPCs apart and also keep them from overlapping wizards.
    fn resolve_collisions(&mut self, wizards: &[Vec3]) {
        let nlen = self.npcs.len();
        for i in 0..nlen {
            if !self.npcs[i].alive { continue; }
            for j in (i + 1)..nlen {
                if !self.npcs[j].alive { continue; }
                let mut dx = self.npcs[j].pos.x - self.npcs[i].pos.x;
                let mut dz = self.npcs[j].pos.z - self.npcs[i].pos.z;
                let d2 = dx * dx + dz * dz;
                let min_d = self.npcs[i].radius + self.npcs[j].radius;
                if d2 < min_d * min_d {
                    let mut d = d2.sqrt();
                    if d < 1e-4 { dx = 1.0; dz = 0.0; d = 1e-4; }
                    dx /= d; dz /= d;
                    let overlap = min_d - d;
                    let push = overlap * 0.5;
                    self.npcs[i].pos.x -= dx * push; self.npcs[i].pos.z -= dz * push;
                    self.npcs[j].pos.x += dx * push; self.npcs[j].pos.z += dz * push;
                }
            }
        }
        let wiz_r = 0.7f32;
        for n in &mut self.npcs {
            if !n.alive { continue; }
            for w in wizards {
                let mut dx = n.pos.x - w.x;
                let mut dz = n.pos.z - w.z;
                let d2 = dx * dx + dz * dz;
                let min_d = n.radius + wiz_r;
                if d2 < min_d * min_d {
                    let mut d = d2.sqrt();
                    if d < 1e-4 { dx = 1.0; dz = 0.0; d = 1e-4; }
                    dx /= d; dz /= d;
                    let overlap = min_d - d;
                    n.pos.x += dx * overlap; n.pos.z += dz * overlap;
                }
            }
        }
    }

    /// Collide moving projectiles against NPC spheres and apply damage.
    /// Returns a list of hit events; `projectiles` is filtered in place.
    pub fn collide_and_damage(
        &mut self,
        projectiles: &mut Vec<crate::gfx::fx::Projectile>,
        dt: f32,
        damage: i32,
    ) -> Vec<HitEvent> {
        let mut events = Vec::new();
        let mut i = 0;
        'outer: while i < projectiles.len() {
            let pr = &projectiles[i];
            let p0 = pr.pos - pr.vel * dt; // previous position
            let p1 = pr.pos;
            for npc in &mut self.npcs {
                if !npc.alive { continue; }
                if segment_hits_sphere(p0, p1, npc.pos, npc.radius) {
                    let hp_before = npc.hp;
                    let hp_after = (npc.hp - damage).max(0);
                    npc.hp = hp_after;
                    if hp_after == 0 { npc.alive = false; }
                    let fatal = !npc.alive;
                    events.push(HitEvent { npc: npc.id, pos: p1, damage, hp_before, hp_after, fatal });
                    projectiles.swap_remove(i);
                    continue 'outer;
                }
            }
            i += 1;
        }
        events
    }
}

fn segment_hits_sphere(p0: Vec3, p1: Vec3, center: Vec3, r: f32) -> bool {
    let v = p1 - p0;
    let w = center - p0;
    let vv = v.dot(v);
    let t = if vv > 0.0 { (w.dot(v) / vv).clamp(0.0, 1.0) } else { 0.0 };
    let closest = p0 + v * t;
    (closest - center).length() <= r
}
