# 95J — Job Scheduler: Budgeted Mesh/Collider/VOX Jobs

Status: COMPLETE

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
- [x] Synchronous scheduler scaffold (can expand to thread pool later).
- [x] Budgets/tick integration: dispatch up to N jobs per type per tick; collect completed results; update components.
- [x] Metrics: histograms and batch counters (simple).

Acceptance
- With many dirty chunks, only `max_remesh_per_tick` / `collider_budget_per_tick` are processed each tick; logs show budgets adhered.
 - Server tick remains below target frame budget (e.g., 16.6 ms for 60Hz) on a mid config; add a warning log when exceeded.

---

## Addendum — Implementation Summary

- Added `server_core::jobs::JobScheduler` (sync). Two helpers:
  - `dispatch_mesh(budget, f)` and `dispatch_collider(budget, f)` — record elapsed ms and increment batch counters.
- Integrated into `server_core::tick::tick_destructibles` to run greedy mesh and collider rebuild under `DestructibleConfig` budgets (`max_remesh_per_tick`, `collider_budget_per_tick`).
- Left room to expand to async/threaded scheduling later without changing call sites.
