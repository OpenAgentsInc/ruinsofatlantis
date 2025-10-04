# Do-This-Next — Ordered Execution Plan (Epic #95)

This README captures the next concrete steps to advance the server‑authoritative ECS plan. It reflects the current docs in `docs/issues/` and the completed items in `docs/issues/completed/`.

Status snapshot
- Complete: 95A (preflight/gates), 95B (client_core/net_core scaffolds), 95C (ECS components)
- In progress: 95E (server carve/mesh/collider — carve+mesh partial), 95F (renderer voxel_upload — helper present)
- Ready to start now: 95E1 (mouselook/action‑combat default), 95T (telemetry scaffold)

Order of execution (small, parallel‑friendly PRs)
1) 95E1 — Wire Client Controller into Host
   - Implement `client_core` systems (mouselook, cursor toggle, camera) and façade (`ControllerState`, `InputQueue`).
   - Bridge winit events in `render_wgpu` to `client_core`; apply pointer‑lock/visibility via a tiny `CursorApi` adapter.
   - Draw a minimal reticle in the renderer UI when in mouselook mode.
   - Inputs: ALT toggles cursor; RMB hold fallback for Classic profile; Q/E/R, Shift, Tab mapped as commands (no gameplay mutation on client).
   - Files to touch
     - `crates/client_core/src/systems/{mouselook.rs,cursor.rs,camera.rs}`
     - `crates/client_core/src/facade/controller.rs`
     - `crates/render_wgpu/src/gfx/renderer/input.rs` (adapter) and `.../update.rs` (apply camera/reticle)
     - (optional) `data/config/input_camera.toml`
   - Tests: unit tests for pitch clamp and mode toggles.
   - Acceptance: reticle + pointer lock work by default; Classic profile available as fallback.

2) 95F — Migrate Renderer Call‑Sites to voxel_upload
   - Replace direct VB/IB creation and map maintenance in `update.rs` with `renderer::voxel_upload::{upload_chunk_mesh,remove_chunk_mesh}`.
   - Standardize chunk keys via a `chunk_key(did, UVec3)` helper; ensure CPU mesh validation paths bail early on invalid data.
   - Files to touch
     - `crates/render_wgpu/src/gfx/renderer/update.rs`
     - `crates/render_wgpu/src/gfx/renderer/voxel_upload.rs` (helper already present)
   - Acceptance: all chunk mesh insert/remove paths use the helper; default build has no client mutation.

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

6) Minimal Replication Path (pre‑Phase 3)
   - Local in‑proc channel from server_core → client_core for `ChunkMeshDelta`.
   - client_core applies deltas; renderer uploads via `voxel_upload` during frame.
   - Files to add/touch
     - `crates/net_core/src/channel.rs` (simple bounded channel)
     - `crates/client_core/src/replication.rs` (apply mesh deltas)
     - `crates/render_wgpu/src/gfx/renderer/update.rs` (consume uploads only)
   - Acceptance: a dummy server tick pushes one mesh delta that the client uploads and renders.

7) Docs & Hygiene
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
- Server systems (carve/mesh/collider): `docs/issues/95E_server_systems_voxel_carve_mesh_collider.md`
- Mouselook/action‑combat defaults: `docs/issues/95E1_action_combat_mouselook.md`
- Renderer upload cleanup: `docs/issues/95F_renderer_cleanup_voxel_upload.md`
- Telemetry guidance: `docs/issues/do-this-next/telemetry.md`

