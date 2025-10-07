# Deviations & Evidence — 2025-10-07

This lists concrete deviations from `docs/issues/ecs_refactor_part_2.md`, with file:line references and notes.

1) Pre‑ECS ActorStore still in tree (unused by ServerState)
- Evidence: `crates/server_core/src/actor.rs:58` defines `struct ActorStore { actors: Vec<Actor> }` with spawn/get/iter helpers.
- Why it matters: Encourages accidental usage and confuses the authoritative store surface.
- Recommendation: Delete `ActorStore` and related comments in `lib.rs`; keep only ECS world types.

2) Position mirroring + PC respawn bridge from renderer → server
- Evidence: `crates/server_core/src/lib.rs:200-270` `sync_wizards()` mirrors client positions; if PC missing/dead it respawns and reattaches casting resources.
- Why it matters: Not final authority path; respawn should be a server policy and movement should come from intents.
- Recommendation: Replace with authoritative movement intents and explicit respawn policy; deprecate then remove `sync_wizards`.

3) Spatial grid lifecycle
- Evidence: `crates/server_core/src/ecs/schedule.rs:70-83, 1040-1180` rebuilds `SpatialGrid` every tick; used for homing and AoE.
- Why it matters: Rebuilds are O(N) per tick; fine now but not scalable. Doc recommends incremental updates on movement.
- Recommendation: Move grid into `WorldEcs` with dirty‑on‑write updates; expose queries for proximity and segment broad‑phase.

4) Projectile broad‑phase still scans all actors
- Evidence: `crates/server_core/src/ecs/schedule.rs` projectile segment collision still iterates over actors; homing uses grid candidates.
- Why it matters: O(N) per projectile segment; grid can prune candidates by visited cells.
- Recommendation: Use `SpatialGrid::query_circle` or a segment‑box traversal to gather candidate cells and test only their actors.

5) Legacy client AI/combat present (feature gated)
- Evidence: `crates/render_wgpu/src/gfx/renderer/update.rs:2035-2040` calls `collide_with_wizards` under `legacy_client_combat`; more `#[cfg(feature = "legacy_client_*"))]` blocks remain.
- Why it matters: Keeps alternative authority paths alive; risks accidental enablement and increases maintenance.
- Recommendation: Remove these features and code after confirming server paths fully replace them. Keep archaeology in Git history.

6) Legacy replication formats still decoded on client
- Evidence: `crates/client_core/src/replication.rs:218-241` decodes `NpcListMsg`; `BossStatusMsg` fallback below.
- Why it matters: Dual formats complicate testing and migration; doc designates actor snapshots as canonical.
- Recommendation: Remove list/boss compatibility decoders once deltas+interest are stable across all views/HUD.

7) Make v3 deltas default and remove env gate
- Evidence: Platform uses env `RA_SEND_V3` to switch between v2 and v3 (crates/platform_winit/src/lib.rs:330-420).
- Why it matters: Keeping both paths increases complexity; v3 is working with tests.
- Recommendation: Default to v3 deltas (always) and remove v2 path for local/demo transport; keep v2 encode only for tooling as needed.

8) `server_core::ecs` specializes a second ECS instead of wrapping `ecs_core::World`
- Evidence: `crates/server_core/src/ecs/mod.rs:15-17` defines its own world; `ecs_core` is a separate crate for scene assembly.
- Why it matters: Acceptable separation (render‑scene ECS vs server‑actors ECS), but the doc suggested reusing `ecs_core` world wrapper. Decide intentionally and document separation to avoid confusion.
- Recommendation: Either (A) rename server ECS to `actors_ecs` to avoid namespace collision and keep them separate, or (B) rewrap `ecs_core::World` with actor‑components. Document the choice in `crates/server_core/README.md`.
