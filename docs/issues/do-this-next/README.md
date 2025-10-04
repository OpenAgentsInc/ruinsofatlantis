# Do-This-Next — Ordered Execution Plan (Epic #95)

This README captures the next concrete steps to advance the server‑authoritative ECS plan. It reflects the current docs in `docs/issues/` and the completed items in `docs/issues/completed/`.

Status snapshot
- Complete: 95A (preflight/gates), 95B (client_core/net_core scaffolds), 95C (ECS components), 95E1 (mouselook/action‑combat), 95F (renderer voxel_upload), 95E (server trio), 95G (projectiles), 96 (telemetry)
- In progress: none
- Ready to start now: 95I (replication v0)

Order of execution (small, parallel‑friendly PRs)
1) 95I — Minimal Replication Path (local loop)
   - Local in‑proc channel from server_core → client_core for `ChunkMeshDelta`.
   - client_core applies deltas; renderer uploads via `voxel_upload` during frame.
   - Files to add/touch
     - `crates/net_core/src/channel.rs` (simple bounded channel)
     - `crates/client_core/src/replication.rs` (apply mesh deltas)
     - `crates/render_wgpu/src/gfx/renderer/update.rs` (consume uploads only)
   - Acceptance: a dummy server tick pushes one mesh delta that the client uploads and renders.

2) Docs & Hygiene
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
 - Telemetry issue: `docs/issues/96_telemetry_observability.md`
