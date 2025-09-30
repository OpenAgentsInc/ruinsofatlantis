//! CPU-side update helpers extracted from gfx/mod.rs

use crate::gfx::Renderer;
use crate::gfx::types::{InstanceSkin, ParticleInstance};
use crate::gfx::{self, anim, fx::Particle, terrain};
use crate::server_ext::CollideProjectiles;
use ra_assets::types::AnimClip;

impl Renderer {
    #[inline]
    pub(crate) fn wrap_angle(a: f32) -> f32 {
        let mut x = a;
        while x > std::f32::consts::PI {
            x -= 2.0 * std::f32::consts::PI;
        }
        while x < -std::f32::consts::PI {
            x += 2.0 * std::f32::consts::PI;
        }
        x
    }
    // update_player_and_camera removed: moved to client_runtime::SceneInputs

    pub(crate) fn apply_pc_transform(&mut self) {
        if !self.pc_alive || self.pc_index >= self.wizard_count as usize {
            return;
        }
        // Update CPU model matrix and upload only the PC instance
        let rot = glam::Quat::from_rotation_y(self.player.yaw);
        // Project player onto terrain height
        let (h, _n) = terrain::height_at(&self.terrain_cpu, self.player.pos.x, self.player.pos.z);
        let pos = glam::vec3(self.player.pos.x, h, self.player.pos.z);
        let m = glam::Mat4::from_scale_rotation_translation(glam::Vec3::splat(1.0), rot, pos);
        self.wizard_models[self.pc_index] = m;
        let mut inst = self.wizard_instances_cpu[self.pc_index];
        inst.model = m.to_cols_array_2d();
        self.wizard_instances_cpu[self.pc_index] = inst;
        let offset = (self.pc_index * std::mem::size_of::<InstanceSkin>()) as u64;
        self.queue
            .write_buffer(&self.wizard_instances, offset, bytemuck::bytes_of(&inst));
    }

    pub(crate) fn update_wizard_palettes(&mut self, time_global: f32) {
        // Build palettes for each wizard with its animation + offset.
        if self.wizard_count == 0 {
            return;
        }
        let joints = self.joints_per_wizard as usize;
        let mut mats: Vec<glam::Mat4> = Vec::with_capacity(self.wizard_count as usize * joints);
        for i in 0..(self.wizard_count as usize) {
            let clip = self.select_clip(self.wizard_anim_index[i]);
            let palette = if self.pc_alive
                && i == self.pc_index
                && self.pc_index < self.wizard_count as usize
            {
                if let Some(start) = self.pc_anim_start {
                    let lt = (time_global - start).clamp(0.0, clip.duration.max(0.0));
                    anim::sample_palette(&self.skinned_cpu, clip, lt)
                } else {
                    anim::sample_palette(&self.skinned_cpu, clip, time_global)
                }
            } else {
                let t = time_global + self.wizard_time_offset[i];
                anim::sample_palette(&self.skinned_cpu, clip, t)
            };
            mats.extend(palette);
        }
        // Upload as raw f32x16
        let mut raw: Vec<[f32; 16]> = Vec::with_capacity(mats.len());
        for m in mats {
            raw.push(m.to_cols_array());
        }
        self.queue
            .write_buffer(&self.palettes_buf, 0, bytemuck::cast_slice(&raw));
    }

    pub(crate) fn select_clip(&self, idx: usize) -> &AnimClip {
        // Honor the requested clip first; fallback only if missing.
        let requested = match idx {
            0 => "PortalOpen",
            1 => "Still",
            _ => "Waiting",
        };
        if let Some(c) = self.skinned_cpu.animations.get(requested) {
            return c;
        }
        for name in ["Waiting", "Still", "PortalOpen"] {
            if let Some(c) = self.skinned_cpu.animations.get(name) {
                return c;
            }
        }
        self.skinned_cpu
            .animations
            .values()
            .next()
            .expect("at least one animation clip present")
    }

