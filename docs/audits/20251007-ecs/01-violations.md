ECS Architecture Guide Violations (detailed)

Legend
- Severity: P0 (blocker), P1 (important), P2 (polish)
- Files are workspace‑relative with line anchors where applicable.

1) Archetype branching in systems (P0)
- Guide: “No hard‑coded archetypes in logic. Systems match on components, not names.”
- Evidence:
  - crates/server_core/src/ecs/schedule.rs:97 — filters `matches!(a.kind, ActorKind::Wizard)` in `wizard_targets`.
  - crates/server_core/src/ecs/schedule.rs:658 — NPC wizard filter uses `ActorKind::Wizard`.
  - crates/server_core/src/ecs/schedule.rs:184 — sets projectile entity `kind: ActorKind::Wizard` (projectiles should not be ‘Wizard’).
  - crates/server_core/src/lib.rs:787–789 — maps ActorKind to numeric in replication.
- Impact: Logic keyed on archetype names makes features brittle and blocks data‑driven expansion.

2) Projectile broad‑phase is O(N) (P0)
- Guide: “No broad O(N) scans; use grid … query_segment.”
- Evidence:
  - crates/server_core/src/ecs/schedule.rs:774+ — `projectile_collision_ecs` iterates all actors per projectile.
  - SpatialGrid currently used only for select AoE query; projectile path ignores it.
- Impact: Scales poorly, risks perf cliffs with many actors/projectiles.

3) Replication schema gaps & legacy messages (P0)
- Guide: ActorRep includes `archetype_id`/`name_id`; wire uses IDs, not strings. One replication mode (v3 delta); no legacy paths.
- Evidence:
  - crates/net_core/src/snapshot.rs: ActorRep lacks `archetype_id`/`name_id`.
  - crates/net_core/src/snapshot.rs:818 — `BossStatusMsg { name: String, … }` persists; tests exist.
  - crates/net_core/src/snapshot.rs:888 — `NpcListMsg` persists; tests exist.
- Impact: UI/model mapping can’t be fully data‑driven; string payloads on wire; legacy types encourage drift.

4) Hard‑coded literals in systems/spawns (P1)
- Guide: “Systems ask Specs; change data—not code.”
- Evidence examples:
  - Arming delay constants in projectile collisions: crates/server_core/src/ecs/schedule.rs:744–752 (`0.10`, `0.08`).
  - Spawn defaults: crates/server_core/src/lib.rs:566+ (Undead move/aggro/melee), 655+ (DK), 607+ (Wizard pool/cooldowns).
- Impact: Tuning requires code edits; risks inconsistencies and regressions.

5) HitFx bypasses event bus (P1)
- Guide: “Evented side‑effects. Damage/explosions/events flow through buses; no ad‑hoc mutation.”
- Evidence:
  - We write VFX hits to `ServerState.fx_hits` directly in `projectile_collision_ecs` instead of `Ctx.hits` (which exists but is unused for replication): crates/server_core/src/ecs/schedule.rs:820.
- Impact: Cross‑cutting side‑effects bypass the per‑tick context; makes testing/collection less uniform.

6) ‘No panics’ not enforced (P0)
- Guide: “No panics in normal gameplay.”
- Evidence (non‑test uses of `unwrap/expect`):
  - crates/server_core/src/tick.rs:68, systems/projectiles.rs:220, systems/projectiles.rs:231, destructible.rs paths (multiple).
- Impact: Runtime panics under data quirks; brittle.

7) Observability gaps (P2)
- Guide: per‑system tracing spans with counters.
- Evidence: No spans around systems in Schedule; ad‑hoc logs exist.
- Impact: Harder perf debugging and regression tracking.

8) CI grep guards not enforced (P1)
- Guide: CI gates for `legacy_client_`, `NpcListMsg`, `BossStatusMsg`, `ActorStore`.
- Evidence: Types still exist and README references legacy flags; no explicit grep step failing PRs.
- Impact: Risk of regressions sneaking back.

