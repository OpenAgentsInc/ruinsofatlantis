Issue: #112 — Port voxel destruction system to ECS

Log start: committing incremental changes with pre-push checks green. This file tracks what I read, design choices, code changes, and validation per step.

Read/Context
- Read gh issue #112 (plan includes data model, schedule wiring, replication, tests).
- Skimmed docs: `docs/ECS.md`, `docs/ECS_ARCHITECTURE_GUIDE.md`, `docs/systems/voxel_destruction_status.md`, `docs/research/hybrid-voxel-system.md`.
- Repo already has: voxel grid (`crates/voxel_proxy`), greedy mesher (`crates/voxel_mesh`), server destructible helpers (`crates/server_core/src/destructible.rs`), an orchestrator for a single grid (`crates/server_core/src/tick.rs`), projectile collision producing `CarveRequest` (`crates/server_core/src/systems/projectiles.rs`), net types for `ChunkMeshDelta` and `DestructibleInstance` (`crates/net_core/src/snapshot.rs`), and client buffer applying chunk-mesh deltas (`crates/client_core/src/replication.rs`).

Plan (incremental)
1) Add server-side registry: multi-proxy destructible state with pending carves and per-proxy dirty/meshes/colliders. (scaffold + unit tests)
2) Wire ECS schedule: collide projectiles vs destructible AABBs → push `CarveRequest` into registry.
3) Apply carves + budgeted meshing/colliders each tick; collect mesh deltas.
4) Replicate: emit `DestructibleInstance` once and `ChunkMeshDelta` per changed chunk in platform_winit demo server.
5) Keep default renderer path unchanged; no client-side mutation.
6) Add focused tests where feasible (CPU-only).

Notes
- Avoid hand-editing Cargo.toml; reuse existing crates. No new deps planned.
- Keep changes scoped to `server_core` and `platform_winit` for demo loopback.
- Honor budgets/config from `destructible::config::DestructibleConfig`.

Commits
- [1] Scaffold registry types in `server_core`, no behavior change.
- [2] Wire ECS schedule: add `destructible_from_projectiles` system to translate projectile segments → `CarveRequest`s against scene AABBs.
- [3] Add `ServerState` fields for destructible registry/instances + helpers to drain mesh deltas and list instances.
- [4] Platform demo server: send one-time `DestructibleInstance` and per-change `ChunkMeshDelta` over local loopback.
- [5] Convert `server_core::destructible` into a proper module (`destructible/mod.rs`) and move new `state.rs` under it to avoid module conflicts; restored required helpers (`raycast_voxels`, `carve_and_spawn_debris`, `queue`, `config`).
- [6] Build fixed with `cargo check`; next step will run full `xtask ci` on push.
