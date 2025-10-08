2025-10-07 — ECS Architecture Compliance Audit (Deep‑Dive)

Scope
- Read docs/ECS_ARCHITECTURE_GUIDE.md and audited the workspace for violations.
- Produced concrete remediation tasks with acceptance criteria, file refs, and owners.

Top Findings (summary)
- Archetype branches in server systems (uses `ActorKind::Wizard`/`Zombie`/`Boss` in logic) — replace with component/Faction predicates.
- Projectile collision uses O(N) scan over actors — add SpatialGrid `query_segment` and use it; keep full scan out of hot path.
- Hard‑coded gameplay literals in systems/spawns (arming delays, default melee/move/aggro) — move to Specs/data.
- Replication schema gaps vs guide — `ActorRep` missing `archetype_id`/`name_id`; legacy `BossStatusMsg`/`NpcListMsg` still exist.
- Evented side‑effects: `HitFx` bypasses the Ctx event bus (written to `ServerState.fx_hits` directly).
- “No panics” — several `unwrap/expect` remain in server_core destructible/systems (non‑test).
- Observability — schedule lacks tracing spans and per‑system counters specified by the guide.

Risk & Priority
- P0: projectile broad‑phase (perf/correctness), archetype branches, replication IDs, no‑panic guarantee.
- P1: literals → Specs, event bus normalization for HitFx, CI grep guard enforcement.
- P2: spans/counters, incremental grid, remove legacy snapshot types from runtime.

High‑level Plan
1) Cut archetype naming from logic (use Faction component or other component predicates).
2) Add grid `query_segment` and use for projectile broad‑phase.
3) Extend `ActorRep` with `{ archetype_id, name_id, unique }`; remove `BossStatusMsg` from runtime path.
4) Move literals to Specs (arming delay, default melee/move/aggro) and data_runtime.
5) Normalize `HitFx` through `Ctx.hits` event bus; platform drains Ctx not ServerState.
6) Sweep `unwrap/expect` in server_core; add deny‑lints and fix.
7) Add tracing spans/counters in each system.
8) Enforce CI grep guards for legacy types/flags.

See 01‑violations.md for details, evidence, and 02‑remediation‑plan.md for tasks.
