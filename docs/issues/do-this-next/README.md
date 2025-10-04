# Do-This-Next — Ordered Execution Plan (Epic #95)

This README captures the next concrete steps to advance the server‑authoritative ECS plan. It reflects the current docs in `docs/issues/` and the completed items in `docs/issues/completed/`.

Status snapshot
- Complete: 95A (preflight/gates), 95B (client_core/net_core scaffolds), 95C (ECS components), 95E1 (mouselook/action‑combat), 95F (renderer voxel_upload), 95E (server trio), 95G (projectiles), 96 (telemetry), 95I (replication v0), 95K (client upload bridge)
- In progress: none
- Ready to start now: 95N (NPCs ECS server) or 95J (job scheduler)

Order of execution (small, parallel‑friendly PRs)
1) 95N — NPCs into ECS (server)
   - Move simple NPC AI into ECS components/systems; keep deterministic.
   - Files to add/touch
     - `crates/ecs_core` components for NPC (pos, radius, hp)
     - `crates/server_core/src/systems/npc.rs` (AI + movement)
   - Acceptance: existing zombie ring logic runs via ECS; tests remain deterministic.

2) 95J — Job scheduler (budgeted)
   - Add a tiny budgeted scheduler for server systems (carve→mesh→collider).
   - Files to add/touch: `crates/server_core/src/schedule.rs`
   - Acceptance: unit tests cover budget adherence.
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
 - Completed replication: `docs/issues/completed/95I_replication_v0_snapshots_deltas_interest.md`
 - Upload bridge: `docs/issues/completed/95K_client_upload_bridge.md`
 - Telemetry issue: `docs/issues/96_telemetry_observability.md`