    pub(crate) fn process_pc_cast(&mut self, t: f32) {
        if !self.pc_alive || self.pc_index >= self.wizard_count as usize {
            return;
        }
        if self.pc_cast_queued {
            self.pc_cast_queued = false;
            if self.wizard_anim_index[self.pc_index] != 0 && self.pc_anim_start.is_none() {
                // Start PortalOpen now
                self.wizard_anim_index[self.pc_index] = 0;
                self.wizard_time_offset[self.pc_index] = -t; // phase=0 at start
                self.wizard_last_phase[self.pc_index] = 0.0;
                self.pc_anim_start = Some(t);
                self.pc_cast_fired = false;
            }
        }
        if let Some(start) = self.pc_anim_start {
            if self.wizard_anim_index[self.pc_index] == 0 {
                let clip = self.select_clip(0);
                let elapsed = t - start;
                // Fire exactly at cast end if not yet fired
                if !self.pc_cast_fired && elapsed >= self.pc_cast_time {
                    let phase = self.pc_cast_time;
                    if let Some(origin_local) = self.right_hand_world(clip, phase) {
                        let inst = self
                            .wizard_models
                            .get(self.pc_index)
                            .copied()
                            .unwrap_or(glam::Mat4::IDENTITY);
                        let origin_w = inst
                            * glam::Vec4::new(origin_local.x, origin_local.y, origin_local.z, 1.0);
                        let dir_w = (inst * glam::Vec4::new(0.0, 0.0, 1.0, 0.0))
                            .truncate()
                            .normalize_or_zero();
                        let right_w = (inst * glam::Vec4::new(1.0, 0.0, 0.0, 0.0))
                            .truncate()
                            .normalize_or_zero();
                        let lateral = 0.20;
                        let spawn = origin_w.truncate() + dir_w * 0.3 - right_w * lateral;
                        match self.pc_cast_kind.unwrap_or(super::super::PcCast::FireBolt) {
                            super::super::PcCast::FireBolt => {
                                let fb_col = [2.6, 0.7, 0.18];
                                self.spawn_firebolt(
                                    spawn,
                                    dir_w,
                                    t,
                                    Some(self.pc_index),
                                    false,
                                    fb_col,
                                );
                                // Start cooldown via SceneInputs (single source of truth)
                                let spell_id = "wiz.fire_bolt.srd521";
                                self.scene_inputs.start_cooldown(
                                    spell_id,
                                    self.last_time,
                                    self.firebolt_cd_dur,
                                );
                            }
                            super::super::PcCast::MagicMissile => {
                                self.spawn_magic_missile(spawn, dir_w, t);
                                // Start cooldown via SceneInputs
                                let spell_id = "wiz.magic_missile.srd521";
                                self.scene_inputs.start_cooldown(
                                    spell_id,
                                    self.last_time,
                                    self.magic_missile_cd_dur,
                                );
                            }
                        }
                        self.pc_cast_fired = true;
                    }
                    // End cast animation and start cooldown window
                    self.wizard_anim_index[self.pc_index] = 1;
                    self.pc_anim_start = None;
                }
            } else {
                self.pc_anim_start = None;
            }
        }
    }

