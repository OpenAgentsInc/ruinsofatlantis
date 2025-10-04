# 95N — NPCs into ECS (Server): Components & Systems

Status: COMPLETE

Labels: ecs, ai, server-authoritative
Depends on: Epic #95, 95G (Server projectile/damage), 95I (Replication)

Intent
- Migrate NPC state/behaviors from `ServerState` vectors to ECS components/systems; replicate to client.

Files
- `crates/server_core/src/systems/npc.rs` (new)
- Port/remove logic from `crates/server_core/src/lib.rs` (resolve collisions, AI loops)
 - Current logic to port:
   - `ServerState::step_npc_ai` (target selection, movement, melee cooldown)
   - `ServerState::resolve_collisions` (npc<->npc and npc<->wizard pushback)
   - Hit application and death handling (currently triggered in renderer explosions and server state)

Tasks
- [x] Components: `Npc { radius, speed_mps, damage, attack_cooldown_s }`, `Velocity` (reuse existing `Transform`, `Health`, `Team`).
- [x] Systems: perception/seek (`npc_ai_seek`), resolve collisions (`resolve_collisions`), melee apply (`melee_apply`).
- [x] Unit tests: seek moves toward target; melee applies damage once per cooldown.
- [ ] Replication & renderer migration tracked separately (95I/95O tie-ins).

Acceptance
- NPC movement/combat runs server-side; client visuals/floaters use replicated components/events.
 - Renderer no longer iterates or mutates `server.npcs` directly.

---

## Addendum — Implementation Summary

- ecs_core
  - Added `components::Velocity` and `components::Npc` with baseline params and attack cooldown tracking.
- server_core
  - New `systems::npc` module with `npc_ai_seek`, `resolve_collisions`, and `melee_apply` helpers; unit-tested and deterministic.
- Integration
  - Existing `ServerState` remains for now; migration of renderer visuals to replicated components will happen under the replication/client-controller tasks.
