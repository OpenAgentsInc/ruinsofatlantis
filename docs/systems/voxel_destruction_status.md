# Voxel/Destructible System — Status after ECS Refactor

This note captures the current state of the voxel/destructible (“foxhole destruction”) pipeline after the recent ECS/server‑authority refactor, where the code now lives, what was removed or gated, and a concrete plan to bring it fully online in the new architecture.

## Summary
- The renderer no longer mutates destructible world state in the default build. Client‑side carve/mesh/collider paths were removed or feature‑gated.
- Authoritative logic for carving, greedy meshing, and (coarse) colliders lives in server‑side crates with unit tests.
- Replication types exist for per‑chunk voxel meshes and per‑instance AABBs; the renderer implements a thin upload bridge to GPU buffers.
- A small, self‑contained demo path (`vox_onepath_demo`) remains for local tooling/screenshots only.

“Foxhole” is not a code identifier in the repository; it appears to describe the destructible set‑piece workflow generally (cover you can carve, debris spawns, etc.). All relevant code is present; no recovery from old history is required.

## Where Things Live (by crate)

- `crates/voxel_proxy/src/lib.rs:1` — Voxel grids, flood‑fill voxelization, carve ops, dirty chunk tracking
  - `VoxelProxyMeta`, `VoxelGrid`, `voxelize_surface_fill`, `carve_sphere` (returns removed centers and marks dirty chunks)
- `crates/voxel_mesh/src/lib.rs:1` — CPU greedy meshing of chunked voxel grids
  - `greedy_mesh_chunk`/`greedy_mesh_all`; normals/winding tests present
- `crates/collision_static/src/chunks.rs:1` — Coarse per‑chunk colliders (AABB/OBB) for debris‑vs‑world preview
- `crates/server_core/src/destructible.rs:1` — Raycast (DDA) into voxel grid, carve + seeded debris spawn
- `crates/server_core/src/systems/destructible.rs:1` — ECS systems: apply carves, greedy mesh budget, collider rebuild budget
- `crates/server_core/src/tick.rs:1` — Orchestrator: carve → mesh (budgeted) → colliders (budgeted)
- `crates/server_core/src/systems/projectiles.rs:1` — Integrate segments; collide with destructible AABBs; emit `CarveRequest`
- `crates/data_runtime/src/configs/destructible.rs:1` — Data‑driven budgets/tuning (TOML) with clamping
- `crates/net_core/src/snapshot.rs:24` — Replication messages
  - `ChunkMeshDelta` (per‑chunk CPU mesh)
  - `DestructibleInstance` (did + world AABB)
- `crates/client_core/src/replication.rs:1` — Client delta buffer; decodes `ChunkMeshDelta` and exposes uploads
- `crates/render_wgpu/src/gfx/renderer/upload_adapter.rs:1` — Renderer implements `client_core::upload::MeshUpload` (uploads/removes chunk meshes)
- `crates/render_wgpu/src/gfx/renderer/voxel_upload.rs:1` — GPU upload helpers for voxel chunk meshes
- `crates/render_wgpu/src/gfx/renderer/render.rs:160` — Replication drain each frame → call MeshUpload to apply chunk mesh updates
- `crates/render_wgpu/src/gfx/vox_onepath.rs:1` — Feature‑gated one‑path demo (procedural block, carve burst, screenshot)

Docs and design notes you can read alongside the code:
- `README.md:63` — Voxel Destructibility Demo flags and overlay
- `docs/research/hybrid-voxel-system.md:1` — Rationale for hybrid mesh+voxel approach; pre‑voxelized assets guidance
- `docs/issues/95L_server_scene_build_destructibles.md:1` — Scene‑driven destructible registry; replication types
- `docs/issues/95Q_remove_legacy_client_carve.md:1` — Removal plan for legacy client carve paths (now done)

## What Changed (git history, condensed)

