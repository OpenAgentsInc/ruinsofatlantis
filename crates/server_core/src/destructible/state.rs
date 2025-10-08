#![cfg(any())]
//! Destructible registry: multi-proxy voxel state owned by the server ECS.
//!
//! Scope
//! - Holds per-proxy voxel grids, dirty chunk sets, mesh caches, and colliders.
//! - Collects `CarveRequest` from gameplay systems and applies them during tick.
//! - Produces compact `ChunkMeshDelta` records for replication.
//!
//! Extending
//! - Add per-proxy transforms for object<->world when carving in object space.
//! - Add interest filtering and per-client delta buffers if/when networking expands.

use ecs_core::components::{CarveRequest, ChunkDirty, ChunkMesh};
use glam::{Mat4, UVec3, Vec3};
use net_core::snapshot::ChunkMeshDelta;
use std::collections::HashMap;
use voxel_proxy::VoxelGrid;

use super::config::DestructibleConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DestructibleId(pub u64);

/// Simple axis-aligned world AABB for broad-phase tests.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldAabb {
    pub min: Vec3,
    pub max: Vec3,
}

/// Per-instance proxy state. Heavy voxel data lives here; ECS entities hold only lightweight refs.
pub struct DestructibleProxy {
    pub did: DestructibleId,
    pub grid: VoxelGrid,
    pub dirty: ChunkDirty,
    pub meshes: ChunkMesh,
    pub colliders: Vec<collision_static::chunks::StaticChunk>,
    pub static_index: Option<collision_static::StaticIndex>,
    pub world_from_object: Mat4,
    pub object_from_world: Mat4,
    pub world_aabb: WorldAabb,
}

impl DestructibleProxy {
    pub fn new(did: DestructibleId, grid: VoxelGrid, world_aabb: WorldAabb) -> Self {
        let world_from_object = Mat4::IDENTITY;
        let object_from_world = Mat4::IDENTITY;
        Self {
            did,
            grid,
            dirty: ChunkDirty::default(),
            meshes: ChunkMesh::default(),
            colliders: Vec::new(),
            static_index: None,
            world_from_object,
            object_from_world,
            world_aabb,
        }
    }
}

/// Server-owned registry for destructible proxies and per-tick carve events.
pub struct DestructibleRegistry {
    pub proxies: HashMap<DestructibleId, DestructibleProxy>,
    pub pending_carves: Vec<CarveRequest>,
    // Accumulated mesh deltas for replication this tick (drained by platform).
    pub pending_mesh_deltas: Vec<ChunkMeshDelta>,
    pub cfg: DestructibleConfig,
}

impl Default for DestructibleRegistry {
    fn default() -> Self {
        Self {
            proxies: HashMap::new(),
            pending_carves: Vec::new(),
            pending_mesh_deltas: Vec::new(),
            cfg: DestructibleConfig::default(),
        }
    }
}

impl std::fmt::Debug for DestructibleRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DestructibleRegistry")
            .field("proxies", &self.proxies.len())
            .field("pending_carves", &self.pending_carves.len())
            .field("pending_mesh_deltas", &self.pending_mesh_deltas.len())
            .finish()
    }
}

impl DestructibleRegistry {
    /// Register a proxy into the registry.
    pub fn insert_proxy(&mut self, proxy: DestructibleProxy) {
        self.proxies.insert(proxy.did, proxy);
    }

    /// Broad-phase helper: does a segment intersect the proxy's world AABB?
    #[inline]
    pub fn seg_intersects_proxy(&self, did: DestructibleId, p0: Vec3, p1: Vec3) -> bool {
        if let Some(p) = self.proxies.get(&did) {
            segment_aabb_enter_t(p0, p1, p.world_aabb.min, p.world_aabb.max).is_some()
        } else {
            false
        }
    }

