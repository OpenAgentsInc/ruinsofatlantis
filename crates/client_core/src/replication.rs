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

#[derive(Default, Debug)]
pub struct ReplicationBuffer {
    pub updated_chunks: usize,
    pending_mesh: Vec<(u64, (u32, u32, u32), crate::upload::ChunkMeshEntry)>,
    pub boss_status: Option<BossStatus>,
    pub npcs: Vec<NpcView>,
    pub projectiles: Vec<ProjectileView>,
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
    pub fn apply_message(&mut self, bytes: &[u8]) -> bool {
        // If the message is framed, unwrap the payload; else fall back to raw
        let payload: &[u8] = match net_core::frame::read_msg(bytes) {
            Ok(p) => p,
            Err(_) => bytes,
        };
        // 0) Prefer new consolidated TickSnapshot, which includes boss + npcs.
        let mut slice_ts: &[u8] = payload;
        if let Ok(ts) = net_core::snapshot::TickSnapshot::decode(&mut slice_ts) {
            self.npcs.clear();
            for n in ts.npcs {
                self.npcs.push(NpcView {
                    id: n.id,
                    hp: n.hp,
                    max: n.max,
                    pos: glam::vec3(n.pos[0], n.pos[1], n.pos[2]),
                    radius: n.radius,
                    alive: n.alive,
                    attack_anim: 0.0,
                    yaw: n.yaw,
                });
            }
            self.projectiles.clear();
            for p in ts.projectiles {
                self.projectiles.push(ProjectileView {
                    id: p.id,
                    kind: p.kind,
                    pos: glam::vec3(p.pos[0], p.pos[1], p.pos[2]),
                    vel: glam::vec3(p.vel[0], p.vel[1], p.vel[2]),
                });
            }
            self.boss_status = ts.boss.map(|b| BossStatus {
                name: b.name,
                ac: b.ac,
                hp: b.hp,
                max: b.max,
                pos: glam::vec3(b.pos[0], b.pos[1], b.pos[2]),
            });
            return true;
        }
        let mut slice_delta: &[u8] = payload;
        if let Ok(delta) = net_core::snapshot::ChunkMeshDelta::decode(&mut slice_delta) {
            let entry = crate::upload::ChunkMeshEntry {
                positions: delta.positions,
                normals: delta.normals,
                indices: delta.indices,
            };
            self.pending_mesh.push((delta.did, delta.chunk, entry));
            self.updated_chunks += 1;
            true
        } else {
            // Prefer NpcListMsg before BossStatusMsg to avoid false-positive
            // decodes on the list payload (both are versioned).
            let mut slice_list: &[u8] = payload;
            if let Ok(list) = net_core::snapshot::NpcListMsg::decode(&mut slice_list) {
                self.npcs.clear();
                for it in list.items {
                    self.npcs.push(NpcView {
                        id: it.id,
                        hp: it.hp,
                        max: it.max,
                        pos: glam::vec3(it.pos[0], it.pos[1], it.pos[2]),
                        radius: it.radius,
                        alive: it.alive != 0,
                        attack_anim: it.attack_anim,
                        yaw: 0.0,
                    });
                }
                true
            } else {
                let mut bs_slice: &[u8] = payload; // reset for boss status
                if let Ok(bs) = net_core::snapshot::BossStatusMsg::decode(&mut bs_slice) {
                    self.boss_status = Some(BossStatus {
                        name: bs.name,
                        ac: bs.ac,
                        hp: bs.hp,
                        max: bs.max,
                        pos: glam::vec3(bs.pos[0], bs.pos[1], bs.pos[2]),
                    });
                    true
                } else {
                    false
                }
            }
        }
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
        use net_core::snapshot::{TickSnapshot, WizardRep, NpcRep, ProjectileRep, BossRep, SnapshotEncode, SnapshotDecode, TAG_TICK_SNAPSHOT};
        let ts = TickSnapshot {
            v: 1,
            tick: 7,
            wizards: vec![WizardRep { id: 1, kind: 0, pos: [0.0, 0.6, 0.0], yaw: 0.0, hp: 100, max: 100 }],
            npcs: vec![NpcRep { id: 9, archetype: 0, pos: [1.0, 0.6, 2.0], yaw: 0.0, radius: 0.9, hp: 20, max: 20, alive: true }],
            projectiles: vec![ProjectileRep { id: 3, kind: 0, pos: [0.0, 0.7, 0.2], vel: [0.0, 0.0, 40.0] }],
            boss: Some(BossRep { id: 99, name: "Nivita".into(), pos: [5.0, 0.6, 5.0], hp: 225, max: 225, ac: 18 })
        };
        let mut buf = Vec::new();
        ts.encode(&mut buf);
        // Frame it
        let mut framed = Vec::new();
        net_core::frame::write_msg(&mut framed, &buf);
        let mut repl = ReplicationBuffer::default();
        assert!(repl.apply_message(&framed));
        assert_eq!(repl.npcs.len(), 1);
        assert_eq!(repl.projectiles.len(), 1);
        assert!(repl.boss_status.is_some());
    }
}
