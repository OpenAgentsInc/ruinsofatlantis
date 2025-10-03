//! ECS component definitions for destructibles and voxel chunk meshes.
//!
//! These components are shared across server and client crates. The server owns
//! authoritative mutation for `VoxelProxy` and emits `ChunkDirty`/`ChunkMesh`
//! updates; the client consumes `ChunkMesh` for GPU upload.

use glam::{DVec3, UVec3, Vec3};
use std::collections::HashMap;

/// Opaque entity identifier (server-assigned). Stable across replication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct EntityId(pub u64);

/// Stable identifier for a destructible object (shared across client/server).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct DestructibleId(pub u64);

/// Destructible tag with material for mass/density.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Destructible {
    pub id: u64,
    pub material: core_materials::MaterialId,
}

/// The CPU voxel proxy metadata for a destructible.
#[derive(Debug, Clone)]
pub struct VoxelProxy {
    pub meta: voxel_proxy::VoxelProxyMeta,
}

/// List of dirty chunk coordinates since last mesh/collider rebuild.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct ChunkDirty(pub Vec<UVec3>);

/// A single CPU mesh buffer for a chunk (positions, normals, indices).
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct MeshCpu {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

impl MeshCpu {
    /// Validate CPU mesh invariants.
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.positions.len() == self.normals.len(),
            "pos/normal len mismatch"
        );
        anyhow::ensure!(
            self.indices.len().is_multiple_of(3),
            "indices not multiple of 3"
        );
        Ok(())
    }
}

/// Mapping from chunk coords to mesh for a given destructible proxy.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct ChunkMesh {
    pub map: HashMap<(u32, u32, u32), MeshCpu>,
}

/// Carve request produced by projectile/destructible intersection on the server.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct CarveRequest {
    pub did: u64,
    pub center_m: DVec3,
    pub radius_m: f64,
    pub seed: u64,
    pub impact_id: u32,
}

/// Canonical chunk key type (local chunk coords).
pub type ChunkKey = (u32, u32, u32);

/// Helper to build stable keys for per-destructible chunk maps.
pub fn chunk_key(did: DestructibleId, c: UVec3) -> (DestructibleId, u32, u32, u32) {
    (did, c.x, c.y, c.z)
}

/// Health component for damage/death application.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct Health {
    pub hp: i32,
    pub max: i32,
}

/// Team affiliation (used for friendly fire and aggro).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct Team {
    pub id: u32,
}

/// Collision shape for broad/narrow collision in server systems.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub enum CollisionShape {
    Sphere {
        center: Vec3,
        radius: f32,
    },
    CapsuleY {
        center: Vec3,
        radius: f32,
        half_height: f32,
    },
    Aabb {
        min: Vec3,
        max: Vec3,
    },
}

/// Projectile component for authoritative integration and collision.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct Projectile {
    pub radius_m: f32,
    pub damage: i32,
    pub life_s: f32,
    pub owner: EntityId,
    pub pos: Vec3,
    pub vel: Vec3,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_mesh_insert_and_validate() {
        let mut cm = ChunkMesh::default();
        let key = (0u32, 1, 2);
        let m = MeshCpu {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            indices: vec![0, 1, 2],
        };
        m.validate().expect("valid tri");
        cm.map.insert(key, m.clone());
        assert!(cm.map.get(&key).is_some());
        assert_eq!(cm.map.get(&key).unwrap().indices.len(), 3);
    }
}
