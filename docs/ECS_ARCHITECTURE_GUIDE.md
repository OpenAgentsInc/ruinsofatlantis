# ECS Architecture & Contribution Contract

> **Scope.** This document defines how gameplay is expressed in our codebase: entities, components, systems, events, networking, and testing. It also lists **hard rules** (do’s/don’ts) so we scale cleanly to **thousands of NPCs** without regressions.

---

## 1) First Principles (non‑negotiable)

1. **Server‑authoritative.** All gameplay state (movement, AI, casts, damage, deaths, effects) updates on the server via the ECS schedule. The client renders only what’s replicated.
2. **Data‑driven, not hard‑coded.** Do **not** branch on story nouns (“Wizard”, “Zombie”, “Boss”). Use components + data/specs. *Wizard/Zombie/Boss* are **archetypes**, not `if` statements.
3. **Composition over inheritance.** Behavior emerges from *which components an entity has*, not from entity “classes.”
4. **Deterministic fixed‑step.** The server runs gameplay in a fixed time step with explicit system ordering.
5. **Linear‑time queries.** Use spatial indexing (grid) and interest culling so hot paths scale ~O(k), not O(N).
6. **Evented side‑effects.** Damage, explosions, status effects, despawns, etc., flow through **events** consumed by dedicated systems—never ad‑hoc mutation.
7. **Stable external IDs.** Every replicated entity has a stable `ActorId` on the wire. Internal entity handles may change; the ID does not.
8. **Strict layering.**

   * **Server (ECS)** owns truth.
   * **net_core/platform** frames commands & snapshots.
   * **client_core** applies snapshots into a replication buffer.
   * **renderer** visualizes replicated state; no gameplay logic.

---

## 2) Mental Model

```
World (ECS)
 ├── Entities (PC, NPCs, projectiles, destructibles…)
 ├── Components (Transform, Health, Faction, MoveSpeed, Melee, Projectile, Homing, Effects…)
 ├── Systems (fixed order: input → cooldowns → cast → ingest → AI → move → melee → homing → integrate → collide → AoE → faction → apply damage → cleanup)
 ├── Event buses (DamageEvent, ExplodeEvent, DeathEvent, HitFx[VFX])
 └── Services (SpatialGrid, FactionMatrix, Specs)
```

**Archetypes** (e.g., “Wizard”, “Skeleton”, “Boss‑X”) are *data entries* that attach a **bundle of components** on spawn. No system should special‑case an archetype name or enum; use components present.

---

## 3) Core Entity Kinds (by *role*, not class)

* **Actor**: any combat unit (PC or NPC). Must have: `Transform`, `Health`, `Radius`, `Faction`. Adds optional: `MoveSpeed`, `AggroRadius`, `AttackRadius`, `Melee`, `Spellbook`, `ResourcePool`, `Cooldowns`, `Stunned`, `Slow`, `Burning`, etc.
* **Projectile**: ephemeral entity with `Transform`, `Projectile{kind, ttl, owner}`, `Velocity`, optional `Homing`.
* **Destructible**: entity representing destructible world pieces (`Destructible`, `VoxelProxy`, etc.).
* **VFX/UI only**: purely visual handles spawned from replication (`HitFx`); **never** affect gameplay.

> **Rule:** Systems match on *components*, not names. E.g., the melee system runs on “anything that has `(Transform, AttackRadius, Melee)`,” regardless of archetype.

---

## 4) Components (baseline set & intent)

> Use small, copyable structs where possible. **Do not** stash behavior inside components.

**Always present on Actors**

* `Transform { pos: Vec3, yaw: f32 }`
* `Radius(f32)` — collision cylinder on XZ
* `Health { hp: i32, max: i32 }`
* `Faction(FactionId)` — **integer** or compact enum; *no logic keyed on specific faction names*
* `ArchetypeId(u16)` — data key for visuals/VO/UI; **no gameplay logic may branch on this**

**Frequently used**

