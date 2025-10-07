//! ECS schedule and systems (phase 2).
//!
//! Moves authoritative logic out of `ServerState::step_authoritative` into
//! ordered systems operating on the ECS world + projectiles, with event buses
//! for damage and AoE.

use glam::{Vec2, Vec3};

use crate::actor::{ActorId, ActorKind, Team};
use crate::ServerState;

#[derive(Copy, Clone, Debug)]
pub struct DamageEvent {
    pub src: Option<ActorId>,
    pub dst: ActorId,
    pub amount: i32,
}

#[derive(Copy, Clone, Debug)]
pub struct ExplodeEvent {
    pub center_xz: Vec2,
    pub r2: f32,
    pub src: Option<ActorId>,
}

#[derive(Default)]
pub struct Ctx {
    pub dt: f32,
    #[allow(dead_code)]
    pub time_s: f32,
    pub dmg: Vec<DamageEvent>,
    pub boom: Vec<ExplodeEvent>,
    pub remove_proj_ids: Vec<u32>,
    pub spatial: SpatialGrid,
}

pub struct Schedule;

impl Schedule {
    pub fn run(&mut self, srv: &mut ServerState, ctx: &mut Ctx, wizard_positions: &[Vec3]) {
        // Rebuild spatial grid once
        ctx.spatial.rebuild(srv);
        // Boss movement (keep behavior parity for now)
        crate::systems::boss::boss_seek_and_integrate(srv, ctx.dt, wizard_positions);
        ai_move_undead_toward_wizards(srv, ctx, wizard_positions);
        melee_apply_when_contact(srv, ctx);
        projectile_integrate(srv, ctx);
        projectile_collision(srv, ctx);
        aoe_apply_explosions(srv, ctx);
        faction_flip_on_pc_hits_wizards(srv, ctx);
        apply_damage_to_ecs(srv, ctx);
        cleanup(srv, ctx);
    }
}

fn wizard_targets(srv: &ServerState) -> Vec<(ActorId, Vec3, f32)> {
    srv
        .ecs
        .iter()
        .filter(|a| a.hp.alive() && matches!(a.kind, ActorKind::Wizard))
        .map(|a| (a.id, a.tr.pos, a.tr.radius))
        .collect()
}

fn ai_move_undead_toward_wizards(srv: &mut ServerState, ctx: &Ctx, _wizards: &[Vec3]) {
    let wiz = wizard_targets(srv);
    let undead_ids: Vec<ActorId> = srv
        .ecs
        .iter()
        .filter(|a| a.hp.alive() && matches!(a.kind, ActorKind::Zombie))
        .map(|a| a.id)
        .collect();
    for uid in undead_ids {
        let (pos_u, rad_u, speed, extra, aggro_m) = if let Some(a) = srv.ecs.get(uid) {
            (
                a.tr.pos,
                a.tr.radius,
                a.move_speed.map(|s| s.mps).unwrap_or(2.0),
                a.attack.map(|r| r.m).unwrap_or(0.35),
                a.aggro.map(|ag| ag.m),
            )
        } else {
            continue;
        };
        // Find nearest wizard
        let mut best: Option<(f32, Vec3, f32)> = None;
        for (_tid, p, r) in &wiz {
            let dx = p.x - pos_u.x;
            let dz = p.z - pos_u.z;
            let d2 = dx * dx + dz * dz;
            if let Some(a) = aggro_m && d2 > a * a { continue; }
            if best.as_ref().map(|(b, _, _)| d2 < *b).unwrap_or(true) {
                best = Some((d2, *p, *r));
            }
        }
        if let Some((_d2, tp, tr)) = best {
            let to = Vec3::new(tp.x - pos_u.x, 0.0, tp.z - pos_u.z);
            let dist = to.length();
            let contact = rad_u + tr + extra;
            if dist > contact + 0.02 {
                let step = (speed * ctx.dt).min(dist - contact);
                if step > 1e-4
                    && let Some(a) = srv.ecs.get_mut(uid)
                {
                    a.tr.pos += to.normalize_or_zero() * step;
                }
            }
        }
    }
}

