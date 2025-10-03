# 95J — Job Scheduler: Budgeted Mesh/Collider/VOX Jobs

Labels: jobs, performance
Depends on: Epic #95, 95E (Server systems)

Intent
- Add a lightweight job scheduler for long-running CPU tasks (greedy mesh, collider build, optional voxelization) with per-tick budgets.

Outcomes
- Server remains responsive under heavy dirty sets; jobs respect budgets; metrics surfaced.

Files
- `crates/server_core/src/jobs/{mod.rs,thread_pool.rs,job_types.rs}` (new)
- Integrate into: `crates/server_core/src/systems/destructible.rs` (GreedyMesh/Collider systems dispatch jobs)
 - Budget values come from `data_runtime` (95D) via `DestructibleConfig`.

Tasks
- [ ] Thread pool (scoped to server) and MPSC queues per job type.
- [ ] Job structs: `MeshChunkJob`, `BuildColliderJob`, `VoxelizeSurfaceJob` (future).
- [ ] Budgets/tick integration: dispatch up to N jobs per type per tick; collect completed results; update components.
- [ ] Metrics: counters for dispatched/completed jobs and durations; log under `destruct_debug`.

Acceptance
- With many dirty chunks, only `max_remesh_per_tick` / `collider_budget_per_tick` are processed each tick; logs show budgets adhered.
 - Server tick remains below target frame budget (e.g., 16.6 ms for 60Hz) on a mid config; add a warning log when exceeded.
