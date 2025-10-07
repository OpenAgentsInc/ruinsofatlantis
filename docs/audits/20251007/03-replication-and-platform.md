Replication & Platform Integration

Replication (server → client)
- Actors: `server_core::tick_snapshot_actors()` emits actors with `ActorKind::{Wizard,Zombie,Boss}` and core fields (pos, yaw, radius, hp/max, alive).
- Projectiles: emitted each tick from ECS entities with `Projectile` and `Velocity` as `ProjectileRep`.
- Boss status: `ServerState::nivita_status()` provides HUD fields for the unique boss.

Platform (demo loop)
- Steps server, then builds deltas and sends frames (v3 delta + projectiles).
- Still mirrors wizard positions via `sync_wizards()`; intents not yet wired.

Renderer
- Builds visuals from replication only in default build; projectile VFX and hit explosions now derive from replicated data and disappearance events.
- Death Knight visuals exist in renderer, but no corresponding ECS actor is replicated today (server never spawns DK).

Implications
- To render DK as an actor, server must spawn/register it and include in actor replication (either as Boss with a subkind or introduce a new kind flag).
- Switching to authoritative intents will remove the last non‑ECS input path (`sync_wizards`).

