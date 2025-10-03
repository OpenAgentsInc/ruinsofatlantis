# Issue 95 — Server‑Authoritative ECS Refactor (Initial Execution Plan)

Purpose
- Translate Issue #95 (epic) into a concrete, repo‑aware plan with phased deliverables, file paths, and acceptance criteria.
- Output serves as the master sub‑issue breakdown and PR checklists.

Out of Scope (this epic)
- Cross‑zone handoff, net encryption/compression, and MMO scale‑out. We will scaffold a clean, testable path and keep changes incremental.

---

## Current Snapshot (repo‑aware)

- Renderer (`crates/render_wgpu`)
  - `gfx/renderer/update.rs` integrates input, projectiles, collisions, destructible carving, debris, per‑chunk meshing/collider rebuilds, and draws.
  - Destructible spawn: mesh‑based voxelization (`build_*_proxy_from_mesh`), chunk dirty queues, VB/IB uploads; logs added.
- Voxel core
  - `crates/voxel_proxy`: `VoxelGrid`, flood‑fill voxelizer, `carve_sphere`, dirty‑chunk tracking, occupancy hash, `VoxelProxyMeta { GlobalId, voxel_m, dims, chunk, material }`.
  - `crates/voxel_mesh`: per‑chunk greedy mesher with tests (normals/winding, boundary ownership).
- Destructible helpers
  - `crates/server_core/src/destructible.rs`: `raycast_voxels`, `carve_and_spawn_debris`, `queue::ChunkQueue`, and `config::DestructibleConfig` with CLI flags.
- ECS
  - `crates/ecs_core`: minimal ECS (entities, transforms, render kinds) used by scene assembly; no full gameplay scheduling yet.
- Data
  - `crates/data_runtime`: schemas and loaders for content; not yet hosting projectile/spec/budget configs.

Pain points (from code)
- Renderer mutates world state and owns gameplay queues (projectiles/destructibles/meshing/colliders/debris).
- Hard‑coded constants for spells/NPCs/destructible budgets sprinkled in `update.rs` and `server_core::lib.rs`.

---

## Architecture Target (v0)

Crates
- `ecs_core` — entity storage + scheduling traits (expand minimally for systems in this epic).
- `server_core` — authoritative systems (projectiles, damage, destructibles, mesh/collider jobs, replication source).
- `client_core` — client systems (replication apply, prediction hooks, upload meshes). New crate.
- `net_core` — snapshot schema + in‑proc replication plumbing (local loop first). New crate.
- `data_runtime` — SpecDb/config for projectile/destructible/NPC/budgets (expand).
- `render_wgpu` — draw + uploads; no gameplay mutations.

Shared Components (cross‑crate model)
- `Transform { pos: Vec3, rot: Quat }`
- `Velocity { lin: Vec3 }`
- `Projectile { kind, speed, radius_m, damage, owner, life_s }`
- `Health { hp, max }`, `Team { id }`
- `CollisionShape { kind: Capsule/Sphere/AABB, params }`
- `Destructible { id: u64, material: MaterialId }`
- `VoxelProxy { meta: VoxelProxyMeta }`
- `ChunkDirty { list: Vec<UVec3> }`
- `ChunkMesh { map: HashMap<(u32,u32,u32), MeshCpu> }`
- `CarveRequest { did: DestructibleId, center_m: DVec3, radius_m: f64, seed: u64, impact_id: u32 }`
- `Debris { pos, vel, age, life, mass }` (server‑side optional)
- `InputCommand { player_id, action, params }`

Systems
- Server tick (fixed dt): ProjectileIntegrate → Collision → DamageApply → DestructibleRaycast (emit CarveRequest) → VoxelCarve (mutates VoxelProxy, sets ChunkDirty) → GreedyMesh (budgeted jobs) → ColliderRebuild (budgeted jobs) → DebrisSpawn/Integrate (optional).
- Client frame: ReplicationApply → UploadMeshes (ChunkMesh→GPU) → Render (+Overlay).

Jobs
- Long‑running: voxelize, greedymesh per chunk, collider builds. Budgeted each tick; off render thread.

Replication (local loop first)
- For dirty chunks: send a compact CPU mesh (`positions: [f32;3], normals: [f32;3], indices: u32`) keyed by `(DestructibleId, cx,cy,cz)`.
- Later optimization: send occ bit diffs or quantized tris; for now keep simple and correct.

---

## Phase 1 — ECS‑first Destructible Carve (2–3 PRs)

