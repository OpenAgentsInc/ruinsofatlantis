//! Client replication scaffolding.
//!
//! Responsibilities
//! - Buffer incoming snapshot deltas
//! - Apply to client ECS/state
//! - Invalidate GPU uploads for changed chunks
//!
//! Filled in later when net_core types are finalized.

/// Opaque replication buffer (placeholder).
use net_core::snapshot::SnapshotDecode;

#[derive(Default, Debug)]
pub struct ReplicationBuffer {
    pub updated_chunks: usize,
    pending_mesh: Vec<(u64, (u32, u32, u32), crate::upload::ChunkMeshEntry)>,
    pub boss_status: Option<BossStatus>,
    pub npcs: Vec<NpcView>,
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
        let mut slice: &[u8] = payload;
        if let Ok(delta) = net_core::snapshot::ChunkMeshDelta::decode(&mut slice) {
            let entry = crate::upload::ChunkMeshEntry {
                positions: delta.positions,
                normals: delta.normals,
                indices: delta.indices,
            };
            self.pending_mesh.push((delta.did, delta.chunk, entry));
            self.updated_chunks += 1;
            true
        } else {
            let mut slice2: &[u8] = payload; // reset since first decode may have advanced
            if let Ok(bs) = net_core::snapshot::BossStatusMsg::decode(&mut slice2) {
                self.boss_status = Some(BossStatus {
                    name: bs.name,
                    ac: bs.ac,
                    hp: bs.hp,
                    max: bs.max,
                    pos: glam::vec3(bs.pos[0], bs.pos[1], bs.pos[2]),
                });
                return true;
            }
            let mut slice3: &[u8] = payload;
            if let Ok(list) = net_core::snapshot::NpcListMsg::decode(&mut slice3) {
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
                    });
                }
                true
            } else {
                false
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
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn buffer_default_is_empty() {
        let b = ReplicationBuffer::default();
        assert_eq!(b.updated_chunks, 0);
    }
}