    /// Apply queued carves, mesh/collider budgets, and collect mesh deltas.
    pub fn tick(&mut self) {
        use crate::systems::destructible::{collider_rebuild_budget, greedy_mesh_budget};
        // 1) Drain carves into per-proxy queues and mark dirty
        if !self.pending_carves.is_empty() {
            let mut reqs = Vec::new();
            std::mem::swap(&mut reqs, &mut self.pending_carves);
            for req in reqs {
                let did = DestructibleId(req.did);
                if let Some(proxy) = self.proxies.get_mut(&did) {
                    // Apply carve in world space (proxy grid origin assumed in world for v0)
                    let _ = crate::systems::destructible::voxel_carve(
                        &mut proxy.grid,
                        &req,
                        &self.cfg,
                        &mut proxy.dirty,
                    );
                }
            }
        }
        // 2) Mesh dirty chunks per proxy within budget; collect changed chunk keys
        let mut updated: Vec<(DestructibleId, UVec3)> = Vec::new();
        for (did, proxy) in self.proxies.iter_mut() {
            if proxy.dirty.0.is_empty() {
                continue;
            }
            // respect global per-tick budget across all proxies by splitting fairly
            let budget = self.cfg.max_chunk_remesh.max(0);
            if budget == 0 {
                continue;
            }
            // Snapshot dirty set; greedy_mesh_budget pops a subset while preserving order
            let before_len = proxy.dirty.0.len();
            let meshed =
                greedy_mesh_budget(&proxy.grid, &mut proxy.dirty, &mut proxy.meshes, budget);
            let after_len = proxy.dirty.0.len();
            let processed = before_len.saturating_sub(after_len).min(meshed);
            // Conservatively scan mesh map for stable ordering; push up to `processed` keys
            // NOTE: This is a coarse approximation; a dedicated list would avoid map scan.
            if processed > 0 {
                let mut keys: Vec<_> = proxy
                    .meshes
                    .map
                    .keys()
                    .copied()
                    .map(|(x, y, z)| UVec3::new(x, y, z))
                    .collect();
                keys.sort_unstable_by_key(|c| (c.x, c.y, c.z));
                for c in keys.into_iter().take(processed) {
                    updated.push((*did, c));
                }
            }
        }
        // 3) Collider rebuild budget per-tick
        for (_did, proxy) in self.proxies.iter_mut() {
            if proxy.meshes.map.is_empty() {
                continue;
            }
            // Deterministic order: sorted chunk keys
            let mut keys: Vec<UVec3> = proxy
                .meshes
                .map
                .keys()
                .copied()
                .map(|(x, y, z)| UVec3::new(x, y, z))
                .collect();
            keys.sort_unstable_by_key(|c| (c.x, c.y, c.z));
            let _ = collider_rebuild_budget(
                &proxy.grid,
                &keys,
                &mut proxy.colliders,
                &mut proxy.static_index,
                self.cfg.collider_budget_per_tick.max(0),
            );
        }
        // 4) Build deltas for updated chunks
        if !updated.is_empty() {
            for (did, c) in updated {
                if let Some(proxy) = self.proxies.get(&did) {
                    let key = (c.x, c.y, c.z);
                    if let Some(mc) = proxy.meshes.map.get(&key) {
                        let delta = ChunkMeshDelta {
                            did: did.0,
                            chunk: key,
                            positions: mc.positions.clone(),
                            normals: mc.normals.clone(),
                            indices: mc.indices.clone(),
                        };
                        self.pending_mesh_deltas.push(delta);
                    } else {
                        // Mesh removed — send empty delta to trigger removal on clients
                        self.pending_mesh_deltas.push(ChunkMeshDelta {
                            did: did.0,
                            chunk: key,
                            positions: Vec::new(),
                            normals: Vec::new(),
                            indices: Vec::new(),
                        });
                    }
                }
            }
        }
    }
}

// Local copy of segment vs AABB enter test (world space), identical to systems::projectiles helper.
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
            if s < minb || s > maxb {
                return None;
            }
        } else {
            let inv = 1.0 / dir;
            let mut t0 = (minb - s) * inv;
            let mut t1 = (maxb - s) * inv;
            if t0 > t1 {
                core::mem::swap(&mut t0, &mut t1);
            }
            tmin = tmin.max(t0);
            tmax = tmax.min(t1);
            if tmin > tmax {
                return None;
            }
        }
    }
    Some(tmin)
}
