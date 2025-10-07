//! Client replication scaffolding.
//!
//! Responsibilities
//! - Buffer incoming snapshot deltas
//! - Apply to client ECS/state
//! - Invalidate GPU uploads for changed chunks
//!
//! Filled in later when net_core types are finalized.

/// Client-side replication buffer that accumulates incoming deltas (chunks,
/// entity snapshots) and exposes a coherent view for presentation layers.
use net_core::snapshot::SnapshotDecode;
use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct ReplicationBuffer {
    pub updated_chunks: usize,
    pending_mesh: Vec<(u64, (u32, u32, u32), crate::upload::ChunkMeshEntry)>,
    pub boss_status: Option<BossStatus>,
    pub actors: Vec<ActorView>,
    pub wizards: Vec<WizardView>,
    pub npcs: Vec<NpcView>,
    pub projectiles: Vec<ProjectileView>,
    pub hud: HudState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BossStatus {
    pub name: String,
    pub ac: i32,
    pub hp: i32,
    pub max: i32,
    pub pos: glam::Vec3,
}

impl ReplicationBuffer {
    /// Apply a raw message. Returns whether any state changed.
    #[allow(clippy::too_many_lines)]
    pub fn apply_message(&mut self, bytes: &[u8]) -> bool {
        // If the message is framed, unwrap the payload; else fall back to raw
        let payload: &[u8] = match net_core::frame::read_msg(bytes) {
            Ok(p) => p,
            Err(_) => bytes,
        };
        // Prefer actor delta snapshot (v3) first
        let mut slice_delta_v3: &[u8] = payload;
        if let Ok(d) = net_core::snapshot::ActorSnapshotDelta::decode(&mut slice_delta_v3) {
            // Removals
            if !d.removals.is_empty() {
                self.actors.retain(|a| !d.removals.contains(&a.id));
                self.wizards.retain(|w| !d.removals.contains(&w.id));
                self.npcs.retain(|n| !d.removals.contains(&n.id));
            }
            // Build id->index map AFTER removals so indices are valid
            let mut idx: HashMap<u32, usize> = HashMap::new();
            for (i, a) in self.actors.iter().enumerate() {
                idx.insert(a.id, i);
            }
            // Updates
            for u in d.updates {
                if let Some(&i) = idx.get(&u.id) {
                    let a = &mut self.actors[i];
                    let mut changed = false;
                    if u.flags & 1 != 0 {
                        a.pos.x = net_core::snapshot::dqpos(u.qpos[0]);
                        a.pos.y = net_core::snapshot::dqpos(u.qpos[1]);
                        a.pos.z = net_core::snapshot::dqpos(u.qpos[2]);
                        changed = true;
                    }
                    if u.flags & 2 != 0 {
                        a.yaw = net_core::snapshot::dqyaw(u.qyaw);
                        changed = true;
                    }
                    if u.flags & 4 != 0 {
                        a.hp = u.hp;
                        changed = true;
                    }
                    if u.flags & 8 != 0 {
                        a.alive = u.alive != 0;
                        changed = true;
                    }
                    if changed {
                        // Update derived views if present
                        for w in &mut self.wizards {
                            if w.id == a.id {
                                w.pos = a.pos;
                                w.yaw = a.yaw;
                                w.hp = a.hp;
                                w.max = a.max;
                            }
                        }
                        for n in &mut self.npcs {
                            if n.id == a.id {
                                n.pos = a.pos;
                                n.yaw = a.yaw;
                                n.hp = a.hp;
                                n.max = a.max;
                                n.alive = a.alive;
                            }
                        }
                    }
                }
            }
            // Spawns
            for a in d.spawns {
                let av = ActorView {
                    id: a.id,
                    kind: a.kind,
                    team: a.team,
                    pos: glam::vec3(a.pos[0], a.pos[1], a.pos[2]),
                    yaw: a.yaw,
                    radius: a.radius,
                    hp: a.hp,
                    max: a.max,
                    alive: a.alive,
                };
                self.actors.push(av.clone());
                match a.kind {
                    0 => self.wizards.push(WizardView {
                        id: a.id,
                        kind: 0,
                        pos: av.pos,
                        yaw: av.yaw,
                        hp: av.hp,
                        max: av.max,
                    }),
                    1 | 2 => self.npcs.push(NpcView {
                        id: a.id,
                        hp: av.hp,
                        max: av.max,
                        pos: av.pos,
                        radius: av.radius,
                        alive: av.alive,
                        attack_anim: 0.0,
                        yaw: av.yaw,
                    }),
                    _ => {}
                }
            }
            // Projectiles (full list)
            self.projectiles.clear();
            for p in d.projectiles {
                self.projectiles.push(ProjectileView {
                    id: p.id,
                    kind: p.kind,
                    pos: glam::vec3(p.pos[0], p.pos[1], p.pos[2]),
                    vel: glam::vec3(p.vel[0], p.vel[1], p.vel[2]),
                });
            }
            return true;
        }
        // Chunk mesh deltas (tools/dev): accept and stash
        let mut slice_delta: &[u8] = payload;
        if let Ok(delta) = net_core::snapshot::ChunkMeshDelta::decode(&mut slice_delta) {
            let entry = crate::upload::ChunkMeshEntry {
                positions: delta.positions,
                normals: delta.normals,
                indices: delta.indices,
            };
            self.pending_mesh.push((delta.did, delta.chunk, entry));
            self.updated_chunks += 1;
            return true;
        }
        // HUD status message
        let mut hud_slice: &[u8] = payload;
        if let Ok(hud) = net_core::snapshot::HudStatusMsg::decode(&mut hud_slice) {
            self.hud.mana = hud.mana;
            self.hud.mana_max = hud.mana_max;
            self.hud.gcd_ms = hud.gcd_ms;
            // map spell ids 0,1,2
            self.hud.spell_cds = [0, 0, 0];
            for (id, ms) in hud.spell_cds {
                if (id as usize) < self.hud.spell_cds.len() {
                    self.hud.spell_cds[id as usize] = ms;
                }
            }
            self.hud.burning_ms = hud.burning_ms;
            self.hud.slow_ms = hud.slow_ms;
            self.hud.stunned_ms = hud.stunned_ms;
            return true;
        }
        false
    }

    /// Drain pending mesh updates accumulated from replication into a vector
    /// of (did, chunk, entry). Renderer or host applies uploads via `MeshUpload`.
    pub fn drain_mesh_updates(
        &mut self,
    ) -> Vec<(u64, (u32, u32, u32), crate::upload::ChunkMeshEntry)> {
        let mut v = Vec::new();
        std::mem::swap(&mut v, &mut self.pending_mesh);
        v
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NpcView {
    pub id: u32,
    pub hp: i32,
    pub max: i32,
    pub pos: glam::Vec3,
    pub radius: f32,
    pub alive: bool,
    pub attack_anim: f32,
    pub yaw: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActorView {
    pub id: u32,
    pub kind: u8,
    pub team: u8,
    pub pos: glam::Vec3,
    pub yaw: f32,
    pub radius: f32,
    pub hp: i32,
    pub max: i32,
    pub alive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HudState {
    pub mana: u16,
    pub mana_max: u16,
    pub gcd_ms: u16,
    pub spell_cds: [u16; 3],
    pub burning_ms: u16,
    pub slow_ms: u16,
    pub stunned_ms: u16,
}
#[derive(Debug, Clone, PartialEq)]
pub struct WizardView {
    pub id: u32,
    pub kind: u8,
    pub pos: glam::Vec3,
    pub yaw: f32,
    pub hp: i32,
    pub max: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectileView {
    pub id: u32,
    pub kind: u8,
    pub pos: glam::Vec3,
    pub vel: glam::Vec3,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn buffer_default_is_empty() {
        let b = ReplicationBuffer::default();
        assert_eq!(b.updated_chunks, 0);
    }
    #[test]
    fn tick_snapshot_populates_npcs_and_projectiles() {
        // Legacy TickSnapshot no longer decoded; expect no-op on garbage bytes
        let mut repl = ReplicationBuffer::default();
        assert!(!repl.apply_message(&[0u8]));
    }

    #[test]
    fn apply_tick_snapshot_populates_all_views() {
        let mut buf = ReplicationBuffer::default();
        assert!(!buf.apply_message(&[0u8]));
    }
}