1.1 Components & Config
- Add/expand shared components (location depends on current patterns):
  - Define components in `ecs_core` or a new `ecs_gameplay` module (scoped to gameplay); prefer `ecs_core` for now.
  - Files to touch:
    - `crates/ecs_core/src/components.rs` (add: Destructible, VoxelProxy, ChunkDirty, ChunkMesh, CarveRequest)
    - `crates/ecs_core/src/lib.rs` (pub mod exports)
- Move destructible tuneables to `data_runtime`:
  - Add a `destructible.toml` or extend runtime config model.
  - Files: `crates/data_runtime/src/lib.rs` + new `configs/destructible.rs`.
  - Keys: `voxel_size_m`, `chunk`, `aabb_pad_m`, `max_remesh_per_tick`, `collider_budget`, `max_debris`.
  - Map CLI flags in `server_core::destructible::config` to override config.

1.2 Server Systems (authoritative)
- Add systems module `crates/server_core/src/systems/destructible.rs`:
  - `DestructibleRaycastSystem` — convert projectile impact events into `CarveRequest`s using the existing `raycast_voxels` against the target `VoxelProxy` grid (selection via AABB path moved here). Inputs: projectile segment (p0,p1), Outputs: `CarveRequest`.
  - `VoxelCarveSystem` — apply `carve_and_spawn_debris` to matching `VoxelProxy`, push removed centers to a debris buffer, and mark `ChunkDirty`.
  - `GreedyMeshSystem` (budgeted) — for each dirty chunk, run `voxel_mesh::greedy_mesh_chunk`, write to `ChunkMesh.map`.
  - `ColliderRebuildSystem` (budgeted) — rebuild per‑chunk colliders using `render_wgpu::gfx::chunkcol` logic moved/bridged into a shared helper (extract a light wrapper under `server_core::collision_static::chunks`).
- Files to add/touch:
  - `crates/server_core/src/systems/mod.rs`
  - `crates/server_core/src/systems/destructible.rs`
  - `crates/server_core/src/collision_static/chunks.rs` (move or mirror builder from renderer)
  - `crates/server_core/src/tick.rs` (fixed‑dt scheduler and system run order)

1.3 Client Changes (renderer stop mutating)
- In `crates/render_wgpu/src/gfx/renderer/update.rs`:
  - Feature‑gate carve/mutate path behind `legacy_client_carve` (default off): guard calls to `explode_fireball_against_destructible`, collider rebuilds, and dirty‑chunk meshing.
  - Add a new `UploadMeshesSystem` call site that reads replicated `ChunkMesh` and uploads VB/IB (reuse existing upload code paths — currently in `process_voxel_queues` → factor into a helper `upload_chunk_mesh((did,c), MeshCpu)` in `renderer/init.rs` or a new `renderer/voxel_upload.rs`).
- Add `client_core` crate:
  - `crates/client_core/src/replication.rs` — apply messages into ECS world: update/add `VoxelProxy`, `ChunkMesh`.
  - `crates/client_core/src/upload.rs` — transform `ChunkMesh` → GPU uploads via a trait implemented by the renderer host (pass a callback/closure or channel of upload jobs).

1.4 Tests
- Add a server‑only test harness: carve a sphere at the center with fixed seed on a 32³ grid; assert `removed.centers_m.len()` within tolerance and `ChunkMesh.map.len() > 0`.
- Add a deterministic mesh count test: single 1×1×1 solid → 6 quads; across chunk boundaries expected dirty set size.

Acceptance (Phase 1)
- Server tick produces `ChunkMesh` updates; client renders via uploads only (no carve/mutate in renderer). Frame time no longer blocks on mesh/collider jobs in the render thread.

---

## Phase 2 — Projectile → Damage Pipeline (2 PRs)

2.1 Components & SpecDb
- Create/extend `SpecDb` in `data_runtime` for projectile kinds, radii, speeds, damage, lifetimes.
- Files: `crates/data_runtime/src/specs/projectiles.rs` + loader; add tests.
- Replace hard‑coded constants in `render_wgpu` and `server_core::lib.rs`.

2.2 Server Systems
- `ProjectileIntegrateSystem` — fixed‑dt integration creating segments per tick.
- `CollisionSystem` — broadphase (spatial grid): projectile vs `CollisionShape` and destructible AABB; narrow phase triggers hit events.
- `DamageApplySystem` — reduce `Health`, spawn death events.
- `DestructibleRaycastSystem` — feed `CarveRequest` on destructible hits (already in Phase 1).

2.3 Client Prediction (optional v0)
- Allow client to predict projectile visuals while authoritative server updates arrive (same SpecDb). Defer reconciliation to a later phase if needed.

Acceptance
- Authoritative projectile → damage → carve loop; client renders based on replication.

---

## Phase 3 — Replication & Interest (2–4 PRs)

