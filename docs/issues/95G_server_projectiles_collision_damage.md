# 95G — Server Systems: Projectiles, Collision, Damage

Labels: ecs, server-authoritative, combat
Depends on: Epic #95, 95D (Projectiles SpecDb), 95C (Components)

Intent
- Implement an authoritative projectile → collision → damage pipeline. Integrate with carve by emitting `CarveRequest` on destructible hits.

Outcomes
- Server integrates `Projectile` entities per tick, resolves collisions against `CollisionShape`/destructibles, applies damage to `Health`, and enqueues `CarveRequest` for destructible impacts.

Files
- `crates/server_core/src/systems/projectiles.rs` (new)
- `crates/server_core/src/systems/collision.rs` (new)
- `crates/server_core/src/systems/damage.rs` (new)
- `crates/server_core/src/tick.rs` (insert ordering)
- `crates/data_runtime/src/specs/projectiles.rs` (from 95D)

Components
- `Projectile { kind: ProjectileId, speed_mps: f32, radius_m: f32, damage: i32, owner: EntityId, life_s: f32 }`
- `CollisionShape { kind: Sphere/Capsule/AABB, params }`
- `Health { hp, max }`, `Team { id }`

Systems
- `ProjectileIntegrateSystem` — fixed dt; update positions and produce segment [p0,p1] per tick; cull by `life_s<=0`.
- `CollisionSystem` — broadphase grid or simple O(n·m) first pass; test projectile spheres against destructible AABBs and entity `CollisionShape`; emit `HitEvent`s; for destructible hits, compute segment–AABB entry t and emit `CarveRequest` with `did`.
- `DamageApplySystem` — apply to `Health`; spawn death events.

Data Wiring
- Load projectile params from `data_runtime::specs::projectiles` (id→speed/radius/damage/life). No hard‑coded constants.

Tests
- Build a tiny world with one projectile and a capsule/sphere target; assert hit, hp decreases deterministically.
- With a destructible AABB target, assert a `CarveRequest` is emitted with a valid center/radius from spec.

Acceptance
- Server tick produces deterministic hits and damage based on SpecDb; destructible hits enqueue `CarveRequest` for 95E.