* `MoveSpeed(f32)` — meters/sec
* `AggroRadius(f32)` — perception
* `AttackRadius(f32)` — melee reach
* `Melee { damage: i32, cooldown_s: f32, next_ready_t: f32 }`
* `Target(Option<Entity>)`
* `Spellbook { known: Vec<SpellId> }`
* `ResourcePool { mana: i32, max: i32, regen_per_s: f32 }`
* `Cooldowns { gcd_s: f32, gcd_ready: f32, per_spell: HashMap<SpellId, f32> }`
* Effects: `Burning { dps: i32, remaining_s: f32 }`, `Slow { mul: f32, remaining_s: f32 }`, `Stunned { remaining_s: f32 }`
* Lifecyle: `DespawnAfter { seconds: f32 }`

**Projectiles**

* `Projectile { kind: ProjKind, ttl_s: f32, owner: Option<Entity> }`
* `Velocity { v: Vec3 }`
* `Homing { target: Option<Entity>, turn_rate: f32, max_range_m: f32, reacquire: bool }`

**Intents (player/NPC input)**

* `IntentMove { dx: f32, dz: f32, run: bool }`
* `IntentAim { yaw: f32 }`

> **Do:** Add new features by adding components + a system that reads/writes them.
> **Don’t:** Extend enums and then switch on them in systems.

---

## 5) Systems & Fixed Order (server tick)

> **One place** defines order. If you change it, document why.

1. **input_apply_intents** — integrate `IntentMove/IntentAim` → `Transform`
2. **cooldown_and_mana_tick** — decay GCD & per‑spell, regen resources
3. **cast_system** — validate `Spellbook/Cooldowns/ResourcePool`; enqueue projectiles
4. **ingest_projectile_spawns** — turn spawn cmds into projectile entities
5. **effects_tick** — DoT/slow/stun timers; generate `DamageEvent`s
6. **spatial.rebuild** (until incremental updates are fully wired)
7. **ai_select_targets** — set/refresh `Target`
8. **ai_move** — use `MoveSpeed` toward `Target`
9. **melee_apply_when_contact** — on reach + ready → `DamageEvent`
10. **homing_acquire_targets** → **homing_update**
11. **projectile_integrate**
12. **projectile_collision** — segment vs cylinder; push `DamageEvent`/`ExplodeEvent`
13. **aoe_apply_explosions** — consume `ExplodeEvent` → `DamageEvent`s
14. **faction_on_damage** — update hostility matrix by rule (data‑driven)
15. **apply_damage** — mutate `Health`, emit Death & `DespawnAfter`
16. **cleanup** — despawn per timers; prune dead

**Invariants**

* Systems skip entities missing required components.
* **Self‑skip**: collision never hits owner.
* **Hostility**: use the **Faction matrix** (`hostile(a_team, b_team)`) only—never direct team constants.

---

## 6) Events (per‑tick buses)

* `DamageEvent { src: Option<Entity>, dst: Entity, amount: i32 }`
* `ExplodeEvent { center: Vec2, r2: f32, src: Option<Entity> }`
* `DeathEvent { entity: Entity }`
* **VFX**: `HitFx { kind: u8, pos: [f32;3] }` *(replicated to client; no gameplay effect)*

> **Rule:** Systems **emit** events; only the designated system **applies** the effect.
> Example: projectile collision **emits** `DamageEvent`; `apply_damage` **mutates** `Health`.

---

## 7) Data & Specs (no literals in systems)

* **Specs table** on the server (or `data_runtime/SpecDb`):

  * Spells: cost, gcd, cooldown, projectile kind, shot pattern
  * Projectiles: speed, ttl, damage, radius, explosion r, homing params
  * Effects: dps/duration/stacking rules
* Systems ask **Specs**; they do not embed constants.
  *If you need to tune a number, you change data—not code.*

---

## 8) Spatial Index (performance)

* Use a 2D XZ **uniform grid** service:

  * `update_on_move(id, old_pos, new_pos)` (incremental)
  * Queries:

    * `query_circle(center, r)` (AoE, perception)
    * `query_segment(a, b, pad)` (projectile broad‑phase)
* Systems never iterate **all** actors for proximity when the grid can be used.

---

## 9) Networking & Replication (server→client)

**Commands (client→server)**

* `Move { dx, dz, run }`, `Aim { yaw }`, `Cast { spell, pos, dir }`
  Rate‑limited server‑side; server enqueues casts.

**Snapshots (server→client)**

