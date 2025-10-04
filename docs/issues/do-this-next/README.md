# Do-This-Next — Ordered Execution Plan (Epic #95)

This README captures the next concrete steps to advance the server‑authoritative ECS plan. It reflects the current docs in `docs/issues/` and the completed items in `docs/issues/completed/`.

Status snapshot
- Complete: 95A (preflight/gates), 95B (client_core/net_core scaffolds), 95C (ECS components), 95E1 (mouselook/action‑combat), 95F (renderer voxel_upload), 95E (server trio), 95G (projectiles), 96 (telemetry), 95I (replication v0), 95K (client upload bridge), 95J (job scheduler), 95N (NPCs), 95M (renderer cleanup)
- In progress: 95L (server scene build)
- Ready to start now: 95L wiring (replication) → 95O (controller/camera migration) → 95P (tests/CI expansion)

Order of execution (small, parallel‑friendly PRs)
1) 95L — Server scene build (replication wiring)
   - Emit `net_core::snapshot::DestructibleInstance` over `net_core::channel::Tx` once on startup.
   - Client: apply into a small registry and build GPU visuals (no GLTF loads).
   - Move renderer seeding fully behind legacy/demo features (already gated) and verify default path uses replication only.

2) 95O — Client controller & camera migration
   - Move `apply_pc_transform` math into `client_core::systems::controller`; renderer calls into it and only uploads GPU buffers.
   - Keep winit plumbing in renderer; preserve input responsiveness and tests.

3) 95P — Tests & CI expansion
   - Add deterministic projectiles/collision cases and replication interest tests.
   - Ensure `cargo deny` present locally; keep the matrix green.

Notes
- Flip statuses in each issue file as work lands (set Status, add an Addendum), and move to `docs/issues/completed/` when done.

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