    /// Update and render-side state for projectiles/particles
    pub(crate) fn update_fx(&mut self, t: f32, dt: f32) {
        // 1) Spawn firebolts for PortalOpen phase crossing (NPC wizards only).
        if self.wizard_count > 0 {
            let zombies_alive = self.any_zombies_alive();
            let cycle = 5.0f32; // synthetic cycle period
            let bolt_offset = 1.5f32; // trigger point in the cycle
            for i in 0..(self.wizard_count as usize) {
                if self.wizard_anim_index[i] != 0 {
                    continue;
                }
                let prev = self.wizard_last_phase[i];
                let phase = (t + self.wizard_time_offset[i]) % cycle;
                let crossed = (prev <= bolt_offset && phase >= bolt_offset)
                    || (prev > phase && (prev <= bolt_offset || phase >= bolt_offset));
                // If wizards have aggroed on the player, they may fire even without zombies present
                let allowed = i == self.pc_index || zombies_alive || self.wizards_hostile_to_pc;
                if allowed && crossed && i != self.pc_index {
                    let clip = self.select_clip(self.wizard_anim_index[i]);
                    let clip_time = if clip.duration > 0.0 {
                        phase.min(clip.duration)
                    } else {
                        0.0
                    };
                    if let Some(origin_local) = self.right_hand_world(clip, clip_time) {
                        let inst = self
                            .wizard_models
                            .get(i)
                            .copied()
                            .unwrap_or(glam::Mat4::IDENTITY);
                        let origin_w = inst
                            * glam::Vec4::new(origin_local.x, origin_local.y, origin_local.z, 1.0);
                        let dir_w = (inst * glam::Vec4::new(0.0, 0.0, 1.0, 0.0))
                            .truncate()
                            .normalize_or_zero();
                        let right_w = (inst * glam::Vec4::new(1.0, 0.0, 0.0, 0.0))
                            .truncate()
                            .normalize_or_zero();
                        let lateral = 0.20;
                        let spawn = origin_w.truncate() + dir_w * 0.3 - right_w * lateral;
                        let fb_col = [2.6, 0.7, 0.18];
                        self.spawn_firebolt(spawn, dir_w, t, Some(i), true, fb_col);
                    }
                }
                self.wizard_last_phase[i] = phase;
            }
        }

        // 2) Integrate projectiles and keep them slightly above ground
        let ground_clearance = 0.15f32; // meters above terrain
        for p in &mut self.projectiles {
            p.pos += p.vel * dt;
            p.pos = gfx::util::clamp_above_terrain(&self.terrain_cpu, p.pos, ground_clearance);
        }
        // 2.5) Server-side collision vs NPCs
        if !self.projectiles.is_empty() && !self.server.npcs.is_empty() {
            let damage = 10; // TODO: integrate with spell spec dice
            let hits = self
                .server
                .collide_and_damage(&mut self.projectiles, dt, damage);
            for h in &hits {
                // Impact burst at hit position
                for _ in 0..16 {
                    let a = rand_unit() * std::f32::consts::TAU;
                    let r = 4.0 + rand_unit() * 1.2;
                    self.particles.push(Particle {
                        pos: h.pos,
                        vel: glam::vec3(a.cos() * r, 2.0 + rand_unit() * 1.2, a.sin() * r),
                        age: 0.0,
                        life: 0.18,
                        size: 0.02,
                        color: [1.7, 0.85, 0.35],
                    });
                }
                // Damage floater above NPC head (terrain/instance-aware)
                // 1) Death Knight (handle first so we can despawn on fatal)
                if self.dk_id.is_some() && self.dk_id.unwrap() == h.npc {
                    // Spawn damage near DK head using its model matrix if present
                    if let Some(m) = self.dk_models.first().copied() {
                        let head = m * glam::Vec4::new(0.0, 1.6, 0.0, 1.0);
                        self.damage.spawn(head.truncate(), h.damage);
                    } else {
                        self.damage
                            .spawn(h.pos + glam::vec3(0.0, 1.2, 0.0), h.damage);
                    }
                    // If fatal, hide the DK instance and clear id
                    if h.fatal {
                        self.dk_count = 0;
                        self.dk_id = None;
                    }
                } else if let Some(idx) = self.zombie_ids.iter().position(|id| *id == h.npc) {
                    let m = self
                        .zombie_models
                        .get(idx)
                        .copied()
                        .unwrap_or(glam::Mat4::IDENTITY);
                    let head = m * glam::Vec4::new(0.0, 1.6, 0.0, 1.0);
                    self.damage.spawn(head.truncate(), h.damage);
                    // Remove zombie visuals if fatal
                    if h.fatal {
                        self.zombie_ids.swap_remove(idx);
                        self.zombie_models.swap_remove(idx);
                        if (idx as u32) < self.zombie_count {
                            self.zombie_instances_cpu.swap_remove(idx);
                            self.zombie_count -= 1;
                            // Recompute palette_base for contiguity
                            for (i, inst) in self.zombie_instances_cpu.iter_mut().enumerate() {
                                inst.palette_base = (i as u32) * self.zombie_joints;
                            }
                            let bytes: &[u8] = bytemuck::cast_slice(&self.zombie_instances_cpu);
                            self.queue.write_buffer(&self.zombie_instances, 0, bytes);
                        }
                    }
                } else if let Some(n) = self.server.npcs.iter().find(|n| n.id == h.npc) {
                    let (hgt, _n) = terrain::height_at(&self.terrain_cpu, n.pos.x, n.pos.z);
                    let pos = glam::vec3(n.pos.x, hgt + n.radius + 0.9, n.pos.z);
                    self.damage.spawn(pos, h.damage);
                } else {
                    self.damage
                        .spawn(h.pos + glam::vec3(0.0, 0.9, 0.0), h.damage);
                    let (hgt, _n) = terrain::height_at(&self.terrain_cpu, h.pos.x, h.pos.z);
                    self.damage
                        .spawn(glam::vec3(h.pos.x, hgt + 0.9, h.pos.z), h.damage);
                }
            }
        }
        // Ground hit or timeout
        let mut burst: Vec<Particle> = Vec::new();
        let mut i = 0;
        while i < self.projectiles.len() {
            let kill = t >= self.projectiles[i].t_die;
            if kill {
                let hit = self.projectiles[i].pos;
                // small flare + compact burst
                burst.push(Particle {
                    pos: hit,
                    vel: glam::Vec3::ZERO,
                    age: 0.0,
                    life: 0.12,
                    size: 0.06,
                    color: [1.8, 1.2, 0.4],
                });
                for _ in 0..10 {
                    let a = rand_unit() * std::f32::consts::TAU;
                    let r = 3.0 + rand_unit() * 0.8;
                    burst.push(Particle {
                        pos: hit,
                        vel: glam::vec3(a.cos() * r, 1.5 + rand_unit() * 1.0, a.sin() * r),
                        age: 0.0,
                        life: 0.12,
                        size: 0.015,
                        color: [1.6, 0.9, 0.3],
                    });
                }
                self.projectiles.swap_remove(i);
            } else {
                i += 1;
            }
        }
        if !burst.is_empty() {
            self.particles.append(&mut burst);
        }

        // 2.6) Collide with wizards/PC (friendly fire on)
        if !self.projectiles.is_empty() {
            self.collide_with_wizards(dt, 10);
        }

        // 3) Simulate impact particles (age, simple gravity, fade)
        let cam = self.cam_follow.current_pos;
        let max_d2 = 400.0 * 400.0; // cull far particles
        let mut j = 0usize;
        while j < self.particles.len() {
            let p = &mut self.particles[j];
            p.age += dt;
            p.vel.y -= 9.8 * dt * 0.5;
            p.vel *= 0.98f32.powf(dt.max(0.0) * 60.0);
            p.pos += p.vel * dt;
            if p.age >= p.life || (p.pos - cam).length_squared() > max_d2 {
                self.particles.swap_remove(j);
                continue;
            }
            j += 1;
        }

        // 4) Upload FX instances (billboard particles)
        let mut inst: Vec<ParticleInstance> =
            Vec::with_capacity(self.projectiles.len() * 3 + self.particles.len());
        for pr in &self.projectiles {
            // Fade head near lifetime end
            let mut head_fade = 1.0f32;
            let fade_window = 0.15f32;
            if pr.t_die > 0.0 {
                let remain = (pr.t_die - t).max(0.0);
                head_fade = (remain / fade_window).clamp(0.0, 1.0);
            }
            // head
            inst.push(ParticleInstance {
                pos: [pr.pos.x, pr.pos.y, pr.pos.z],
                size: 0.18,
                color: [
                    pr.color[0] * head_fade,
                    pr.color[1] * head_fade,
                    pr.color[2] * head_fade,
                ],
                _pad: 0.0,
            });
            // short trail segments behind
            let dir = pr.vel.normalize_or_zero();
            for k in 1..=2 {
                let tseg = k as f32 * 0.02;
                let p = pr.pos - dir * (tseg * pr.vel.length());
                let fade = (1.0 - (k as f32) * 0.35) * head_fade;
                inst.push(ParticleInstance {
                    pos: [p.x, p.y, p.z],
                    size: 0.13,
                    color: [
                        pr.color[0] * 0.8 * fade,
                        pr.color[1] * 0.8 * fade,
                        pr.color[2] * 0.8 * fade,
                    ],
                    _pad: 0.0,
                });
            }
        }
        // Impacts
        for p in &self.particles {
            let f = 1.0 - (p.age / p.life).clamp(0.0, 1.0);
            let size = p.size * (1.0 + 0.5 * (1.0 - f));
            inst.push(ParticleInstance {
                pos: [p.pos.x, p.pos.y, p.pos.z],
                size,
                color: [
                    p.color[0] * f * 1.5,
                    p.color[1] * f * 1.5,
                    p.color[2] * f * 1.5,
                ],
                _pad: 0.0,
            });
        }
        if (inst.len() as u32) > self._fx_capacity {
            inst.truncate(self._fx_capacity as usize);
        }
        self.fx_count = inst.len() as u32;
        if self.fx_count > 0 {
            self.queue
                .write_buffer(&self.fx_instances, 0, bytemuck::cast_slice(&inst));
        }

        // 5) If no zombies remain, retire NPC wizards from the casting loop unless hostile to player
        if !self.any_zombies_alive() && !self.wizards_hostile_to_pc {
            for i in 0..(self.wizard_count as usize) {
                if i == self.pc_index {
                    continue;
                }
                if self.wizard_anim_index[i] == 0 {
                    self.wizard_anim_index[i] = 2;
                }
            }
        }
    }

