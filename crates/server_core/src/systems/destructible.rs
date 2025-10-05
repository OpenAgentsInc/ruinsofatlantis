//! Authoritative destructible systems: VoxelCarve and GreedyMesh (budgeted).

use crate::destructible::config::DestructibleConfig;
use crate::destructible::{carve_and_spawn_debris, queue::ChunkQueue};
use ecs_core::components::{CarveRequest, ChunkDirty, ChunkMesh, MeshCpu};
use glam::UVec3;
use std::time::Instant;
use voxel_proxy::VoxelGrid;

/// Apply a carve request to the grid and enqueue dirty chunks.
pub fn voxel_carve(
    grid: &mut VoxelGrid,
    req: &CarveRequest,
    cfg: &DestructibleConfig,
    dirty: &mut ChunkDirty,
) -> usize {
    let t0 = Instant::now();
    let _ = carve_and_spawn_debris(
        grid,
        req.center_m,
        core_units::Length::meters(req.radius_m),
        cfg.seed ^ (req.seed),
        req.impact_id as u64,
        cfg.max_debris,
    );
    let enq = grid.pop_dirty_chunks(usize::MAX);
    let n = enq.len();
    dirty.0.extend(enq);
    let _ = t0; // reserved for telemetry
    n
}

/// Greedy-mesh up to `budget` chunks from `dirty` into `out_mesh`.
pub fn greedy_mesh_budget(
    grid: &VoxelGrid,
    dirty: &mut ChunkDirty,
    out_mesh: &mut ChunkMesh,
    budget: usize,
) -> usize {
    if budget == 0 || dirty.0.is_empty() {
        return 0;
    }
    let t0 = Instant::now();
    // Use a deterministic queue to pop a fixed budget
    let mut q = ChunkQueue::new();
    q.enqueue_many(std::mem::take(&mut dirty.0));
    let chunks = q.pop_budget(budget);
    // put the remainder back
    dirty.0.extend(q.pop_budget(usize::MAX));
    let mut processed = 0usize;
    for c in chunks {
        let mb = voxel_mesh::greedy_mesh_chunk(grid, c);
        if mb.indices.is_empty() {
            out_mesh.map.remove(&(c.x, c.y, c.z));
            continue;
        }
        let mc = MeshCpu {
            positions: mb.positions.clone(),
            normals: mb.normals.clone(),
            indices: mb.indices.clone(),
        };
        if mc.validate().is_ok() {
            out_mesh.map.insert((c.x, c.y, c.z), mc);
            processed += 1;
        }
    }
    let _ = t0; // reserved for telemetry
    processed
}

/// Rebuild up to `budget` coarse colliders for the provided chunk list and refresh the static index.
pub fn collider_rebuild_budget(
    grid: &VoxelGrid,
    chunks: &[UVec3],
    store: &mut Vec<collision_static::chunks::StaticChunk>,
    static_index: &mut Option<collision_static::StaticIndex>,
    budget: usize,
) -> usize {
    use collision_static::chunks::{build_chunk_collider, rebuild_static_index, swap_in_updates};
    if budget == 0 || chunks.is_empty() {
        return 0;
    }
    let t0 = Instant::now();
    let take = budget.min(chunks.len());
    let mut updates = Vec::new();
    for c in chunks.iter().copied().take(take) {
        if let Some(sc) = build_chunk_collider(grid, c) {
            updates.push(sc);
        }
    }
    if !updates.is_empty() {
        swap_in_updates(store, updates);
        *static_index = Some(rebuild_static_index(store));
    }
    let _ = t0; // reserved for telemetry
    take
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_units::Length;
    use voxel_proxy::{GlobalId, VoxelProxyMeta};

    fn mk_grid(d: UVec3, c: UVec3, vox_m: f64) -> VoxelGrid {
        let meta = VoxelProxyMeta {
            object_id: GlobalId(1),
            origin_m: glam::DVec3::ZERO,
            voxel_m: Length::meters(vox_m),
            dims: d,
            chunk: c,
            material: core_materials::find_material_id("stone").unwrap(),
        };
        VoxelGrid::new(meta)
    }

    #[test]
    fn carve_then_mesh_processes_within_budget() {
        let mut grid = mk_grid(UVec3::new(32, 32, 32), UVec3::new(8, 8, 8), 0.25);
        let mut dirty = ChunkDirty::default();
        let mut meshes = ChunkMesh::default();
        let cfg = DestructibleConfig::default();
        let req = CarveRequest {
            did: 1,
            center_m: glam::DVec3::new(2.5, 2.5, 2.5),
            radius_m: 0.6,
            seed: 42,
            impact_id: 1,
        };
        let touched = voxel_carve(&mut grid, &req, &cfg, &mut dirty);
        assert!(touched > 0);
        let budget = 3usize;
        let processed = greedy_mesh_budget(&grid, &mut dirty, &mut meshes, budget);
        assert!(processed <= budget);
        assert!(!meshes.map.is_empty());
    }

    #[test]
    fn collider_rebuild_uses_budget() {
        let mut grid = mk_grid(UVec3::new(16, 16, 16), UVec3::new(8, 8, 8), 1.0);
        // mark some voxels solid across two chunks
        grid.set(1, 1, 1, true);
        grid.set(9, 1, 1, true);
        let chunks = vec![UVec3::new(0, 0, 0), UVec3::new(1, 0, 0)];
        let mut store: Vec<collision_static::chunks::StaticChunk> = Vec::new();
        let mut idx = None;
        let done = super::collider_rebuild_budget(&grid, &chunks, &mut store, &mut idx, 1);
        assert_eq!(done, 1);
        assert_eq!(store.len(), 1);
        let done2 = super::collider_rebuild_budget(&grid, &chunks[done..], &mut store, &mut idx, 8);
        assert!(done2 >= 1);
        assert!(
            idx.as_ref()
                .map(|i| !i.colliders.is_empty())
                .unwrap_or(false)
        );
    }
}