- Scaffolded system:
  - `446f61c` — voxel_proxy + voxel_mesh scaffolds (chunked grid, flood‑fill; greedy faces)
  - `2d34bea` — Per‑chunk meshing API + coarse chunk colliders
  - `ff815db` — Voxel DDA raycast; carve + seeded debris
  - `a3e493a`/`d98d887` — Wire queues + overlay; per‑chunk greedy meshing + draw
  - `515d1e8`/`bc15077` — Triplanar shader for voxel meshes; normals/winding test
- ECS/server‑authority move:
  - `82cc738` — ECS components (`Destructible`, `VoxelProxy`, `ChunkDirty`, `ChunkMesh`, `CarveRequest`)
  - `a811c2d` — Data‑driven destructible budgets (TOML)
  - `b39e1f6`/`85410db`/`1df6000` — VoxelCarve + GreedyMesh systems, `tick_destructibles`, `JobScheduler`
  - `2de32c0` — Scene destructibles schema + server world AABB builder; replication plumbing
- Renderer cleanup/gating:
  - `357c133`/`29e562a`/`2826799` — Remove legacy client carve from default; enforce `render_wgpu` no‑default doesn’t link `server_core`; keep `vox_onepath_demo` gated for tools only

No repository issues/PRs were found via `gh` search for “voxel/destruct/debris”; the repo tracks work in `docs/issues/*` instead.

## Current Runtime Flow (server‑authoritative)

1) Projectile integration and collision
   - `crates/server_core/src/systems/projectiles.rs:1` integrates segments and collides against destructible world AABBs (server‑built). On intersection, emit `CarveRequest { did, center_m, radius_m, … }`.

2) Destructible tick (deterministic/budgeted)
   - `crates/server_core/src/tick.rs:10` applies all pending carves to the appropriate grid, greedily meshes up to `max_chunk_remesh`, and rebuilds a coarse subset of chunk colliders up to `collider_budget_per_tick`.

3) Replication (messages exist; wiring is partial)
   - `crates/net_core/src/snapshot.rs:25` `ChunkMeshDelta { did, chunk, positions, normals, indices }` encodes CPU meshes.
   - `crates/net_core/src/snapshot.rs:760` `DestructibleInstance { did, world_min, world_max }` provides world AABBs for instances.
   - Platform currently sends actor deltas every frame (`crates/platform_winit/src/lib.rs:420`). Emission for `DestructibleInstance` (once) and `ChunkMeshDelta` (on change) is the next wiring step (see Plan).

4) Client apply / GPU upload
   - Renderer drains messages each frame (`crates/render_wgpu/src/gfx/renderer/render.rs:160`), decodes via `client_core::replication::ReplicationBuffer`, and invokes its `MeshUpload` implementation (`crates/render_wgpu/src/gfx/renderer/upload_adapter.rs:8`) to upload/remove per‑chunk meshes via `voxel_upload`.

5) Visual debris
   - Debris spawn is computed server‑side by `carve_and_spawn_debris` (`crates/server_core/src/destructible.rs:150`). The renderer retains a local cube‑debris visual with simple gravity/ground collision; coarse debris‑vs‑world using per‑chunk colliders can be toggled via `--debris-vs-world` in demo builds (default off).

## Legacy/Tooling Path (kept for archaeology)

- The self‑contained demo (`vox_onepath_demo`) builds a procedural block, carves it on input, and forces immediate remesh for screenshots: `crates/render_wgpu/src/gfx/vox_onepath.rs:1`.
- Default builds do not include or use this path; CI enforces that `render_wgpu` does not depend on `server_core` without features (`xtask` layering guard).
- `src/README.md:136` flags these as deprecated and not for general use.

Notably, the old GLTF tri→voxel path was removed during cleanup. The remaining `voxelize_surface_fill` assumes a precomputed surface mask; see “Pre‑voxelized assets” below.

## Scene/Instance Data

