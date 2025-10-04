# Do-This-Next — Ordered Execution Plan (Epic #95)

This README captures the next concrete steps to advance the server‑authoritative ECS plan. It reflects the current docs in `docs/issues/` and the completed items in `docs/issues/completed/`.

Status snapshot
- Complete: 95A (preflight/gates), 95B (client_core/net_core scaffolds), 95C (ECS components), 95E1 (mouselook/action‑combat), 95F (renderer voxel_upload), 95E (server trio), 95G (projectiles)
- In progress: none
- Ready to start now: 95T (telemetry scaffold), 95I (replication v0)

Order of execution (small, parallel‑friendly PRs)
1) 95T — Telemetry Scaffold (Logs, Metrics, Traces)
   - Initialize `tracing` subscribers and Prometheus metrics (server), pretty console in client dev.
   - Add initial counters/histograms for voxel budgets and controller mode transitions.
   - Files to add/touch
     - `crates/server_core/src/telemetry.rs`, `crates/client_core/src/telemetry.rs`
     - `crates/data_runtime/src/configs/telemetry.rs`
   - Acceptance: `/metrics` exports basic signals; structured logs enabled; no high‑rate logs in hot loops.

2) 95I — Minimal Replication Path (local loop)
   - Local in‑proc channel from server_core → client_core for `ChunkMeshDelta`.
   - client_core applies deltas; renderer uploads via `voxel_upload` during frame.
   - Files to add/touch
     - `crates/net_core/src/channel.rs` (simple bounded channel)
     - `crates/client_core/src/replication.rs` (apply mesh deltas)
     - `crates/render_wgpu/src/gfx/renderer/update.rs` (consume uploads only)
   - Acceptance: a dummy server tick pushes one mesh delta that the client uploads and renders.

3) 95E — Finalize Server Trio (ColliderRebuild + Orchestrator)
   - Add `ColliderRebuildSystem` budget and a simple tick orchestrator that sequences carve → mesh → collider.
   - Instrument with light `destruct_debug` logs (or tracing if 95T lands first).
   - Files to touch
     - `crates/server_core/src/systems/destructible.rs` and `.../mod.rs`
     - `crates/server_core/src/tick.rs` (fixed‑dt scheduler)
   - Tests: budget adherence and collider count matches processed chunks.
   - Acceptance: server tick updates meshes/colliders within budgets.

4) 95T — Telemetry Scaffold (Logs, Metrics, Traces)
   - Initialize `tracing` subscribers and Prometheus metrics (server), pretty console in client dev.
   - Add initial counters/histograms for voxel budgets and controller mode transitions.
   - Files to add/touch
     - `crates/server_core/src/telemetry.rs`, `crates/client_core/src/telemetry.rs`
     - `crates/data_runtime/src/configs/telemetry.rs`
   - Acceptance: `/metrics` exports basic signals; structured logs enabled; no high‑rate logs in hot loops.

5) 95G — Projectile Specs + Server Systems (v0)
   - Add `data_runtime/specs/projectiles.rs`; wire server projectile integrate/collide; emit `CarveRequest` on destructible hits.
   - Keep visuals client‑side; no client mutation.
   - Files to add/touch
     - `crates/data_runtime/src/specs/projectiles.rs`
     - `crates/server_core/src/systems/projectiles.rs`
   - Tests: component lifetime, collision on simple shapes, carve emission on destructible AABB hits.

3) Docs & Hygiene
   - Flip statuses at the top of each issue file as work lands (e.g., “Status: COMPLETE”).
   - Move completed issues from `docs/issues/` to `docs/issues/completed/`.
   - Keep `src/README.md` updated with input keybindings (ALT toggle, RMB hold) and any renderer module changes.

Notes & policies
- Keybindings: avoid F1–F12. Use letters/digits and simple modifiers; ALT for cursor toggle, RMB as hold‑to‑look in Classic profile.
- Default build: no client mutation; feature builds allowed for A/B only.
- CI: ensure `cargo xtask ci` runs tests/clippy for both default and feature combos.

Pointers to specs/design
- Epic plan: `docs/issues/0095_ecs_server_authority_plan.md`
- Server budgets/config: `docs/issues/95D_data_config_destructible_budgets.md`
- Server systems (carve/mesh/collider): `docs/issues/completed/95E_server_systems_voxel_carve_mesh_collider.md`
- Mouselook/action‑combat defaults: `docs/issues/completed/95E1_action_combat_mouselook.md`
- Renderer upload cleanup: `docs/issues/completed/95F_renderer_cleanup_voxel_upload.md`
- Projectiles: `docs/issues/completed/95G_server_projectiles_collision_damage.md`
- Telemetry guidance: `docs/issues/do-this-next/telemetry.md`
