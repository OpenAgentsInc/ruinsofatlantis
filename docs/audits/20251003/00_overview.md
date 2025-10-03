# Ruins of Atlantis — Codebase Audit (2025‑10‑03)

Author: Internal engineering audit (MMO/ECS readiness)

Scope
- Repo‑wide review with focus on MMO production readiness and ECS/systemization.
- Identify hard‑coded logic, monolithic flows, and client/server coupling.
- Recommend incremental, low‑risk steps to converge on server‑authoritative ECS.

Snapshot
- Workspace: modular crates for renderer (`render_wgpu`), platform (`platform_winit`), sim (`sim_core`), data (`data_runtime`), HUD (`ux_hud`), minimal ECS (`ecs_core`), and server helpers (`server_core`). Tools under `tools/` and `xtask` automation in place.
- Recent wins: voxel destructibles split into `voxel_proxy` + `voxel_mesh` with unit tests; per‑chunk budgets; logging/diagnostics added; demo paths gated; CI runs xtask.
- Tests: strong for data/sim and voxel CPU helpers; orchestration/replication tests missing.

Key Strengths
- Clear crate boundaries and strong documentation hygiene.
- Determinism ethos (seeded RNG, CPU‑only tests where possible).
- Modern WGPU path with organized WGSL and validation wired via xtask.

Top Risks (MMO/ECS)
1) Renderer update performs gameplay and world mutations. Input, projectiles, explosion damage, destructible carving, debris, collider rebuilds, and chunk meshing live in `gfx/renderer/update.rs`; these should be ECS systems, server‑authoritative.
2) Hard‑coded game constants and topology. Projectile radius/damage, NPC radii/speeds, AABB paddings, budgets live in code; should be data/config‑driven.
3) No replication/interest/prediction layers. Client writes world; no snapshot/delta protocol or spatial interest management; no reconciliation under latency.
4) Ad‑hoc queues/maps in client. Work queues and maps (`voxel_meshes`, `chunk_colliders`, etc.) bypass ECS; concurrency/backpressure not formalized.
5) Heavy CPU jobs on render thread. Voxelization/meshing/collider rebuilds occur inline; needs a job system with budgets.

High‑Impact Recommendations (90 days)
- ECS‑first carve/damage path. Define components (`Destructible`, `VoxelProxy`, `ChunkDirty`, `ChunkMesh`, `Projectile`, `Debris`) and systems for carve → mesh → colliders with budgets. Renderer only uploads/draws.
- Server authority & replication skeleton. Fixed tick server that integrates projectiles, resolves collision/damage, applies `CarveRequest`s; client consumes snapshots/deltas with basic spatial interest.
- Job scheduler. Lightweight job system crate for voxelization/meshing/colliders with budgeted execution; integrate into server tick.
- Data/config hygiene. Move projectile/NPC/destructible constants to `data_runtime`/config; centralize destructible tagging in scene build/data.
- Renderer orchestration split. Extract client input/controller, gameplay, and collision into `client_core` systems; renderer consumes scene inputs and component data only.

Deliverables in this audit set
- Specific per‑area audits and prioritized backlog:
  - Renderer refactor targets (03_renderer_audit.md)
  - Simulation/Server/ECS plan (04_sim_audit.md)
  - Data pipeline/ID policy/destructible tagging (05_data_pipeline_audit.md)
  - Platform/tools/input hard‑coding cleanup (06_platform_and_tools.md)
  - CI/DevEx strengthening (07_ci_and_devx.md)
  - Security/licensing notes (08_security_and_licensing.md)