* **ActorSnapshotDelta v4** *(single path; no legacy fallbacks)*:

  * `spawns: Vec<ActorRep>` — `{ id, kind(u8), faction(u8), archetype_id(u16), name_id(u16), unique(u8), pos, yaw, radius, hp, max, alive }`
  * `updates: Vec<ActorDeltaRec>` — bitmask for `{pos, yaw, hp, alive}` (quantized)
  * `removals: Vec<u32>` — actor ids
  * `projectiles: Vec<ProjectileRep>` — ephemeral list every tick
  * `hits: Vec<HitFx>` — VFX only (no gameplay)
* **Quantization:** positions and yaw quantized; deltas carry only changed bits.
* **Interest management:** per‑client center/radius; only include nearby actors.

**HUD (server→client)**

* `HudStatus` includes PC mana/max, GCD remaining, per‑spell cooldowns, effect timers.
* `HudToast` conveys short, transient HUD messages by code. Defined codes:
  * `1 = Not enough mana` — client shows a brief red center text and suppresses local cast animation.

**Client apply path**

* `client_core::ReplicationBuffer` applies v3:

  * Stores `actors` + **derived** `views` (e.g., `npcs`, `wizards` are *derived subsets*, not authorities)
  * Stores `projectiles`, `hits`, `hud`

> **Rule:** The renderer may **only** read the replication buffer. No gameplay mutation on the client.

---

## 10) Rendering & Animation (client)

* Visuals created/updated/destroyed strictly by replicated **actor IDs**.
* Animation picks are **replication‑driven** (speed from Δpos, `alive` flag):

  * idle vs jog vs sprint by speed threshold
  * one‑shot death on `alive=false`, then hold
* Projectiles drawn from replicated list; Fireball VFX triggered on projectile **disappear/explode**; `HitFx` spawns “spark” bursts on direct hits.
* HUD renders from `HudStatus`; actor overhead health bars use replicated HP/positions, not client‑side overlays.

> **Never** do client‑side collisions, damage, AI, or “fake” hit particles with gameplay implications.

---

## 11) AI & Targeting

* Perception from `AggroRadius` + grid queries.
* Target selection writes `Target(Entity)`; **no “only chase PCs”** shortcuts—use the **Faction** matrix and distance.
* Behavior selection is data‑driven (cooldown availability, cluster density); systems may look at counts/geometry but not archetype names.

---

## 12) Combat & Casting

* Casting gated by **GCD**, **per‑spell cooldown**, **ResourcePool**; on accept:

  * debit cost, set timers, enqueue projectile spawns
* **Magic Missile**: homing (with reacquire if enabled)
* **Fireball**: AoE via `ExplodeEvent` either on proximity hit or TTL expire
* **Firebolt**: direct hit only
* Effects applied by the AoE or collision system according to **Specs** (e.g., MM applies Slow).

---

## 13) Scaling & Performance Expectations

* Spatial grid used in projectile & AoE systems (no O(N) scans).
* Interest culling limits replication.
* Systems avoid heap churn in tight loops (reuse scratch buffers).
* Fixed‑step schedule, deterministic given the same command stream + seed.
* Heavy work (meshing/colliders) runs off‑thread with budget; results committed at frame fences.

---

## 14) Testing Requirements (what we always cover)

**Server (unit/integration)**

* Casting gates: GCD, per‑spell cooldown, mana debit/regen.
* Projectile collisions: direct hit, arming delay, owner skip, wizard radius sanity.
* AoE explosions: overlap counts, friendly‑fire rules (via Faction), effect application.
* Melee cooldown & reach: damage ticks only on ready and within radius.
* Homing reacquire: target dies/leaves range → retarget within max range.
* Lifecycle: death emits despawn timer; entity removed after timer.
* Separation/physics sanity if applicable (e.g., undead separation).
* **Determinism**: same seed/inputs → same outputs.

**Networking**

* v3 encode/decode round‑trips for spawns/updates/removals/projectiles/hits.
* Quantization tolerance property tests (pos/yaw).

**Client**

* Replication applies HP‑only update to derived views.
* Projectiles appear from v3; disappear triggers FB explosion render hook.
* Animation mapping: Δpos/alive → idle/jog/death.

**CI Gates**

* `clippy -D warnings` on workspace
* Tests green on workspace
* Grep guards: no `legacy_client_`, `NpcListMsg`, `BossStatusMsg`, `ActorStore`

---

## 15) How to Add a Feature (checklist)

