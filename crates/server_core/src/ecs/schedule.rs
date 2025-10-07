//! ECS schedule and systems (phase 2).
//!
//! Moves authoritative logic out of `ServerState::step_authoritative` into
//! ordered systems operating on the ECS world + projectiles, with event buses
//! for damage and AoE.

use glam::{Vec2, Vec3};

use crate::actor::{ActorId, ActorKind, Team};
use crate::ServerState;
use crate::ecs::geom::segment_hits_circle_xz;

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
    pub spatial: SpatialGrid,
    pub cmd: crate::ecs::world::CmdBuf,
}

pub struct Schedule;

impl Schedule {
    pub fn run(&mut self, srv: &mut ServerState, ctx: &mut Ctx, wizard_positions: &[Vec3]) {
        // Rebuild spatial grid once
        ctx.spatial.rebuild(srv);
        // Boss movement (keep behavior parity for now)
        crate::systems::boss::boss_seek_and_integrate(srv, ctx.dt, wizard_positions);
        ingest_projectile_spawns(srv, ctx);
        // Apply spawns immediately so integration/collision see them this tick
        srv.ecs.apply_cmds(&mut ctx.cmd);
        ai_move_undead_toward_wizards(srv, ctx, wizard_positions);
        melee_apply_when_contact(srv, ctx);
        homing_update(srv, ctx);
        projectile_integrate_ecs(srv, ctx);
        projectile_collision_ecs(srv, ctx);
        aoe_apply_explosions(srv, ctx);
        faction_flip_on_pc_hits_wizards(srv, ctx);
        apply_damage_to_ecs(srv, ctx);
        cleanup(srv, ctx);
        srv.ecs.apply_cmds(&mut ctx.cmd);
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

fn ingest_projectile_spawns(srv: &mut ServerState, ctx: &mut Ctx) {
    if srv.pending_projectiles.is_empty() { return; }
    let pending: Vec<_> = srv.pending_projectiles.drain(..).collect();
    use std::collections::HashMap;
    // For MagicMissile, pick distinct targets per owner within range
    let mut mm_pools: HashMap<Option<ActorId>, Vec<ActorId>> = HashMap::new();
    for cmd in pending {
        let spec = srv.projectile_spec(cmd.kind);
        let v = cmd.dir.normalize_or_zero() * spec.speed_mps;
        let yaw = cmd.dir.x.atan2(cmd.dir.z);
        let mut homing = None;
        if matches!(cmd.kind, crate::ProjKind::MagicMissile) {
            let owner_team = cmd
                .owner
                .and_then(|id| srv.ecs.get(id).map(|a| a.team))
                .unwrap_or(crate::actor::Team::Pc);
            let entry = mm_pools.entry(cmd.owner).or_insert_with(|| {
                // Build candidate list sorted nearest-first
                let mut cands: Vec<(f32, ActorId)> = srv
                    .ecs
                    .iter()
                    .filter(|a| a.hp.alive() && a.id != cmd.owner.unwrap_or(ActorId(u32::MAX)))
                    .filter(|a| srv.factions.effective_hostile(owner_team, a.team))
                    .map(|a| {
                        let dx = a.tr.pos.x - cmd.pos.x;
                        let dz = a.tr.pos.z - cmd.pos.z;
                        (dx * dx + dz * dz, a.id)
                    })
                    .filter(|(d2, _)| *d2 <= 30.0 * 30.0)
                    .collect();
                cands.sort_by(|l, r| l.0.partial_cmp(&r.0).unwrap_or(std::cmp::Ordering::Equal));
                // store reversed so pop() yields nearest
                cands.into_iter().rev().map(|(_, id)| id).collect()
            });
            if let Some(tid) = entry.pop() {
                homing = Some(crate::ecs::world::Homing { target: tid, turn_rate: 9.0 });
            }
        }
        let comps = crate::ecs::world::Components {
            id: crate::actor::ActorId(0),
            kind: crate::actor::ActorKind::Wizard, // unused for projectile
            team: crate::actor::Team::Neutral,
            tr: crate::actor::Transform { pos: cmd.pos, yaw, radius: 0.1 },
            hp: crate::actor::Health { hp: 1, max: 1 },
            move_speed: None,
            aggro: None,
            attack: None,
            melee: None,
            projectile: Some(crate::ecs::world::Projectile { kind: cmd.kind, ttl_s: spec.life_s, age_s: 0.0 }),
            velocity: Some(crate::ecs::world::Velocity { v }),
            owner: cmd.owner.map(|id| crate::ecs::world::Owner { id }),
            homing,
        };
        ctx.cmd.spawns.push(comps);
    }
}

fn homing_update(srv: &mut ServerState, ctx: &mut Ctx) {
    // Pre-fetch MagicMissile speed to avoid borrow conflicts
    let mm_speed = srv.projectile_spec(crate::ProjKind::MagicMissile).speed_mps.max(0.1);
    let dt = ctx.dt;
    use std::collections::HashMap;
    let pos_map: HashMap<ActorId, Vec3> = srv.ecs.iter().map(|a| (a.id, a.tr.pos)).collect();
    for c in srv.ecs.iter_mut() {
        if let (Some(_proj), Some(vel), Some(hm)) = (c.projectile.as_ref(), c.velocity.as_mut(), c.homing.as_ref()) {
            let Some(tpos) = pos_map.get(&hm.target).copied() else { continue; };
            let to = glam::vec3(tpos.x - c.tr.pos.x, 0.0, tpos.z - c.tr.pos.z);
            let dist2 = to.length_squared();
            if dist2 < 1e-6 { continue; }
            let cur = if vel.v.length_squared() > 1e-6 { vel.v.normalize() } else { glam::vec3(0.0, 0.0, 1.0) };
            let cur_yaw = cur.x.atan2(cur.z);
            let want = to.normalize();
            let want_yaw = want.x.atan2(want.z);
            let two_pi = std::f32::consts::TAU;
            let mut delta = want_yaw - cur_yaw;
            // Wrap to [-PI, PI]
            if delta > std::f32::consts::PI { delta -= two_pi; }
            if delta < -std::f32::consts::PI { delta += two_pi; }
            let max_step = hm.turn_rate * dt;
            let step = delta.clamp(-max_step, max_step);
            let new_yaw = cur_yaw + step;
            let new_dir = glam::vec3(new_yaw.sin(), 0.0, new_yaw.cos());
            let speed = vel.v.length().max(mm_speed);
            vel.v = new_dir * speed;
        }
    }
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

fn projectile_integrate_ecs(srv: &mut ServerState, ctx: &mut Ctx) {
    let fb_aoe_r2 = { let s = srv.projectile_spec(crate::ProjKind::Fireball); (s.aoe_radius_m * s.aoe_radius_m).max(0.0) };
    for c in srv.ecs.iter_mut() {
        if let (Some(proj), Some(vel)) = (c.projectile.as_mut(), c.velocity.as_ref()) {
            c.tr.pos += vel.v * ctx.dt;
            proj.age_s += ctx.dt;
            if proj.age_s >= proj.ttl_s {
                if matches!(proj.kind, crate::ProjKind::Fireball) {
                    ctx.boom.push(ExplodeEvent { center_xz: Vec2::new(c.tr.pos.x, c.tr.pos.z), r2: fb_aoe_r2, src: c.owner.map(|o| o.id) });
                }
                ctx.cmd.despawns.push(c.id);
            }
        }
    }
}

fn projectile_collision_ecs(srv: &mut ServerState, ctx: &mut Ctx) {
    // copy list of ids to iterate without borrow issues
    let fireball_r2 = {
        let s = srv.projectile_spec(crate::ProjKind::Fireball);
        (s.aoe_radius_m * s.aoe_radius_m).max(0.0)
    };
    let mut list = Vec::new();
    for c in srv.ecs.iter() {
        if let (Some(proj), Some(vel)) = (c.projectile.as_ref(), c.velocity.as_ref()) {
            let p1 = c.tr.pos;
            let p0 = p1 - vel.v * ctx.dt;
            list.push((c.id, p0, p1, proj.kind, c.owner.map(|o| o.id)));
        }
    }
    for (pid, p0, p1, kind, owner) in list {
        // test against actors
        let mut hit_any = false;
        for a in srv.ecs.iter() {
            if !a.hp.alive() { continue; }
            if let Some(owner_id) = owner && owner_id == a.id { continue; }
            if segment_hits_circle_xz(p0, p1, a.tr.pos, a.tr.radius) {
                match kind {
                    crate::ProjKind::Fireball => {
                        ctx.boom.push(ExplodeEvent { center_xz: Vec2::new(p1.x, p1.z), r2: fireball_r2, src: owner });
                    }
                    _ => {
                        ctx.dmg.push(DamageEvent { src: owner, dst: a.id, amount: projectile_damage(srv, kind) });
                    }
                }
                hit_any = true;
                break;
            }
        }
        if hit_any {
            ctx.cmd.despawns.push(pid);
        } else if matches!(kind, crate::ProjKind::Fireball) {
            // proximity explode: nearest pass within AoE radius
            let r2 = fireball_r2;
            let seg_a = Vec2::new(p0.x, p0.z);
            let seg_b = Vec2::new(p1.x, p1.z);
            let ab = seg_b - seg_a;
            let len2 = ab.length_squared();
            let mut best_d2 = f32::INFINITY;
            let mut best_center = seg_b;
            let mid = (seg_a + seg_b) * 0.5;
            let seg_half = (seg_b - seg_a).length() * 0.5;
            let query_r = seg_half + r2.sqrt() + 1.0;
            for aid in ctx.spatial.query_circle(mid, query_r) {
                let Some(act) = srv.ecs.get(aid) else { continue; };
                if !act.hp.alive() { continue; }
                let c = Vec2::new(act.tr.pos.x, act.tr.pos.z);
                let t = if len2 <= 1e-12 { 0.0 } else { ((c - seg_a).dot(ab) / len2).clamp(0.0, 1.0) };
                let closest = seg_a + ab * t;
                let d2 = (closest - c).length_squared();
                if d2 < best_d2 { best_d2 = d2; best_center = closest; }
            }
            if best_d2 <= r2 {
                ctx.boom.push(ExplodeEvent { center_xz: best_center, r2, src: owner });
                ctx.cmd.despawns.push(pid);
            }
        }
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

fn cleanup(srv: &mut ServerState, _ctx: &mut Ctx) { srv.ecs.remove_dead(); }

#[inline]
fn projectile_damage(srv: &ServerState, kind: crate::ProjKind) -> i32 {
    srv.projectile_spec(kind).damage
}

#[inline]
fn projectile_damage_aoe(srv: &ServerState) -> i32 {
    srv.projectile_spec(crate::ProjKind::Fireball).damage
}

// segment intersection helper is in ecs::geom

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
        out.sort_by_key(|id| id.0);
        out.into_iter()
    }
}