fn melee_apply_when_contact(srv: &mut ServerState, ctx: &mut Ctx) {
    let wiz = wizard_targets(srv);
    let undead_ids: Vec<ActorId> = srv
        .ecs
        .iter()
        .filter(|a| a.hp.alive() && matches!(a.kind, ActorKind::Zombie))
        .map(|a| a.id)
        .collect();
    for uid in undead_ids {
        let (pos_u, rad_u, extra, mut cd_ready, cd_total, dmg) = if let Some(a) = srv.ecs.get(uid) {
            (
                a.tr.pos,
                a.tr.radius,
                a.attack.map(|r| r.m).unwrap_or(0.35),
                a.melee.map(|m| m.ready_in_s).unwrap_or(0.0),
                a.melee.map(|m| m.cooldown_s).unwrap_or(0.6),
                a.melee.map(|m| m.damage).unwrap_or(5),
            )
        } else {
            continue;
        };
        let mut best: Option<(ActorId, f32, Vec3, f32)> = None;
        for (tid, p, r) in &wiz {
            let dx = p.x - pos_u.x;
            let dz = p.z - pos_u.z;
            let d2 = dx * dx + dz * dz;
            if best.as_ref().map(|(_, b, _, _)| d2 < *b).unwrap_or(true) {
                best = Some((*tid, d2, *p, *r));
            }
        }
        if let Some((tid, _d2, tp, tr)) = best {
            let to = Vec3::new(tp.x - pos_u.x, 0.0, tp.z - pos_u.z);
            let dist = to.length();
            let reach = rad_u + tr + extra;
            // Cooldown update
            cd_ready = (cd_ready - ctx.dt).max(0.0);
            if dist <= reach && cd_ready <= 0.0 {
                ctx.dmg.push(DamageEvent { src: Some(uid), dst: tid, amount: dmg });
                // write back cooldown
                if let Some(u) = srv.ecs.get_mut(uid)
                    && let Some(m) = &mut u.melee
                {
                    m.ready_in_s = cd_total.max(0.05);
                }
            } else {
                // write back cd after decrement
                if let Some(u) = srv.ecs.get_mut(uid)
                    && let Some(m) = &mut u.melee
                {
                    m.ready_in_s = cd_ready;
                }
            }
        }
    }
}

fn projectile_integrate(srv: &mut ServerState, ctx: &mut Ctx) {
    // Precompute Fireball radius^2 to avoid borrowing conflicts
    let fireball_r2 = {
        let s = srv.projectile_spec(crate::ProjKind::Fireball);
        (s.aoe_radius_m * s.aoe_radius_m).max(0.0)
    };
    for p in &mut srv.projectiles {
        p.pos += p.vel * ctx.dt;
        p.age += ctx.dt;
        if p.age >= p.life {
            if matches!(p.kind, crate::ProjKind::Fireball) {
                ctx.boom.push(ExplodeEvent {
                    center_xz: Vec2::new(p.pos.x, p.pos.z),
                    r2: fireball_r2,
                    src: p.owner,
                });
            }
            ctx.remove_proj_ids.push(p.id);
        }
    }
}

fn projectile_collision(srv: &mut ServerState, ctx: &mut Ctx) {
    // copy list of ids to iterate without borrow issues
    let fireball_r2 = {
        let s = srv.projectile_spec(crate::ProjKind::Fireball);
        (s.aoe_radius_m * s.aoe_radius_m).max(0.0)
    };
    let ids: Vec<u32> = srv.projectiles.iter().map(|p| p.id).collect();
    for pid in ids {
        let Some((idx, p)) = srv.projectiles.iter().enumerate().find(|(_, p)| p.id == pid) else { continue; };
        let p0 = p.pos - p.vel * ctx.dt; // previous pos approximated
        let p1 = p.pos;
        let owner = p.owner;
        // test against actors
        let mut hit_any = false;
        for a in srv.ecs.iter() {
            if !a.hp.alive() { continue; }
            if let Some(owner_id) = owner && owner_id == a.id { continue; }
            if segment_hits_circle_xz(p0, p1, a.tr.pos, a.tr.radius) {
                match p.kind {
                    crate::ProjKind::Fireball => {
                        ctx.boom.push(ExplodeEvent { center_xz: Vec2::new(p1.x, p1.z), r2: fireball_r2, src: owner });
                    }
                    _ => {
                        ctx.dmg.push(DamageEvent { src: owner, dst: a.id, amount: projectile_damage(srv, p.kind) });
                    }
                }
                hit_any = true;
                break;
            }
        }
        if hit_any {
            ctx.remove_proj_ids.push(pid);
        } else if matches!(p.kind, crate::ProjKind::Fireball) {
            // proximity explode: nearest pass within AoE radius
            let r2 = fireball_r2;
            let seg_a = Vec2::new(p0.x, p0.z);
            let seg_b = Vec2::new(p1.x, p1.z);
            let ab = seg_b - seg_a;
            let len2 = ab.length_squared();
            let mut best_d2 = f32::INFINITY;
            let mut best_center = seg_b;
            for act in srv.ecs.iter() {
                if !act.hp.alive() { continue; }
                let c = Vec2::new(act.tr.pos.x, act.tr.pos.z);
                let t = if len2 <= 1e-12 { 0.0 } else { ((c - seg_a).dot(ab) / len2).clamp(0.0, 1.0) };
                let closest = seg_a + ab * t;
                let d2 = (closest - c).length_squared();
                if d2 < best_d2 { best_d2 = d2; best_center = closest; }
            }
            if best_d2 <= r2 {
                ctx.boom.push(ExplodeEvent { center_xz: best_center, r2, src: owner });
                ctx.remove_proj_ids.push(pid);
            }
        }
        let _ = idx; // silence unused warning when optimized differently
    }
}

