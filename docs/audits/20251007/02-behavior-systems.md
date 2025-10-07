Behavior Systems — What drives movement, melee, casting

Schedule orchestration
- `crates/server_core/src/ecs/schedule.rs:61` `Schedule::run()` order:
  - `cooldown_and_mana_tick` (present)
  - `boss_seek_and_integrate` (Nivita move toward nearest wizard)
  - `cast_system` (drains `pending_casts` — PC-only today)
  - `ingest_projectile_spawns` (translates pending projectiles to ECS)
  - `effects_tick` (burn/slow/stun)
  - `ai_move_undead_toward_wizards` (movement for Zombies only)
  - `melee_apply_when_contact` (melee for Zombies only)
  - `homing_acquire_targets`, `homing_update`
  - `projectile_integrate_ecs`, `projectile_collision_ecs`, `aoe_apply_explosions`
  - `apply_damage_to_ecs`, `cleanup`

Movement
- Zombies move toward nearest wizard: `ai_move_undead_toward_wizards()` filters by `ActorKind::Zombie`.
- Nivita movement is handled in a separate helper `boss_seek_and_integrate()`.
- No general “all hostiles with MoveSpeed/AggroRadius” movement system exists yet.

Melee
- `melee_apply_when_contact()` computes contact vs nearest wizard but only iterates actors filtered by `ActorKind::Zombie`.
- Result: Nivita has Melee components but never attacks (not included by filter).

Casting
- `cast_system()` drains `pending_casts` created by `ServerState::enqueue_cast()`; this is driven by player input and demo plumbing.
- No ECS AI system issues casts for NPC wizards (Team::Wizards). They never enqueue spells.

Projectiles & collisions
- `ingest_projectile_spawns()` and `projectile_integrate_ecs()` create/move ECS projectile entities.
- `projectile_collision_ecs()` handles direct hits and AoE, with arming delay and owner-skip. Hostility gating currently permissive for the demo.

Effects & lifecycle
- Effects tick (burn/slow/stun) and damage application push `DamageEvent` → HP changes; despawn timers handled in `cleanup`.

Gap vs ECS refactor plan (docs/issues/ecs_refactor_part_3.md)
- Missing: `input_apply_intents` before everything; platform still mirrors PC through `sync_wizards()`.
- Movement/melee should be generalized by components, not actor kind.
- NPC wizard casting system absent.

