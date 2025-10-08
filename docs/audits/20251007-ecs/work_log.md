2025-10-07 — ECS Architecture Audit Log

What I did
- Read docs/ECS_ARCHITECTURE_GUIDE.md end‑to‑end.
- Surveyed server systems/replication for guide compliance.
- Gathered evidence via ripgrep for archetype branches, O(N) scans, unwraps, and legacy message types.
- Wrote violations matrix and a remediation plan with owners and acceptance.

Evidence added
- evidence/rg-archetype-branches.txt — ActorKind in logic and projectiles.
- evidence/rg-broadphase.txt — projectile O(N) scan excerpt.
- evidence/rg-unwrap.txt — non‑test unwraps in server_core.
- evidence/rg-replication-legacy.txt — legacy messages + ActorRep gaps.

Notes
- Many current behaviors are close to the guide (server‑auth, v3 deltas, evented damage/AoE, fixed schedule). The biggest gaps are performance (broad‑phase), data‑driven constants, and replication IDs.
- Minimal changes are needed for most items (grid query, schema fields, moving literals to Specs, and normalizing HitFx to Ctx).

Next steps (suggested ordering)
1) Implement grid query_segment and use it in projectile collision (P0).
2) Add `archetype_id/name_id/unique` to ActorRep and remove BossStatus/NpcList from runtime (P0).
3) Literals → Specs for arming delay and spawn defaults (P1).
4) Normalize HitFx through Ctx, then drain to ServerState at end of tick (P1).
5) Deny unwrap/expect and sweep server_core (P0).
6) Tracing spans + counters (P2) and CI grep guard (P1).