fn aoe_apply_explosions(srv: &mut ServerState, ctx: &mut Ctx) {
    for e in ctx.boom.drain(..) {
        for a in srv.ecs.iter() {
            if !a.hp.alive() { continue; }
            let dx = a.tr.pos.x - e.center_xz.x;
            let dz = a.tr.pos.z - e.center_xz.y;
            if dx * dx + dz * dz <= e.r2 {
                ctx.dmg.push(DamageEvent { src: e.src, dst: a.id, amount: projectile_damage_aoe(srv) });
            }
        }
    }
}

fn faction_flip_on_pc_hits_wizards(srv: &mut ServerState, ctx: &mut Ctx) {
    for d in &ctx.dmg {
        if let Some(src) = d.src
            && let (Some(sa), Some(v)) = (srv.ecs.get(src), srv.ecs.get(d.dst))
            && sa.team == Team::Pc && v.team == Team::Wizards
        {
            srv.factions.pc_vs_wizards_hostile = true;
        }
    }
}

fn apply_damage_to_ecs(srv: &mut ServerState, ctx: &mut Ctx) {
    for d in ctx.dmg.drain(..) {
        if let Some(a) = srv.ecs.get_mut(d.dst) {
            a.hp.hp = (a.hp.hp - d.amount).max(0);
        }
    }
}

fn cleanup(srv: &mut ServerState, ctx: &mut Ctx) {
    if !ctx.remove_proj_ids.is_empty() {
        use std::collections::HashSet;
        let set: HashSet<u32> = ctx.remove_proj_ids.drain(..).collect();
        srv.projectiles.retain(|p| !set.contains(&p.id));
    }
    srv.ecs.remove_dead();
}

#[inline]
fn projectile_damage(srv: &ServerState, kind: crate::ProjKind) -> i32 {
    srv.projectile_spec(kind).damage
}

#[inline]
fn projectile_damage_aoe(srv: &ServerState) -> i32 {
    srv.projectile_spec(crate::ProjKind::Fireball).damage
}

#[inline]
fn segment_hits_circle_xz(p0: Vec3, p1: Vec3, center: Vec3, radius: f32) -> bool {
    let a = Vec2::new(p0.x, p0.z);
    let b = Vec2::new(p1.x, p1.z);
    let c = Vec2::new(center.x, center.z);
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 <= 1e-12 {
        return (a - c).length_squared() <= radius * radius;
    }
    let t = ((c - a).dot(ab) / len2).clamp(0.0, 1.0);
    let closest = a + ab * t;
    (closest - c).length_squared() <= radius * radius
}

// ----------------------------------------------------------------------------
// Spatial grid (2D XZ uniform grid) for broad-phase queries
// ----------------------------------------------------------------------------

use std::collections::HashMap;

#[derive(Default)]
pub struct SpatialGrid {
    cell: f32,
    buckets: HashMap<(i32, i32), Vec<ActorId>>,
}

impl SpatialGrid {
    pub fn rebuild(&mut self, srv: &ServerState) {
        self.cell = 4.0; // meters per cell
        self.buckets.clear();
        for a in srv.ecs.iter() {
            let key = self.key(a.tr.pos.x, a.tr.pos.z);
            self.buckets.entry(key).or_default().push(a.id);
        }
    }
    fn key(&self, x: f32, z: f32) -> (i32, i32) {
        let cx = (x / self.cell).floor() as i32;
        let cz = (z / self.cell).floor() as i32;
        (cx, cz)
    }
    #[allow(dead_code)]
    pub fn query_circle(&self, center: Vec2, r: f32) -> impl Iterator<Item = ActorId> + '_ {
        let cr = (r / self.cell).ceil() as i32;
        let (cx, cz) = ((center.x / self.cell).floor() as i32, (center.y / self.cell).floor() as i32);
        let mut out: Vec<ActorId> = Vec::new();
        for dx in -cr..=cr {
            for dz in -cr..=cr {
                if let Some(v) = self.buckets.get(&(cx + dx, cz + dz)) {
                    out.extend_from_slice(v);
                }
            }
        }
        out.into_iter()
    }
}