1. **Model it as data + components.**

   * Add/extend a spec entry (data_runtime or server `Specs`).
   * Add a component if state must persist on entities.
2. **Add a system** (or extend one) that reads/writes those components.
3. **Emit events** for side‑effects; consume them in the appropriate system.
4. **Replicate** only what the client needs to render (actors/projectiles/hitfx/hud).
5. **Tests**:

   * Unit test for the system logic.
   * Integration test that exercises the end‑to‑end tick.
   * Net round‑trip test if serialization changes.
6. **Docs**:

   * Update this guide (if you added a new component/event/system category).
7. **CI**:

   * Ensure clippy/tests pass; keep grep guards clean.

---

## 16) Anti‑Patterns (red lines)

* **No hard‑coded archetypes in logic.**
  Bad: `if kind == Wizard { ... }`
  Good: `if has::<Spellbook>() { ... }`
* **No client gameplay.**
  Bad: client collides projectiles against NPCs for damage/FX.
  Good: client spawns FX on replicated `HitFx` or projectile disappear.
* **No broad O(N) scans** in hot paths—use the grid.
* **No hidden global flips.** Use `Faction` matrix and log flips explicitly.
* **No “special boss” pathway.** Boss is an archetype bundle; only data differs.
* **No raw string names on the wire.** Use IDs (`archetype_id`, `name_id`) and map client‑side.
* **No panics** in normal gameplay paths. Return errors or clamp.

---

## 17) Observability

* Use `tracing` spans per system with counters:

  * `system={name} dur_ms=…`
  * `events.damage=…`, `events.explode=…`
  * `grid.candidates=…`, `actors.replicated=…`
* Dev toggles: `RA_LOG_CASTS`, `RA_LOG_SNAPSHOTS`, `RA_LOG_PROJECTILES` — **debug only**.

---

## 18) Versioning, Flags, & Cleanup

* **One** replication mode: **v3 delta** (spawns/updates/removals + projectiles + hits). No legacy fallbacks in runtime.
* Feature flags only for **optional dev tooling** (e.g., `vox_onepath_demo`). Gameplay features are not hidden behind flags.

---

## 19) Example Flows (reference)

**Cast → Projectile → Hit → Damage → Death → Despawn → Replicate**

1. Client sends `Cast(spell, pos, dir)`.
2. Server `cast_system` validates (GCD, cooldowns, mana) → enqueue spawn.
3. `ingest_projectile_spawns` creates projectile entity with specs.
4. `projectile_integrate` advances; `projectile_collision` tests segment vs actors (grid candidates).
5. On **direct hit**: emit `DamageEvent`; push `HitFx` for VFX; mark projectile for despawn.
6. On **AoE**: emit `ExplodeEvent` → `aoe_apply_explosions` → `DamageEvent`s; push `HitFx` entries as desired.
7. `apply_damage` mutates HP; emits `DeathEvent`, sets `DespawnAfter`.
8. `cleanup` removes projectiles and entities with expired timers.
9. platform builds v3 delta (with `hits`) → client applies → renderer shows FX/HUD.

---

## 20) Definitions of Done (per PR)

* No archetype‑specific branches in logic.
* New behavior expressed via components + systems.
* v3 encode/decode tests updated if schema changed.
* Unit tests + an integration test cover the change.
* CI green; clippy clean; grep guards clean.
* Brief doc addition (this file) if you added/changed a component or event contract.

---

### Appendix: Naming & Style

* **Components**: singular nouns (`Transform`, `MoveSpeed`, `Melee`, `Projectile`).
* **Systems**: verb‑phrases (`input_apply_intents`, `projectile_collision_ecs`).
* **Events**: past‑participle nouns (`DamageEvent`, `ExplodeEvent`).
* **IDs**: `ActorId(u32)`, `ArchetypeId(u16)`, `NameId(u16)`.
  Wire never carries free‑form names.

---

### Final Word

This contract is what keeps the codebase coherent as we scale content and contributors. If you’re about to add a branch keyed on “Wizard,” **stop** and express the behavior in components + data. If you’re about to scan all actors each frame, **stop** and add a grid query. If you’re about to make the client “just handle” something, **stop** and replicate a minimal event.

If a change cannot meet these constraints, write an ADR explaining the exception, the blast radius, and the plan to remove it.
