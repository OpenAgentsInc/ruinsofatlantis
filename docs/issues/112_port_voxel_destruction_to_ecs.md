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
- [7] Tests: added CPU-only coverage for destructible primitives & registry broad-phase
  - `crates/server_core/tests/destructible_primitives.rs`
  - `crates/server_core/tests/destructible_registry_broadphase.rs`
- [8] ECS compliance fixes
  - Added `Ctx.carves` bus; producers now push `CarveRequest`s
  - Split registry work into schedule systems: apply → mesh → colliders
  - Object-space carve conversion in apply step (future-proof)
  - Platform only sends chunk deltas after instance is sent; tracked per-client set
  - Demo scene registers a simple ruins proxy (`scene_build::add_demo_ruins_destructible`)
- [9] Demo scene: wired `add_demo_ruins_destructible` into platform demo server; platform now sends `DestructibleInstance` before any `ChunkMeshDelta` (tracked via `sent_destr_instances`).
- [10] Validation: ran `cargo test` and pre-push `xtask ci`; all tests green, clippy/wgsl/schema checks pass.
 - [11] Bugfix: removed top slab in demo ruins proxy to prevent hovering block artifacts
   - Updated `scene_build::add_demo_ruins_destructible` to fill only bottom floor + vertical side walls (no ceiling)
   - Added test `demo_ruins_has_no_top_slab` under `crates/server_core/tests/destructible_integration.rs`
 - [12] Client gating + tests
   - Client defers `ChunkMeshDelta` until corresponding `DestructibleInstance` arrives
   - Test: `crates/client_core/tests/destructible_instance_before_delta.rs`
 - [13] Collider refresh queue
   - Registry tracks `touched_this_tick` chunks and refreshes colliders under budget across ticks
   - Test: `crates/server_core/tests/destructible_colliders_budget.rs`
 - [14] Geometry helper centralization
   - Introduced `server_core::ecs::geom::segment_aabb_enter_t`; schedule uses this helper (backfill: dedupe call sites)
 - [15] Backpressure & hooks
   - Carve bus cap added (soft cap; logs dropped count)
   - Pre-commit hook runs `cargo fmt` and stages formatting changes; pre-push runs `cargo xtask ci` (fmt --check, clippy, tests, WGSL, schema)
   - Configured hooks via `scripts/setup-git-hooks.sh`; verified `core.hooksPath=.githooks`
- [16] Final validation and push
  - Ran `cargo xtask ci` locally and via pre-push; all green
  - Ensured working tree clean after push; no stray fmt diffs

- [17] Fix: voxel chunk meshes spawning at origin
  - Root cause: deltas carried object-space positions; renderer applied identity model → chunks drew at origin
  - Fix: server now transforms positions/normals to world-space before encoding `ChunkMeshDelta`
  - Verified visually and with client gating tests

- [18] Explosion → carve robustness
  - Added surface-pick: require OS voxel ray hit from explosion center toward proxy before enqueueing carve
  - Added hard distance guard (`~30m`) to prevent far NPC Fireballs carving distant ruins
  - Tests: `explosion_surface_pick.rs`, `destructible_distance_guard.rs`

- [19] Schedule helpers for structural tests
  - `system_names_for_test()` and `destructible_from_explosions_for_test()` behind `#[cfg(test)]`
  - Test: `explosion_vs_actor_damage_order.rs`

- [20] More tests (server_core/client_core/net_core)
  - Bus cap: `destructible_carve_bus_cap.rs`
  - Projectile gating: `projectile_gating_carves.rs`
  - Segment-AABB math: `geom_segment_aabb.rs`
  - Non-uniform scale (warn + carve apply): `object_space_scale_warn.rs`
  - Collider queue drain: `collider_refresh_touched_budget.rs`
  - Client: defer multiple DIDs until instance (`destructible_defer_multiple_dids.rs`)
  - Client HUD toast decode (`hud_toast_decode.rs`)
  - Net wire round-trips (`destructible_roundtrips.rs`)

Notes for future
- Replace demo proxy with baked `data/voxel/ruins.voxgrid`; keep current box as fallback
- Renderer: hide static ruins draw after first non-empty delta for a DID; unhide when proxy empties
- Consider persistence: append op-log on carves and replay on load


- [2025-10-09 01:48:34Z] Server snapshot: exclude projectile entities from ActorRep
  - Fixes client spawning zombie/NPC views at projectile positions (MM/FB/FBolt)
  - Change: `tick_snapshot_actors()` now filters out `projectile.is_some()`
  - Added test: `crates/server_core/tests/projectiles_not_in_actor_snapshot.rs`
  - Rationale: projectiles are replicated via `projectiles` list only. Actors list is reserved for real actors.

- [2025-10-09 02:41:20Z] Client delta validation hardened (indices bounds, finite verts, empty delta allowed)
  - Code: `crates/client_core/src/replication.rs` strict checks
  - Tests:
    - `crates/client_core/tests/destructible_delta_validation_strict.rs`
    - `crates/client_core/tests/projectiles_do_not_mutate_npcs.rs`
- [2025-10-09 02:41:20Z] ECS test wrappers and projectile→carve/Firebolt gating tests
  - Wrappers in `crates/server_core/src/ecs/schedule.rs` for test-only invocation
  - Tests:
    - `crates/server_core/tests/destructible_from_projectiles_segments.rs`
    - `crates/server_core/tests/cast_cooldown_rejection.rs`
    - `crates/server_core/tests/aoe_capsule_edge.rs`

- [2025-10-09 03:06:29Z] ECS Guide tightened
  - Version consistency: v3→v4; corrected `projectile_collision_ecs` name
  - Replication invariants spelled out: indices in-bounds; finite vertex data; projectile–actor id collision note
  - Added §§ 31–34: Determinism & RNG, Frame Type Tags, Budgets & Back-Pressure, Security & Rate-Limits
  - Expanded §14 testing requirements with replication guard cases

- [2025-10-09 03:39:52Z] Fix: NPC spawns avoid destructible AABBs
  - Added `push_out_of_destructibles()` and applied to `spawn_wizard_npc`, `spawn_undead`, `spawn_death_knight`, and boss spawn.
  - Prevents hidden casters spawning inside the ruins and firing continuously from within geometry.
  - Test: `crates/server_core/tests/spawn_not_inside_destructible.rs`.

- [2025-10-09 04:33:15Z] Renderer: stable wizard transforms from replication; gate DK draw on boss_status; bars cull with replicated PC
- [2025-10-09 04:33:15Z] Server: spawn separation vs actors; tests for DK stacking; PC & boss despawn tests added