    pub(crate) fn collide_with_wizards(&mut self, dt: f32, damage: i32) {
        let mut i = 0usize;
        while i < self.projectiles.len() {
            let pr = self.projectiles[i];
            let p0 = pr.pos - pr.vel * dt;
            let p1 = pr.pos;
            let mut hit_someone = false;
            for j in 0..(self.wizard_count as usize) {
                if Some(j) == pr.owner_wizard {
                    continue;
                } // do not hit the caster
                let hp = self.wizard_hp.get(j).copied().unwrap_or(self.wizard_hp_max);
                if hp <= 0 {
                    continue;
                }
                let m = self.wizard_models[j].to_cols_array();
                let center = glam::vec3(m[12], m[13], m[14]);
                let r = 0.7f32; // generous cylinder radius
                if segment_hits_circle_xz(p0, p1, center, r) {
                    let before = self.wizard_hp[j];
                    let after = (before - damage).max(0);
                    self.wizard_hp[j] = after;
                    let fatal = after == 0;
                    // Floating damage number
                    let head = center + glam::vec3(0.0, 1.7, 0.0);
                    self.damage.spawn(head, damage);
                    // If the player hit any wizard, all wizards become hostile to the player
                    if pr.owner_wizard == Some(self.pc_index) {
                        self.wizards_hostile_to_pc = true;
                    }
                    if fatal {
                        if j == self.pc_index {
                            self.kill_pc();
                        } else {
                            self.remove_wizard_at(j);
                        }
                    }
                    // impact burst
                    for _ in 0..14 {
                        let a = rand_unit() * std::f32::consts::TAU;
                        let r2 = 3.5 + rand_unit() * 1.0;
                        self.particles.push(Particle {
                            pos: p1,
                            vel: glam::vec3(a.cos() * r2, 2.0 + rand_unit() * 1.0, a.sin() * r2),
                            age: 0.0,
                            life: 0.16,
                            size: 0.02,
                            color: [1.8, 0.8, 0.3],
                        });
                    }
                    self.projectiles.swap_remove(i);
                    hit_someone = true;
                    break;
                }
            }
            if !hit_someone {
                i += 1;
            }
        }
    }

