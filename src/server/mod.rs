//! Server-side simulation scaffold (in-process for now).
//!
//! This module holds authoritative NPC state (positions, health) and performs
//! simple projectile collision and damage resolution. It is intentionally
//! decoupled from rendering so it can be moved to a standalone crate/process
//! later.

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
}

impl Npc {
    pub fn new(id: NpcId, pos: Vec3, radius: f32, hp: i32) -> Self {
        Self { id, pos, radius, hp, max_hp: hp, alive: true, attack_cooldown: 0.0 }
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

    /// Simple melee step: move NPCs toward nearest wizard and attack when in range.
    /// Returns list of (wizard_idx, damage) applied this step.
    pub fn step_npc_ai(&mut self, dt: f32, wizards: &[Vec3]) -> Vec<(usize, i32)> {
        if wizards.is_empty() { return Vec::new(); }
        let speed = 2.0f32; // m/s
        let attack_range = 1.2f32;
        let attack_cd = 1.5f32;
        let damage = 5i32;
        let mut hits = Vec::new();
        for n in &mut self.npcs {
            if !n.alive { continue; }
            // reduce cooldown
            n.attack_cooldown = (n.attack_cooldown - dt).max(0.0);
            // nearest wizard in XZ
            let mut best_i = 0usize;
            let mut best_d2 = f32::INFINITY;
            for (i, w) in wizards.iter().enumerate() {
                let dx = w.x - n.pos.x;
                let dz = w.z - n.pos.z;
                let d2 = dx*dx + dz*dz;
                if d2 < best_d2 { best_d2 = d2; best_i = i; }
            }
            let target = wizards[best_i];
            let to = Vec3::new(target.x - n.pos.x, 0.0, target.z - n.pos.z);
            let dist = to.length();
            if dist > 1e-3 {
                let step = (speed * dt).min(dist);
                n.pos += to.normalize() * step;
            }
            if dist <= attack_range && n.attack_cooldown <= 0.0 {
                hits.push((best_i, damage));
                n.attack_cooldown = attack_cd;
            }
        }
        hits
    }
    pub fn ring_spawn(&mut self, count: usize, radius: f32, hp: i32) {
        for i in 0..count {
            let a = (i as f32) / (count as f32) * std::f32::consts::TAU;
            // Raise to half-height so cubes sit on ground visually
            let pos = Vec3::new(radius * a.cos(), 0.6, radius * a.sin());
            // Cube scale ~1.2 => half extent ~0.6, bounding circle ~0.6*sqrt(2) ~ 0.85.
            // Add projectile radius (~0.1) => ~0.95 to reduce miss-through.
            self.spawn_npc(pos, 0.95, hp);
        }
    }

    /// Collide moving projectiles against NPC spheres and apply damage.
    /// Returns a list of hit events; `projectiles` is filtered in place to
    /// remove those that hit.
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
                    // remove projectile
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
    // Treat collision in XZ plane (ignore Y) like a cylinder to make gameplay forgiving.
    segment_hits_circle_xz(glam::vec2(p0.x, p0.z), glam::vec2(p1.x, p1.z), glam::vec2(center.x, center.z), r)
}

fn segment_hits_circle_xz(p0: glam::Vec2, p1: glam::Vec2, c: glam::Vec2, r: f32) -> bool {
    let d = p1 - p0;
    let m = p0 - c;
    let a = d.dot(d);
    if a <= 1e-6 { return m.length() <= r; }
    let t = -(m.dot(d)) / a;
    let t = t.clamp(0.0, 1.0);
    let closest = p0 + d * t;
    (closest - c).length() <= r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_sphere_intersects() {
        // XZ circle: path across origin should intersect regardless of Y difference
        let c = Vec3::new(0.0, 10.0, 0.0);
        let p0 = Vec3::new(-2.0, 1.6, 0.0);
        let p1 = Vec3::new(2.0, 1.6, 0.0);
        assert!(segment_hits_sphere(p0, p1, c, 0.5));
    }

    #[test]
    fn server_applies_damage_and_kills() {
        let mut s = ServerState::new();
        let id = s.spawn_npc(Vec3::ZERO, 0.95, 10);
        let mut projs = vec![crate::gfx::fx::Projectile{ pos: Vec3::new(0.25, 0.0, 0.0), vel: Vec3::new(1.0, 0.0, 0.0), t_die: 1.0, owner_wizard: None }];
        let ev = s.collide_and_damage(&mut projs, 0.1, 10);
        assert!(projs.is_empty());
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].npc, id);
        assert!(ev[0].fatal);
        assert!(!s.npcs[0].alive);
        assert_eq!(s.npcs[0].hp, 0);
    }

    #[test]
    fn diagonal_graze_hits_due_to_radius_padding() {
        let mut s = ServerState::new();
        // Target at origin with larger radius ~0.95
        s.spawn_npc(Vec3::ZERO, 0.95, 10);
        // Projectile path just outside cube half-extent but within padded circle
        // Simulate a step where p0 -> p1 crosses near the circle's edge
        // Here p1 is current position (after integration in the runtime), dt=0.5s
        let mut projs = vec![crate::gfx::fx::Projectile{ pos: Vec3::new(-1.0, 0.0, 0.8), vel: Vec3::new(-5.0, 0.0, 0.0), t_die: 1.0, owner_wizard: None }];
        let ev = s.collide_and_damage(&mut projs, 0.5, 5);
        assert_eq!(ev.len(), 1);
    }
}
