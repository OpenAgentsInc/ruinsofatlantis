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
* **Arming delay**: projectiles ignore collisions for a brief `arming_delay_s` after spawn so they don’t detonate on the caster or immediately at the hand.
* **AoE radius**: planar XZ AoE uses the explosion radius plus a small pad and the target’s collision capsule radius (min 0.30 m) to avoid thin‑target edge misses.
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
* Important: the server excludes entities with a `Projectile` component from the `actors` list.
  Projectiles are carried only in the dedicated `projectiles` vector to prevent clients from
  creating NPC views from projectile replicas.
* **Quantization:** positions and yaw quantized; deltas carry only changed bits.
* **Interest management:** per‑client center/radius; only include nearby actors.

**HUD (server→client)**

* `HudStatus` includes PC mana/max, GCD remaining, per‑spell cooldowns, effect timers.
* `HudToast` conveys short, transient HUD messages by code. Defined codes:
  * `1 = Not enough mana` — client shows a brief red center text and suppresses local cast animation.

**Destructibles (server→client)**

* `DestructibleInstance` announces a destructible proxy by a stable DID and its world AABB.
* `ChunkMeshDelta` carries CPU mesh for a single chunk (`did`, `chunk=(x,y,z)`, positions/normals/indices).
* Clients must accept deltas only after the matching instance arrives; unknown DIDs are deferred. Duplicate deltas for the same DID+chunk are de‑duped.
* Presentation: once a client receives the first non‑empty chunk mesh for a DID, hide the static GLTF proxy for that DID (debug override: `RA_SHOW_STATIC_RUINS=1`).
 * Demo note: the prototype level registers exactly one server‑authoritative destructible ruin (DID=1), positioned on the ground slightly in front of the PC spawn. The renderer no longer spawns any static ruins; all visible ruins geometry comes from `ChunkMeshDelta` messages.

**Client apply path**

* `client_core::ReplicationBuffer` applies v4:

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

**Client‑side cast gating**

* The client gates local cast VFX and command emission using replicated HUD state (GCD, per‑spell cooldowns, and mana). If a cast would be rejected server‑side, the client does not spawn local VFX and shows the appropriate HUD toast instead. This keeps visuals consistent with server authority.

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

Implementation notes:

* Projectile spawn origin is derived from the caster’s transform (hand/chest offset) and sent as-is in world space. Do not terrain-clamp on the client; the server is authoritative over terrain/collision and resolves the true outcome.
* Collisions skip the owner and honor `arming_delay_s` to avoid immediate self‑detonation.

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
* Replication safety: unrelated frames may never be mis‑decoded as chunk meshes or spawns; deltas must pass validation.
  - Non‑delta frames (HUD, etc.) are not mis‑decoded as `ChunkMeshDelta`.
  - `ChunkMeshDelta` validation: sizes, indices in‑bounds, positions/normals finite, empty deltas allowed, AABB containment vs instance.
  - Invalid `DestructibleInstance` payload does not register DID nor flush deferred deltas; valid instance flushes.
  - Projectiles never create/modify NPC views (even with id collisions).

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

* **One** replication mode: **v4 delta** (spawns/updates/removals + projectiles + hits). No legacy fallbacks in runtime.
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
9. platform builds v4 delta (with `hits`) → client applies → renderer shows FX/HUD.

---

## 20) Definitions of Done (per PR)

* No archetype‑specific branches in logic.
* New behavior expressed via components + systems.
* v3 encode/decode tests updated if schema changed.
* Unit tests + an integration test cover the change.
* CI green; clippy clean; grep guards clean.
* Brief doc addition (this file) if you added/changed a component or event contract.

---

## 21) Destructibles (multi‑proxy pipeline)

The destructible system is server‑authoritative and multi‑proxy (many destructible instances per scene):