3.1 `net_core` crate (local loop first)
- Define snapshot types:
  - `EntityHeader { id, archetype }`
  - `Transform`, `Health`, `Projectile`, `Destructible`, `VoxelProxyMeta`, `ChunkMeshDelta { did, chunk, positions, normals, indices }`
- Baseline + delta encoding (simple for v0), per‑client interest via grid around camera/player.

3.2 Server → Client channel (in‑proc)
- For now, keep an in‑process channel applying to client ECS; later swap to sockets/WebSockets.

3.3 Apply & Reconcile (client_core)
- `ReplicationApplySystem` updates/creates entities/components; invalidates uploads when `ChunkMeshDelta` arrives.

Acceptance
- Dirty chunk meshes stream to the client as snapshots/deltas; only chunks in interest radius are sent.

---

## Phase 4 — Data‑Driven Destructible Tagging (1–2 PRs)

4.1 Scene build
- Move destructible registry seeding out of renderer into scene build (server side):
  - For each tagged model instance (GLTF extras or external JSON in `/data/scenes/*`), create `Destructible` + `VoxelProxyMeta` entities with stable IDs.
  - File touch: `crates/render_wgpu/src/gfx/scene.rs` (stop reloading GLTF here; use provided registry) and a new `server_core::scene_build.rs`.

4.2 Authoring data
- Add schema for destructible tagging in `crates/data_runtime/schemas/scene_destructibles.json` and loader.

Acceptance
- Renderer no longer owns destructible seeding; server scene build emits entities; selection logic references ECS instead of renderer locals.

---

## Phase 5 — Renderer Cleanup & Feature Gates (1–2 PRs)

- Remove/guard legacy carve/mutate code in `render_wgpu` (default off), keeping a dev feature to compare paths short‑term.
- Move all gameplay constants out of `update.rs` into `data_runtime`/SpecDb.
- Extract mesh upload code into a small module `renderer/voxel_upload.rs`; add a CPU‑only test for mesh winding/normal invariants (already present in `voxel_mesh`).

Acceptance
- Renderer compiles and runs in a mode where it never mutates world state; gameplay flows via ECS + replication.

---

## File/Module References (concrete changes)

- Move out of renderer:
  - `crates/render_wgpu/src/gfx/renderer/update.rs` — selection, carve, debris, collider rebuilds, chunk queue/mesh; gate with `legacy_client_carve` and migrate to systems.
  - `crates/render_wgpu/src/gfx/renderer/init.rs` — factor chunk mesh upload (VB/IB) into a helper to be called from `client_core`.
  - `crates/render_wgpu/src/gfx/renderer/render.rs` — draw loop unchanged.
- Use existing logic:
  - `crates/server_core/src/destructible.rs` — reuse `raycast_voxels`, `carve_and_spawn_debris`, `queue`, `config` in systems.
  - `crates/voxel_proxy`, `crates/voxel_mesh` — unchanged; orchestrated by systems.
- New modules:
  - `crates/server_core/src/systems/{destructible.rs,projectiles.rs,collision.rs,damage.rs}`
  - `crates/client_core/{src/replication.rs,src/upload.rs,src/lib.rs}`
  - `crates/net_core/{src/snapshot.rs,src/apply.rs}`
  - `crates/data_runtime/src/specs/{projectiles.rs,destructible.rs}`

---

## Tests & Acceptance per Phase

- Phase 1: server‑only carve determinism; mesh count invariants; render thread no blocking jobs.
- Phase 2: projectile → damage loop; hit counts stable; carve emits expected dirty sets.
- Phase 3: replication round‑trip; interest filter excludes far chunks; client renders near chunks only.
- Phase 4: scene build data‑driven; renderer no GLTF reloads for destructibles.
- Phase 5: renderer mutation features behind flags; default build is server‑authoritative.

---

## Risks & Mitigations

- Scope creep: lock phases; ship small PRs with tests.
- API churn: stabilize component names and message types early; add rustdoc.
- Perf regressions: keep budgets configurable; add timing to overlays; include perf smoke in CI.

---

## Next Actions (immediate TODOs)

- [ ] Create `client_core` and `net_core` crates (empty scaffolds).
- [ ] Add ECS components for Destructible/VoxelProxy/ChunkDirty/ChunkMesh/CarveRequest.
- [ ] Implement `VoxelCarveSystem` + `GreedyMeshSystem` in `server_core` and a simple in‑proc replication of `ChunkMesh` updates.
- [ ] Feature‑gate legacy carve in `render_wgpu` and add `UploadMeshesSystem` entrypoint for replicated meshes.

(As we execute, we will split these into sub‑issues and attach PR checklists.)