    pub(crate) fn spawn_firebolt(
        &mut self,
        origin: glam::Vec3,
        dir: glam::Vec3,
        t: f32,
        owner: Option<usize>,
        snap_to_ground: bool,
        color: [f32; 3],
    ) {
        let mut speed = 40.0;
        // Base lifetime for visuals; will be clamped by spec range below.
        let base_life = 1.2 * 1.5;
        // Compute range clamp from spell spec (default 120 ft)
        let mut max_range_m = 120.0 * 0.3048;
        if let Some(spec) = &self.fire_bolt
            && let Some(p) = &spec.projectile
        {
            speed = p.speed_mps;
            max_range_m = (spec.range_ft as f32) * 0.3048;
        }
        let flight_time = if speed > 0.01 {
            max_range_m / speed
        } else {
            base_life
        };
        let life = base_life.min(flight_time);
        // Ensure initial spawn is terrain-aware.
        let origin = if snap_to_ground {
            let (h, _n) = terrain::height_at(&self.terrain_cpu, origin.x, origin.z);
            glam::vec3(origin.x, h + 0.15, origin.z)
        } else {
            gfx::util::clamp_above_terrain(&self.terrain_cpu, origin, 0.15)
        };
        self.projectiles.push(gfx::fx::Projectile {
            pos: origin,
            vel: dir * speed,
            t_die: t + life,
            owner_wizard: owner,
            color,
        });
    }

