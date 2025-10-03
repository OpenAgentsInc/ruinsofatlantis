//! Authoritative projectile integration and simple collision helpers.

use ecs_core::components::{CarveRequest, CollisionShape, EntityId, Health, Projectile};
use glam::{Vec3};

/// Integrate projectiles forward by `dt` seconds; returns list of (index, p0, p1).
pub fn integrate(projectiles: &mut [Projectile], dt: f32) -> Vec<(usize, Vec3, Vec3)> {
    let mut segs = Vec::with_capacity(projectiles.len());
    for (i, p) in projectiles.iter_mut().enumerate() {
        if p.life_s <= 0.0 {
            continue;
        }
        let p0 = p.pos;
        p.pos += p.vel * dt;
        p.life_s -= dt;
        let p1 = p.pos;
        segs.push((i, p0, p1));
    }
    segs
}

#[inline]
fn segment_sphere_hit(p0: Vec3, p1: Vec3, center: Vec3, radius: f32) -> bool {
    let d = p1 - p0;
    let m = p0 - center;
    let a = d.dot(d);
    if a <= 1e-6 { return m.length() <= radius; }
    let t = (-(m.dot(d)) / a).clamp(0.0, 1.0);
    let c = p0 + d * t;
    (c - center).length() <= radius
}

#[inline]
fn segment_aabb_enter_t(p0: Vec3, p1: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let d = p1 - p0;
    let mut tmin = 0.0f32;
    let mut tmax = 1.0f32;
    for i in 0..3 {
        let s = p0[i];
        let dir = d[i];
        let minb = min[i];
        let maxb = max[i];
        if dir.abs() < 1e-6 {
            if s < minb || s > maxb { return None; }
        } else {
            let inv = 1.0 / dir;
            let mut t0 = (minb - s) * inv;
            let mut t1 = (maxb - s) * inv;
            if t0 > t1 { core::mem::swap(&mut t0, &mut t1); }
            tmin = tmin.max(t0);
            tmax = tmax.min(t1);
            if tmin > tmax { return None; }
        }
    }
    Some(tmin)
}

/// Resolve collisions for segments against a set of shapes; apply damage and emit carve requests.
pub fn collide_and_damage(
    segs: &[(usize, Vec3, Vec3)],
    projectiles: &mut Vec<Projectile>,
    targets: &mut [(EntityId, CollisionShape, Health)],
    destructs: &[(ecs_core::components::DestructibleId, Vec3, Vec3)],
    out_carves: &mut Vec<CarveRequest>,
) {
    for (pi, p0, p1) in segs.iter().copied() {
        let pr = projectiles[pi];
        // Entities first
        for (_eid, shape, h) in targets.iter_mut() {
            match *shape {
                CollisionShape::Sphere { center, radius } => {
                    if segment_sphere_hit(p0, p1, center, radius) {
                        h.hp = (h.hp - pr.damage).max(0);
                    }
                }
                CollisionShape::CapsuleY { .. } => {
                    // Not implemented in v0
                }
                CollisionShape::Aabb { .. } => {}
            }
        }
        // Destructibles: segment vs AABB, emit CarveRequest on entry
        for (did, min, max) in destructs {
            if let Some(t) = segment_aabb_enter_t(p0, p1, *min, *max) {
                let hit = p0 + (p1 - p0) * t.max(0.0);
                out_carves.push(CarveRequest {
                    did: did.0,
                    center_m: hit.as_dvec3(),
                    radius_m: pr.radius_m as f64,
                    seed: 0,
                    impact_id: 0,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sphere_hit_applies_damage() {
        let owner = EntityId(1);
        let mut projectiles = vec![Projectile { radius_m: 0.2, damage: 10, life_s: 1.0, owner, pos: Vec3::new(-2.0, 0.0, 0.0), vel: Vec3::new(4.0, 0.0, 0.0) }];
        let segs = integrate(&mut projectiles, 0.25);
        let mut targets = vec![(EntityId(2), CollisionShape::Sphere { center: Vec3::ZERO, radius: 0.5 }, Health { hp: 30, max: 30 })];
        let mut carves = Vec::new();
        collide_and_damage(&segs, &mut projectiles, &mut targets, &[], &mut carves);
        assert_eq!(targets[0].2.hp, 20);
        assert!(carves.is_empty());
    }
    #[test]
    fn destructible_hit_emits_carve() {
        let owner = EntityId(1);
        let mut projectiles = vec![Projectile { radius_m: 0.6, damage: 1, life_s: 1.0, owner, pos: Vec3::new(-2.0, 0.0, 0.0), vel: Vec3::new(4.0, 0.0, 0.0) }];
        let segs = integrate(&mut projectiles, 0.25);
        let mut targets: Vec<(EntityId, CollisionShape, Health)> = Vec::new();
        let min = Vec3::new(-1.0, -1.0, -1.0);
        let max = Vec3::new(1.0, 1.0, 1.0);
        let did = ecs_core::components::DestructibleId(1);
        let mut carves = Vec::new();
        collide_and_damage(&segs, &mut projectiles, &mut targets, &[(did, min, max)], &mut carves);
        assert_eq!(carves.len(), 1);
        assert!((carves[0].radius_m - 0.6).abs() < 1e-6);
        assert_eq!(carves[0].did, 1);
    }
}