- Minimal TOML loader for destructible instances: `crates/data_runtime/src/scene/destructibles.rs:1`.
- Server builds world‑space AABBs from these declarations: `crates/server_core/src/scene_build.rs:1`.
- Net type `DestructibleInstance` exists; platform should emit these once so the client can cull/organize proxies without loading GLTFs.

## Open Gaps

- Server→client emission is not yet implemented for destructible messages:
  - `DestructibleInstance` (once on startup/zone load)
  - `ChunkMeshDelta` (on meshed‑chunk changes from `tick_destructibles`)
- Per‑destructible multi‑proxy ownership on the server:
  - Systems operate on a `VoxelGrid` today; ECS scaffolding (`DestructibleId`, `ChunkMesh`) is multi‑proxy ready, but the orchestration needs a map `did → VoxelGrid + ChunkDirty + ChunkMesh`.
- Asset voxelization:
  - Runtime tri→voxelization was intentionally dropped. Adopt pre‑voxelized assets: embed object‑space voxel data or generate a sidecar once at import time (see `docs/research/hybrid-voxel-system.md:71`).
- Debris vs. world on the client is a demo toggle only. If kept, consider server‑side debris and a replicated lightweight debris FX record; otherwise keep the current client‑only visual.

## Plan to Bring It Fully Online

Short, concrete steps that align with the new ECS/server authority:

1) Multi‑proxy state on the server
   - Maintain `HashMap<DestructibleId, { grid: VoxelGrid, dirty: ChunkDirty, meshes: ChunkMesh, colliders: Vec<StaticChunk> }>` in server state.
   - Route `CarveRequest.did` to the right grid; call `tick_destructibles` per‑proxy within budgets (or batch by collecting dirty sets across proxies and round‑robin).

2) Replicate chunk meshes and instances
   - On first scene build, emit one `DestructibleInstance` per proxy over the local transport.
   - Whenever `greedy_mesh_budget` produces triangles for a chunk, encode and send a `ChunkMeshDelta` for `(did, chunk)`; send an empty/zero‑index delta to remove an emptied chunk.
   - Files to touch:
     - Server: add emit points near `tick_destructibles` and scene build.
     - Platform: package these alongside actor deltas in `crates/platform_winit/src/lib.rs:420`.

3) Client upload (already present)
   - Renderer’s `MeshUpload` impl is ready; it uploads per‑chunk CPU meshes to GPU (`crates/render_wgpu/src/gfx/renderer/upload_adapter.rs:8`). Nothing extra needed once messages flow.

4) Pre‑voxelized assets (replace tri→voxel at runtime)
   - Add a small tool (under `tools/`) to bake object‑space voxel proxies at import time (voxel size, dims, chunk size, material). Store alongside GLTF (`*.voxel.json` or binary) and load via `data_runtime`.
   - Consume baked proxies on the server when building destructible entities; skip runtime voxelization entirely.

5) Optional: debris‑vs‑world polish
   - If kept, refresh colliders in tandem with mesh updates and keep the client toggle wired (`--debris-vs-world`). Otherwise, leave as a demo visual with coarse ground bounce only.

## How to Preview Today (for sanity)

- Demo (tooling only): see `README.md:63` for `--voxel-demo` flags and overlay.
- Tests: run workspace tests; voxel units cover carve/mesh/collider and skip/ordering logic:
  - `crates/voxel_proxy/src/lib.rs:383` (carve/dirty), `crates/voxel_mesh/src/lib.rs:520` (meshing), `crates/collision_static/src/chunks.rs:106` (colliders), `crates/server_core/src/tick.rs:58` (orchestrator)

## Final Notes

- No “foxhole” identifiers exist in code or docs; treat it as the destructible set‑piece feature. All core building blocks are present and tested.
- Default builds keep the renderer presentation‑only; avoid re‑introducing client mutation paths except under tooling features.
- See `docs/issues/*` for the tracked tasks; `xtask ci` enforces layering and cleans stubs/features for the default path.

