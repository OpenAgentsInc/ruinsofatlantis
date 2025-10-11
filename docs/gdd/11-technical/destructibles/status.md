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
   - `crates/server_core/src/tick.rs:10` applies all pending carves to the app

- Demo (tooling only): see `README.md:63` for `--voxel-demo` flags and overlay.
- Tests: run workspace tests; voxel units cover carve/mesh/collider and skip/ordering logic:
  - `crates/voxel_proxy/src/lib.rs:383` (carve/dirty), `crates/voxel_mesh/src/lib.rs:520` (meshing), `crates/collision_static/src/chunks.rs:106` (colliders), `crates/server_core/src/tick.rs:58` (orchestrator)

## Final Notes

- No “foxhole” identifiers exist in code or docs; treat it as the destructible set‑piece feature. All core building blocks are present and tested.
- Default builds keep the renderer presentation‑only; avoid re‑introducing client mutation paths except under tooling features.
- See `docs/issues/*` for the tracked tasks; `xtask ci` enforces layering and cleans stubs/features for the default path.

