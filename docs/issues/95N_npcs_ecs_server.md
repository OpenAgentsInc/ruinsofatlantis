# 95N — NPCs into ECS (Server): Components & Systems

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
- [ ] Components: `Npc { radius, speed }`, `Transform`, `Velocity`, `Health`, `Team`.
- [ ] Systems: `NpcPerceptionSystem`, `NpcAiSystem`, `NpcResolveCollisionsSystem`, `NpcMeleeSystem`, `NpcDeathSystem`.
- [ ] Replicate `Transform` and `Health` to client; remove renderer use of `server.npcs`.
 - [ ] Replace any client-side aggro toggles (e.g., `wizards_hostile_to_pc`) with server-side systems and replicated flags.

Acceptance
- NPC movement/combat runs server-side; client visuals/floaters use replicated components/events.
 - Renderer no longer iterates or mutates `server.npcs` directly.