    /// Spawn Magic Missile visuals: three darts on a horizontal plane.
    /// The center dart flies straight forward; the side darts fly with a slight
    /// outward yaw so they gradually spread as they travel.
    pub(crate) fn spawn_magic_missile(&mut self, origin: glam::Vec3, dir: glam::Vec3, t: f32) {
        let base_dir = dir.normalize_or_zero();
        // Ultra-tight spread: ±2 degrees about Y (horizontal plane)
        let spread_rad = 2.0_f32.to_radians();
        let left_dir = glam::Quat::from_rotation_y(-spread_rad) * base_dir;
        let right_dir = glam::Quat::from_rotation_y(spread_rad) * base_dir;

        let mm_col = [1.3, 0.7, 2.3];
        // Spawn all three at the same origin so they separate over distance
        self.spawn_firebolt(origin, base_dir, t, Some(self.pc_index), false, mm_col);
        self.spawn_firebolt(origin, left_dir, t, Some(self.pc_index), false, mm_col);
        self.spawn_firebolt(origin, right_dir, t, Some(self.pc_index), false, mm_col);
    }

    pub(crate) fn right_hand_world(&self, clip: &AnimClip, phase: f32) -> Option<glam::Vec3> {
        let h = self.hand_right_node?;
        let m = anim::global_of_node(&self.skinned_cpu, clip, phase, h)?;
        let c = m.to_cols_array();
        Some(glam::vec3(c[12], c[13], c[14]))
    }

    #[allow(dead_code)]
    pub(crate) fn root_flat_forward(&self, clip: &AnimClip, phase: f32) -> Option<glam::Vec3> {
        let r = self.root_node?;
        let m = anim::global_of_node(&self.skinned_cpu, clip, phase, r)?;
        let z = (m * glam::Vec4::new(0.0, 0.0, 1.0, 0.0)).truncate();
        let mut f = z;
        f.y = 0.0;
        if f.length_squared() > 1e-6 {
            Some(f.normalize())
        } else {
            None
        }
    }
}

// Small helpers used by input/update
pub(super) fn wrap_angle(a: f32) -> f32 {
    let mut x = a;
    while x > std::f32::consts::PI {
        x -= std::f32::consts::TAU;
    }
    while x < -std::f32::consts::PI {
        x += std::f32::consts::TAU;
    }
    x
}

pub(super) fn rand_unit() -> f32 {
    use rand::Rng as _;
    let mut r = rand::rng();
    r.random::<f32>() * 2.0 - 1.0
}

pub(super) fn segment_hits_circle_xz(
    p0: glam::Vec3,
    p1: glam::Vec3,
    c: glam::Vec3,
    r: f32,
) -> bool {
    let p0 = glam::vec2(p0.x, p0.z);
    let p1 = glam::vec2(p1.x, p1.z);
    let c = glam::vec2(c.x, c.z);
    let d = p1 - p0;
    let m = p0 - c;
    let a = d.dot(d);
    if a <= 1e-6 {
        return m.length() <= r;
    }
    let t = (-(m.dot(d)) / a).clamp(0.0, 1.0);
    let closest = p0 + d * t;
    (closest - c).length() <= r
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn segment_circle_intersects_center_cross() {
        let c = glam::vec3(0.0, 0.0, 0.0);
        let p0 = glam::vec3(-2.0, 0.5, 0.0);
        let p1 = glam::vec3(2.0, 0.5, 0.0);
        assert!(segment_hits_circle_xz(p0, p1, c, 0.5));
    }
}
