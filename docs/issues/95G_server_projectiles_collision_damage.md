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
 - Reference current client usage to replace later:
   - `crates/render_wgpu/src/gfx/renderer/update.rs`:
     - Projectile spawn: `spawn_fireball`, `spawn_firebolt`, `spawn_magic_missile`
     - Projectile integrate and collision against NPCs (zombies/DK) and destructibles (selection call)

Components
- `Projectile { kind: ProjectileId, speed_mps: f32, radius_m: f32, damage: i32, owner: EntityId, life_s: f32 }`
- `CollisionShape { kind: Sphere/Capsule/AABB, params }`
- `Health { hp, max }`, `Team { id }`

Systems
- `ProjectileIntegrateSystem` — fixed dt; update positions and produce segment [p0,p1] per tick; cull by `life_s<=0`.
- `CollisionSystem` — broadphase grid or simple O(n·m) first pass; test projectile spheres against destructible AABBs and entity `CollisionShape`; emit `HitEvent`s; for destructible hits, compute segment–AABB entry t and emit `CarveRequest` with `did`.
- `DamageApplySystem` — apply to `Health`; spawn death events.
 - Optional: `AggroSystem` to set wizard hostility flags server‑side (replacing client‑side toggles in renderer).

Data Wiring
- Load projectile params from `data_runtime::specs::projectiles` (id→speed/radius/damage/life). No hard‑coded constants.
 - Replace hard‑coded spell mappings in renderer (e.g., firebolt/fireball) with ids used by server when emitting projectiles.

Tests
- Build a tiny world with one projectile and a capsule/sphere target; assert hit, hp decreases deterministically.
- With a destructible AABB target, assert a `CarveRequest` is emitted with a valid center/radius from spec.

Acceptance
- Server tick produces deterministic hits and damage based on SpecDb; destructible hits enqueue `CarveRequest` for 95E.
- Client still renders projectile visuals (prediction ok), and reconciles on hits once replication arrives.

---

## Addendum — Implementation Summary (95G partial)

- server_core::systems
  - Added `systems/projectiles.rs` with:
    - `integrate(projectiles, dt)` returning segments.
    - `collide_and_damage(..)` testing sphere hits for entities and AABB hits for destructibles, emitting `CarveRequest`.
  - Unit tests:
    - Sphere target loses hp deterministically when crossed by a segment.
    - Destructible AABB emits a `CarveRequest` with expected `did` and radius.
- ecs_core::components extended with:
  - `Projectile`, `CollisionShape`, `Health`, and `Team` components.
- Data specs for projectiles (95D portion) are TBD; current tests embed projectile params directly.
Status: PARTIAL (ProjectileIntegrate + simple Collision + Damage landed with tests; data specs TBD)
