# Server Systems & Scheduler — 2025-10-04

Snapshot
- Orchestrator: `tick_destructibles` performs carve → greedy-mesh → collider rebuild with budgets and a lightweight scheduler (crates/server_core/src/tick.rs:1,36,48).
- Budgets: `DestructibleConfig { max_chunk_remesh, collider_budget_per_tick }` with tests asserting caps (crates/server_core/src/destructible.rs:269,300; crates/server_core/src/tick.rs:74).
- Scheduler: `JobScheduler` dispatches budgeted jobs (crates/server_core/src/jobs/mod.rs:8,23,29) — currently synchronous closures; extendable to a job pool later.

Risks
- `unwrap` calls in server systems can panic in prod (evidence/panics-server.txt).
- Limited metrics around per-phase times and budgets consumed.

Findings
- F-SIM-009: Replace `unwrap/expect` in server paths with error handling or `expect` with actionable messages; avoid panics in prod (P1 Med).

Recommendations
- Emit per-phase metrics: `carve.count/time`, `mesh.budget/used/time`, `collider.budget/used/time`.
- Track chunk queue sizes and longest-waiting chunk age for backpressure insights.
- Consider multi-threaded dispatcher in the future; keep determinism via stable queues and bounded work.