1. Broad‑phase: projectiles and explosions emit `CarveRequest`s via `Ctx.carves` when segment/AABB intersects a proxy (surface‑pick for explosions).
2. Apply: `destructible_apply_carves` converts WS→OS, scales radius, carves the grid, enqueues dirty chunks.
3. Mesh: `destructible_remesh_budgeted` consumes dirty chunks deterministically and produces CPU meshes; emits `ChunkMeshDelta` per chunk.
4. Colliders: `destructible_refresh_colliders` refreshes touched chunk colliders under a budget across ticks.
5. Replication: platform sends `DestructibleInstance` (once) and chunk deltas; client defers deltas until the instance is known.

Rules:

- Never run destructible work outside the ECS order. All mutation happens in systems.
- Surface‑pick explosions: carve only when an OS ray hits voxels near the AABB, and guard with a max carve distance.
- Chunk deltas must be validated client‑side (see §22). No “guessing” allowed.

Tests we always keep:

- Ray DDA correctness, carve budget determinism, queue ordering.
- Registry AABB broad‑phase true/false.
- Explosions: surface‑pick required and distance guard.
- Collider refresh drained by `touched_this_tick` under budget.

---

## 22) Replication Hardening & Message Parsing Order

To prevent mis‑decoding and protect the renderer:

- Outer framing may carry any payload. Decoders must be tried in the same order the server encodes:
  1) `ActorSnapshotDelta` (v4)
  2) `DestructibleInstance` (validate AABB extents)
  3) `ChunkMeshDelta` (validate shape + AABB containment if instance known)
  4) `HudStatusMsg`, `HudToastMsg`
- Client stores destructible instance AABBs and rejects deltas whose bbox falls outside the AABB (±epsilon).
- Deltas must have `positions.len()==normals.len()`, `indices.len()%3==0`, and pass size caps.
- All indices are in-bounds: `max(indices) < positions.len()`.
- All vertex data are finite: reject any `NaN`/`Inf` coordinates or normals.
- Long‑term: prefer a one‑byte type tag per frame (TODO: ADR if we adopt tags) to remove guess‑decoding entirely.

Never couple projectiles to NPC views:

- `ActorSnapshotDelta.projectiles` is presentation‑only and must not create or mutate NPC views. Tests enforce that only actor spawns/updates drive views.
- Projectile–Actor ID collisions are allowed; clients must still never create or mutate actor views from `projectiles`.

---

## 23) Overlays & Bars (client)

- Overhead health bars draw from replicated **wizard** views (positions + HP). Do not pull from local ECS state.
- Distance cull NPC bars (default 25 m; env `RA_NPC_BAR_RADIUS` overrides). PC bar always shows.
- Bars are screen‑space quads anchored to head `(pos + y≈1.7)`; ensure culling to prevent “green bands” in large crowds.

---

## 24) Boss (Nivita/Death Knight) HUD & Model Updates

- `boss_status` derives from `ActorSnapshotDelta` by scanning actors with `kind=Boss` and `unique=1` (prefer unique, fall back to any boss if unique absent).
- Renderer uses `boss_status` to update the DK banner and snap the DK model to the replicated position (terrain‑aware).
- No special boss gameplay pathway on client; all combat is server‑side.

---

## 25) Casting Inputs, Spawn Origin & VFX

- Client sends `Cast { pos, dir }`. Server validates GCD/cooldowns/mana, enqueues projectile spawns.
- PC local VFX may be spawned immediately for responsiveness, but server remains authoritative for damage/hits.
- Spawn origins should be derived from the actor’s hand/world transform (not camera rays), then clamped above terrain.

---

## 26) Spatial Grid & Collision Shape Floors

- Use `SpatialGrid.query_segment(a,b,pad)` for projectile broad‑phase; do not scan all actors.
- Always enforce a **minimum collision radius** (e.g., 0.3 m) on queries to prevent “thin target” misses at very low HP or skinny capsules.
- AoE tests should consider 3D distance with a small radius pad.

---

## 27) Destructibles – Agent Playbook

