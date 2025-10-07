Inventory — NPC ECS Registration (where/how they are created)

Zombies
- Server spawn: `crates/server_core/src/lib.rs:454` `spawn_undead(pos, radius, hp)`
  - Components set: `MoveSpeed`, `AggroRadius`, `AttackRadius`, `Melee`.
  - Kind/team: `ActorKind::Zombie`, `Team::Undead`.

Nivita (Unique Boss)
- Server spawn: `crates/server_core/src/lib.rs:481` `spawn_nivita_unique(pos)`
  - Kind/team: `ActorKind::Boss`, `Team::Undead`.
  - Components set: `MoveSpeed`, `AggroRadius`, `AttackRadius`, `Melee`.
  - Boss stats snapshot for HUD: `nivita_stats` and `nivita_status()`.
  - Movement helper: `crates/server_core/src/systems/boss.rs:9` `boss_seek_and_integrate()`.

NPC Wizards (Team::Wizards)
- Mirrored from platform: `crates/server_core/src/lib.rs:258` `sync_wizards(&[Vec3])`
  - Ensures PC (Team::Pc) exists; spawns/positions NPC wizards for extra entries.
  - No ECS attack AI attached to wizards; no casting system enqueues their spells.

Death Knight
- Renderer visuals present: `crates/render_wgpu/src/gfx/deathknight.rs`, referenced in renderer init.
- No ECS registration on server. Not spawned by server; no behavior.

Replication
- Actors: `crates/server_core/src/lib.rs:632` `tick_snapshot_actors()` builds actor list with `ActorKind::{Wizard,Zombie,Boss}`.
- Projectiles: included from ECS entities with `Projectile`+`Velocity` into `ProjectileRep`.
- Platform delta (v3) path used; client decodes to replication buffer.

