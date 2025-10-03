//! Authoritative destructible systems: VoxelCarve and GreedyMesh (budgeted).

use crate::destructible::{carve_and_spawn_debris, queue::ChunkQueue};
use crate::destructible::config::DestructibleConfig;
use ecs_core::components::{CarveRequest, ChunkDirty, ChunkMesh, MeshCpu};
use glam::UVec3;
use voxel_proxy::VoxelGrid;

/// Apply a carve request to the grid and enqueue dirty chunks.
pub fn voxel_carve(
    grid: &mut VoxelGrid,
    req: &CarveRequest,
    cfg: &DestructibleConfig,
    dirty: &mut ChunkDirty,
) -> usize {
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
        let mut mc = MeshCpu::default();
        mc.positions = mb.positions.clone();
        mc.normals = mb.normals.clone();
        mc.indices = mb.indices.clone();
        if mc.validate().is_ok() {
            out_mesh.map.insert((c.x, c.y, c.z), mc);
            processed += 1;
        }
    }
    processed
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
        let req = CarveRequest { did: 1, center_m: glam::DVec3::new(2.5, 2.5, 2.5), radius_m: 0.6, seed: 42, impact_id: 1 };
        let touched = voxel_carve(&mut grid, &req, &cfg, &mut dirty);
        assert!(touched > 0);
        let budget = 3usize;
        let processed = greedy_mesh_budget(&grid, &mut dirty, &mut meshes, budget);
        assert!(processed <= budget);
        assert!(meshes.map.len() > 0);
    }
}