- Add a new proxy: register `DestructibleProxy` with world AABB; ensure instance is replicated before deltas.
- Carve logic: WS→OS center conversion; uniform‑scale radius; retain requests if proxy not yet registered.
- Mesh/colliders: budgeted, stable order; enqueue only touched chunks for colliders; process across ticks.
- Replication: instances once; deltas per changed chunk; client defers deltas until instance known; deltas validated.
- Tests: add CPU‑only tests for DDA, queues, carve budgeting, collider drain, registry broad‑phase.

---

## 28) System Names & Order (reference)

These names are asserted in tests. Changing them requires updating tests and documenting the rationale.

1. `input_apply_intents`
2. `cooldown_and_mana_tick`
3. `ai_caster_cast_and_face`
4. `cast_system`
5. `ingest_projectile_spawns`
6. `spatial.rebuild`
7. `effects_tick`
8. `ai_move_hostiles`
9. `separate_undead`
10. `melee_apply_when_contact`
11. `homing_acquire_targets`
12. `homing_update`
13. `projectile_integrate_ecs`
14. `projectile_collision_ecs`
15. `destructible_from_projectiles`
16. `destructible_from_explosions`
17. `destructible_apply_carves`
18. `destructible_remesh_budgeted`
19. `destructible_refresh_colliders`
20. `aoe_apply_explosions`
21. `faction_flip_on_pc_hits_wizards`
22. `apply_damage_to_ecs`
23. `cleanup`

---

## 29) Dev Env & Toggles

- `RA_NPC_BAR_RADIUS` — meters for NPC wizard bar visibility (default 25.0).
- `RA_LOG_CASTS`, `RA_LOG_TICK`, `RA_LOG_SNAPSHOTS`, `RA_LOG_PROJECTILES` — development logging only.
- Feature flags are only for optional demos (e.g., `vox_onepath_demo`). Gameplay is not feature‑gated.

---

## 30) Agent Do/Don’t Cheat‑Sheet

Do:

- Add components + systems; emit/consume events; keep schedule order explicit.
- Use the spatial grid for proximity/broad‑phase.
- Validate replication inputs client‑side (AABBs, sizes); defer deltas until instances are known.
- Keep the client presentation‑only; derive views from actor deltas; update overlays from replication.

Don’t:

- Branch on archetypes in server logic.
- Add client collisions/damage/AI.
- Upload chunk meshes before `DestructibleInstance` is known; or accept deltas outside instance AABBs.
- Create NPC views from `projectiles` — projectiles are separate visuals.

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

---

## 31) Determinism & RNG (server)

- All randomness is seeded (world seed + tick + entity id). No wall‑clock or OS entropy in systems.
- Gameplay never reads `Instant::now()`; use tick counters and `dt` throughout.
- Avoid non‑deterministic math in hot loops; prefer stable approximations if needed.
- Tests: same inputs/seed → identical HP totals, despawn sets, and event counts.

## 32) Framing & Type Tags (wire)

- Every frame on the wire carries a one‑byte type tag:
  - `0x01 = ActorSnapshotDelta(v4)`
  - `0x02 = DestructibleInstance`
  - `0x03 = ChunkMeshDelta`
  - `0x10 = HudStatusMsg`
  - `0x11 = HudToastMsg`
- Clients dispatch by tag; unknown tags are ignored. If tags are negotiated off, strict decode order + validation in §22 applies.
- Tests: randomized bytes never panic; mis‑tagged frames do not leak into other decoders.

## 33) Budgets, Back‑Pressure & Drops

- Expensive systems run under explicit per‑tick budgets (meshing, colliders, carve bus).
- Use FIFO within a DID and round‑robin across DIDs; increment metrics on drops (`destruct.*`).
- Tests: queues drain over ticks; drops counted; order stable and deterministic.

## 34) Security & Rate‑Limits (commands)

- Server validates and rate‑limits all `Cast`/`Move`; rejects on cooldown/mana with a HUD toast.
- Projectiles enforce an `arming_delay_s` to avoid immediate self‑hits.
- Tests: cast bursts throttle; cooldown rejection produces toast; owner cannot be hit during arming window.
