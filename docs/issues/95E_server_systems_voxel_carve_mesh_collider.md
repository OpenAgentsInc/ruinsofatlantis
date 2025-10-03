# 95E — Server Systems: VoxelCarve, GreedyMesh, ColliderRebuild (Authoritative)

Labels: server-authoritative, ecs, jobs, voxel
Depends on: Epic #95, 95C (ECS Components), 95D (Data Config)

Intent
- Move destructible carve/mesh/collider from the renderer to server_core ECS systems with tick budgets and optional jobs.

Outcomes
- Server consumes `CarveRequest` components, mutates `VoxelProxy`, marks `ChunkDirty`, produces `ChunkMesh` entries, and refreshes per-chunk colliders — all within configured budgets.
- Renderer no longer performs carve/collider/mesh mutations (gated by 95A); it will later upload meshes via replication.

Repo‑aware Inventory
- Use existing helpers: `crates/server_core/src/destructible.rs::{raycast_voxels,carve_and_spawn_debris}`, `queue::ChunkQueue`, `config::DestructibleConfig`.
- Voxel core: `crates/voxel_proxy` (grid + carve + flood-fill), `crates/voxel_mesh` (per-chunk greedy mesher).
- Renderer colliders: builder logic under `render_wgpu::gfx::chunkcol`; mirror it under `server_core::collision_static::chunks`.
 - Current client mutation sites to replace (for reference):
   - `crates/render_wgpu/src/gfx/renderer/update.rs`:
     - Selector: `find_destructible_hit(p0,p1)`
     - Carve: `explode_fireball_against_destructible(owner,p0,p1,did,t_hit,radius,damage)`
     - Meshing: `process_one_ruin_vox(..)`, `process_all_ruin_queues(..)`
     - Collider builds: calls to `chunkcol::build_chunk_collider`, `swap_in_updates`, `rebuild_static_index`
     - Debris: `update_debris` (visual; keep client‑side only)

Files
- `crates/server_core/src/systems/mod.rs` (new)
- `crates/server_core/src/systems/destructible.rs` (new)
- `crates/server_core/src/collision_static/chunks.rs` (new mirror)
- `crates/server_core/src/tick.rs` (new or extend) — fixed‑dt scheduler and system ordering

Systems (initial, sequential; jobs optional)
- `VoxelCarveSystem` (authoritative):
  - Inputs: `CarveRequest { did, center_m, radius_m, seed, impact_id }`
  - Steps: locate target `VoxelProxy` by `did`; call `carve_and_spawn_debris`; collect removed centers (debris buffer optional in v0); push affected chunk coords into `ChunkDirty`.
  - Tuning: limit carve volume using `max_carve_chunks` from config.
- `GreedyMeshSystem` (budgeted):
  - Pop up to `max_remesh_per_tick` from `ChunkDirty`; run `voxel_mesh::greedy_mesh_chunk`; write to `ChunkMesh.map` as a `MeshCpu` entry.
  - Optional: skip upload if occupancy hash unchanged (use `VoxelGrid::chunk_occ_hash`).
- `ColliderRebuildSystem` (budgeted):
  - For each processed chunk, build a coarse collider (AABB per chunk coord) using mirrored `chunkcol::build_chunk_collider` and swap into a server spatial index.
  - Budget: `collider_budget_per_tick`.

Data & Config Wiring
- Read `DestructibleConfig` defaults from `data_runtime` (95D) and permit CLI overrides; log effective values once per run.
- Keys: `voxel_size_m`, `chunk`, `aabb_pad_m`, `max_remesh_per_tick`, `collider_budget_per_tick`, `max_debris`, `max_carve_chunks`, `close_surfaces`, `seed`.

API/Components
- Reuse components from 95C: `Destructible`, `VoxelProxy`, `ChunkDirty`, `ChunkMesh`, `CarveRequest`.
- Provide lookup helpers: `fn proxy_mut(world, did: DestructibleId) -> Option<&mut VoxelProxy>`.

Logging
- Add `#[cfg(feature = "destruct_debug")]` logs per system: counts of dirty processed, remesh and collider timings.

Tests
- Unit (server‑only):
  - Build `VoxelProxy` with dims ~ `UVec3::new(32,32,32)`; enqueue a `CarveRequest` at center with fixed seed/radius; run systems once; assert `ChunkMesh.map.len() > 0` and quads > 0.
  - Budget adherence: with many dirty chunks, ensure only `max_remesh_per_tick` entries are processed per tick.
- Optional: `ColliderRebuildSystem` yields count equal to processed chunks.

Acceptance
- With a seeded grid + `CarveRequest`, server tick produces `ChunkMesh` entries and collider updates within budget.
- No renderer carve/collider/mesh mutation needed for visual updates once replication lands.
- Verified by temporarily stubbing a local apply of `ChunkMesh` to renderer (or by printing mesh counts in logs under `destruct_debug`).

---

## Addendum — Implementation Summary (95E partial)

- server_core::systems
  - Added `systems/mod.rs` and `systems/destructible.rs` with:
    - `voxel_carve(grid, req, cfg, dirty) -> touched_chunks`
    - `greedy_mesh_budget(grid, dirty, out_mesh, budget) -> processed_count`
  - Unit test constructs a small grid, issues a `CarveRequest`, and verifies that `greedy_mesh_budget` processes up to the budget and produces meshes.
- Collision rebuild is deferred; existing `collision_static` crate can be integrated in a subsequent step.
- CI remains green.
Status: PARTIAL (VoxelCarve + GreedyMesh landed with tests; ColliderRebuild TBD)
