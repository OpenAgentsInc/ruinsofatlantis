# Deviations & Evidence — 2025-10-07

This lists concrete deviations from `docs/issues/ecs_refactor_part_2.md`, with file:line references and notes.

1) Pre‑ECS ActorStore still in tree (unused by ServerState)
- Evidence: `crates/server_core/src/actor.rs:58` defines `struct ActorStore { actors: Vec<Actor> }` with spawn/get/iter helpers.
- Why it matters: Encourages accidental usage and confuses the authoritative store surface.
- Recommendation: Delete `ActorStore` and related comments in `lib.rs`; keep only ECS world types.

2) Position mirroring bridge from renderer → server
- Evidence: `crates/server_core/src/lib.rs:168` `pub fn sync_wizards(&mut self, wiz_pos: &[Vec3])` mirrors client positions into server actors, also lazily spawns PC/NPC wizards.
- Why it matters: Not the final authority path; server should own wizard transforms via input intents and prediction/reconciliation. The bridge is okay for demo but differs from the doc’s InputSystem intent.
- Recommendation: Replace with input‑intent components and server‑side movement; deprecate and then delete `sync_wizards`.

3) Spatial grid lifecycle
- Evidence: `crates/server_core/src/ecs/schedule.rs:41-53` rebuilds `SpatialGrid` every tick; grid type at `:323-364`.
- Why it matters: Rebuilds are O(N) per tick; fine now but not scalable. Doc recommends incremental updates on movement.
- Recommendation: Move grid into `WorldEcs` with dirty‑on‑write updates; expose queries for proximity and segment broad‑phase.

4) Projectile broad‑phase still scans all actors
- Evidence: `crates/server_core/src/ecs/schedule.rs:171-213` iterates every actor for segment intersection; separate proximity explode pass at `:215-238`.
- Why it matters: O(N) per projectile segment; grid can prune candidates by visited cells.
- Recommendation: Use `SpatialGrid::query_circle` or a segment‑box traversal to gather candidate cells and test only their actors.

5) Legacy client AI/combat present (feature gated)
- Evidence: `crates/render_wgpu/src/gfx/renderer/update.rs:2035-2040` calls `collide_with_wizards` under `legacy_client_combat`; numerous additional blocks behind `legacy_client_ai` and `legacy_client_combat`.
- Why it matters: Keeps alternative authority paths alive; risks accidental enablement and increases maintenance.
- Recommendation: Remove these features and code after confirming server paths fully replace them. Keep archaeology in Git history.

6) Legacy replication formats still decoded on client
- Evidence: `crates/client_core/src/replication.rs:162-180` decodes `NpcListMsg`; `BossStatusMsg` fallback below.
- Why it matters: Dual formats complicate testing and migration; doc designates actor snapshots as canonical.
- Recommendation: Remove list/boss compatibility decoders once deltas+interest are stable across all views/HUD.

7) Server logging based on env flags inside hot paths
- Evidence: `crates/server_core/src/lib.rs:142-162` logs Fireball spawn when `RA_LOG_FIREBALL=1`.
- Why it matters: Benign, but consider `tracing` with level filters and per‑system metrics rather than ad‑hoc env checks.
- Recommendation: Convert to `tracing` and metrics counters/histograms per system as suggested in the doc.

8) `server_core::ecs` specializes a second ECS instead of wrapping `ecs_core::World`
- Evidence: `crates/server_core/src/ecs/mod.rs:15-17` defines its own world; `ecs_core` is a separate crate for scene assembly.
- Why it matters: Acceptable separation (render‑scene ECS vs server‑actors ECS), but the doc suggested reusing `ecs_core` world wrapper. Decide intentionally and document separation to avoid confusion.
- Recommendation: Either (A) rename server ECS to `actors_ecs` to avoid namespace collision and keep them separate, or (B) rewrap `ecs_core::World` with actor‑components. Document the choice in `crates/server_core/README.md`.

