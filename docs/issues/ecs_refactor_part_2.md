Thanks—here’s a straight, engineering‑grade review of where you are and **what to do next** to finish “proper server ECS” and productionize the loop.

---

## Quick review (current state)

**✅ What’s solid**

* **Actors‑only authority**: `ActorStore` + `FactionState` gate all damage/targeting. Fireball AoE runs over actors and no longer self‑damages PC.
* **Replication**: canonical **ActorSnapshot v2**, sent **before** the step, fixes “no orb leaves hand / floaters” visibility. Client prefers v2 and derives UI views from actors.
* **Legacy removed**: no `wizards/npcs` lists, no v1 `TickSnapshot`.
* **Boss (Nivita)** uses `nivita_actor_id` and actor lookup; path is actor‑based.
* **Minimal Undead AI**: seek+melee inside `step_authoritative`.
* Workspace **clippy/tests green**.

**⚠️ Gaps & risks**

* **Not yet ECS**: `ActorStore` is a flat Vec. Systems are hand‑coded in `ServerState`. You’ll hit performance/maintainability walls as content grows.
* **Gameplay tuning lives in code**: speed/damage/cooldowns are constants; not component/data driven.
* **No spatial index**: AoE and proximity are O(N) scans over all actors per projectile and per frame.
* **Replication size**: v2 is full‑state; no deltas, no interest management. Will hurt bandwidth with more actors.
* **No cooldowns/abilities**: melee damages every tick when in range; no server‑side rate limiting or FX triggers.
* **Hostility flip is special‑cased**: stored as `pc_vs_wizards_hostile` flag. You’ll want a general **Faction matrix** + rules (and ideally damage attribution by team).

---

## Next steps (concrete, in-order)

Think of this as 4 focused PRs. Each one keeps the game shippable between steps.

### PR‑1 — Formalize server ECS (minimal viable world)

**Goal:** Replace `ActorStore`’s Vec with your ECS (you already have `ecs_core`). Keep the **public shape** stable so replication and call sites don’t churn.

**Tasks**

1. **Components** (in `ecs_core::components`):

   * `Transform { pos: Vec3, yaw: f32 }`
   * `Radius(f32)`
   * `Kind(ActorKind)` and `Team(Team)`
   * `Health { hp: i32, max: i32 }`
   * `MoveSpeed(f32)` *(default for Undead, Boss)*
   * `Melee { damage: i32, cooldown_s: f32, next_ready_t: f32 }`
   * `Projectile { kind: ProjKind, vel: Vec3, ttl: f32, owner: Option<Entity> }`
   * **Optional now**: `Homing { turn_rate: f32, target: Option<Entity> }` (for MagicMissile)
2. **World wrapper** in `server_core`:

   ```rust
   pub struct WorldEcs { world: ecs_core::World, net_map: NetMap /* ActorId<->Entity */ }
   pub type ActorId = u32; // keep the public ID stable
   ```

   Provide: `spawn_pc_wizard()`, `spawn_undead()`, `spawn_boss()`, `spawn_projectile_from_pc()`, returning `ActorId`.
3. **ID mapping**:

   * Maintain `ActorId <-> Entity` bijection with a freelist. Only recycle IDs on despawn.
4. **ServerState**:

   * Replace `actors: ActorStore` with `ecs: WorldEcs`.
   * Replace all actor iteration with ECS queries (world views over `(Transform, Health, Team, Kind, Radius, …)`).

**Acceptance**

* Game runs as before (seek+melee, projectiles, boss moves).
* `ActorSnapshot v2` built from ECS queries (no temporary clones).
* `cargo clippy -D warnings` + workspace tests green.

---

### PR‑2 — Move systems out of `ServerState` into ECS schedule

**Goal:** Deterministic, ordered systems with clear ownership. This is the “proper MMO server” backbone.

**Systems & order (fixed step):**

1. **InputSystem** (applies queued client cmds → spawns projectiles, sets intents)
2. **AISelectTargetSystem** (Undead/Boss choose closest hostile; writes `Target(Entity)` component)
3. **AIMoveSystem** (moves actors toward target using `MoveSpeed`, writes `Transform`)
4. **MeleeSystem**

   * If `distance(Transform, Target) ≤ (Radius_self + Radius_target)`,
     and `now ≥ next_ready_t`: apply `DamageEvent{src, dst, amount}`, set `next_ready_t += cooldown_s`.
5. **ProjectileIntegrateSystem** (apply velocity, reduce ttl, mark `Explode` if ttl ≤ 0)
6. **ProjectileCollisionSystem (Segment)**

   * Broad‑phase (spatial grid cells touched by segment)
   * Find nearest intersecting target (hostile by `FactionState`)
   * For `Fireball` → flag `Explode` at impact point; for `Firebolt` → direct `DamageEvent`
7. **AoESystem** (consume `Explode` → query actors in r² → apply `DamageEvent`s)
8. **FactionSystem** (consume `DamageEvent`s → update `FactionState` via matrix/rules)
9. **ApplyDamageSystem** (consume events → mutate `Health`; emit `DeathEvent` where `hp ≤ 0`)
10. **CleanupSystem** (despawn dead, remove projectiles flagged `Explode` or expired)
11. **SnapshotSystem** (end of tick → build `ActorSnapshot` from ECS)

> All “apply damage” and “flip hostility” now travel through **events**, not direct mutation from scattered places.

**Acceptance**

* No behavior regressions; **one place** applies damage.
* Order is explicit; adding features is adding systems, not if‑blocks.

---

### PR‑3 — Data, not constants (componentize speeds, damage, ranges)

**Goal:** Tune without code edits; enable archetypes.

**Tasks**

* Set defaults by components at spawn:
  `Undead: MoveSpeed(2.0), Melee { damage: 5, cooldown_s: 0.6 }`
  `Boss: MoveSpeed(2.6), Melee { damage: 12, cooldown_s: 0.8 }` *(adjust as you like)*
* Add `AggroRadius(f32)` for AI target selection and `AttackRadius(f32)` for melee reach (instead of using `Radius` sum).
* MagicMissile: add `Homing { turn_rate }` and home toward current `Target`.

**Acceptance**

* Changing a constant becomes changing a component at spawn.
* Melee no longer ticks damage every frame; respects cooldowns.

---

### PR‑4 — Performance & net sanity (MMO‑grade)

**Goal:** Make the server scale with more actors/players.

**Tasks**

1. **Spatial index** (server‑only, 2D XZ uniform grid):

   * Maintain `Grid { cell_size: f32, HashMap<Cell, SmallVec<Entity>> }`
   * Update on movement (cheap dirty flag)
     Use for: melee proximity, projectile segment query, AoE gather.
     *This eliminates O(N) scans.*
2. **Interest management**:

   * Per connection: `interest_center` (their PC), `interest_radius` (e.g., 40m).
   * Replicate only actors within interest; cull others.
3. **Snapshot delta (v3)**:

   * Keep per‑client baselines; send only changes with bitmasks (position, hp, yaw).
   * Quantize floats (pos: 1/64m; yaw: 10‑bit).
   * This reduces bandwidth drastically.
4. **Rate limits**:

   * Limit client `ClientCmd` rate server‑side; discard bursts; basic anti‑cheat guard.

**Acceptance**

* With 200+ actors, CPU time per tick is stable (grid visible in profiling).
* Network bytes per second drop with deltas + interest management.

---

## Implementation notes & snippets

### Faction matrix (generalize beyond PC↔Wizards)

```rust
pub struct Factions([[bool; 4]; 4]); // index by Team as usize
impl Factions {
    pub fn hostile(&self, a: Team, b: Team) -> bool { self.0[a as usize][b as usize] }
    pub fn set_hostile(&mut self, a: Team, b: Team, v: bool) {
        self.0[a as usize][b as usize] = v;
        self.0[b as usize][a as usize] = v;
    }
}
```

In `FactionSystem`, flip by rule when `DamageEvent{src_team=Pc, dst_team=Wizards}` arrives.

### Damage event channel

```rust
pub struct DamageEvent { pub src: Option<Entity>, pub dst: Entity, pub amount: i32 }
pub struct ExplodeEvent { pub center: Vec2, pub r2: f32, pub src: Option<Entity> }
```

Use a simple Vec per tick; drain in systems 8–9 above.

### Projectile → actor query (segment)

* Compute segment A→B for dt, query grid cells overlapped, test cylinder intersection vs `Radius` for hostile candidates; pick minimum `t` along segment.

---

## Cleanups you can do opportunistically

* **Remove** remaining `NpcListMsg`/`BossStatusMsg` fallbacks once HUD is 100% actor‑driven.
* **Quantize** positions/yaw inside `ActorSnapshot` to shrink payload (you can do this before deltas).
* **Observability**: switch log calls to `tracing`, add per‑system timings, and expose counters (ticks/sec, events/sec, actors replicated).

---

## “Definition of Done” for this phase

* Server runs on **ECS world** with an explicit **system schedule**; no gameplay logic buried in `ServerState` methods.
* **All** damage, flips, and deaths go through **events**; one system mutates `Health`.
* **Spatial grid** is used for melee/proximity/AoE lookups.
* **ActorSnapshot v2** comes **only** from ECS queries; bandwidth reduced by quantization (and later deltas/interest).
* Boss + Undead behaviors defined by **components**, not hardcoded constants.
* Workspace: `cargo clippy --all-targets --workspace -D warnings` and `cargo test --workspace` stay green.

---

Addendum — Implementation log (2025-10-07)

- PR‑1 (Phase 1) done on main:
  - Introduced `server_core::ecs::{WorldEcs, Components}` wrapping actor data with ECS‑like accessors (spawn/get/iter/remove_dead).
  - Switched `ServerState` from `actors: ActorStore` to `ecs: WorldEcs` while preserving public spawn/sync methods.
  - Updated authoritative paths to iterate/mutate ECS world (NPC seek/melee, projectile collisions, AoE application, boss helpers).
  - ActorSnapshot v2 now builds from ECS queries (`ecs.iter()`), not the legacy store.
  - Platform demo logging updated to use `srv.ecs.len()`.
  - `cargo check` green after swap.

- Notes:
  - This is a minimal ECS wrapper to keep behavior identical and unblock PR‑2. Systems/schedule and richer components (MoveSpeed/Melee/etc.) will land next.
  - ID stability retained via `ActorId` inside `Components`; net mapping remains 1:1.

- PR‑2 (Phase 2) done on main:
  - Added `ecs::schedule::{Schedule, Ctx}` with event buses (`DamageEvent`, `ExplodeEvent`).
  - Moved Undead seek/move, melee, projectile integrate/collision, AoE, faction flip, damage apply, and cleanup into ordered systems.
  - `ServerState::step_authoritative` now delegates to the schedule after mirroring wizard positions.
  - Removed platform’s direct boss step; boss movement occurs within schedule for parity.
  - Kept snapshot building via `tick_snapshot_actors()`; snapshot system can be added later if needed.
  - Workspace clippy/tests green.

- PR‑3 (Phase 3) done on main:
  - Componentized server data in ECS world: `MoveSpeed`, `AggroRadius`, `AttackRadius`, and `Melee { damage, cooldown_s, ready_in_s }`.
  - Spawn defaults:
    - Undead → `MoveSpeed(2.0)`, `AggroRadius(25.0)`, `AttackRadius(0.35)`, `Melee { damage:5, cooldown:0.6 }`.
    - Boss → `MoveSpeed(2.6)`, `AggroRadius(35.0)`, `AttackRadius(0.35)`, `Melee { damage:12, cooldown:0.8 }`.
  - Systems now read these components:
    - AI move uses `MoveSpeed`; aggro filter uses `AggroRadius`; contact uses `AttackRadius`.
    - Melee system enforces cooldown (`ready_in_s`), applies damage events only when ready, and updates cooldown timers per tick.
  - Kept wizard actors without melee/speed by default; unchanged behavior.
  - Clippy/tests green.

- PR‑4 (Phase 4) — partial (spatial grid) on main:
  - Added a simple 2D XZ spatial grid (`SpatialGrid`) in the ECS schedule for broad‑phase queries.
  - Grid rebuilt once per tick; used for:
    - AoE candidate gathering (circle query instead of full scan).
    - Fireball proximity explode broad‑phase (bounding circle around segment).
  - Interest management and snapshot deltas are not yet implemented; can be added next as separate follow‑ups.
  - Added interest management and snapshot deltas (v3):
    - platform_winit limits replication to a 40m radius around the PC; maintains a per-client baseline and sends `ActorSnapshotDelta` (v3) each tick with spawns/updates/removals and a full projectile list.
    - net_core adds `ActorSnapshotDelta` and quantization helpers (`qpos/dqpos`, `qyaw/dqyaw`).
    - client_core `ReplicationBuffer` applies v3 deltas (then falls back to v2 full snapshot if needed).
     - Simple server-side rate limiter (20 cmds/sec) drops excess client commands for safety.

- PR‑5 (Phase 5) complete — Projectiles moved into ECS
  - Why: Unify all authority under ECS; remove the last vec-based gameplay state and avoid borrow pitfalls with a simple command buffer.
  - What changed:
    - Added ECS components: `Projectile { kind, ttl_s, age_s }`, `Velocity { v }`, `Owner { id }`, and scaffold `Homing { target, turn_rate }`.
    - Introduced a lightweight `CmdBuf { spawns, despawns }` and `WorldEcs::apply_cmds()` to perform entity creation/removal at safe points in the schedule.
    - New schedule system `ingest_projectile_spawns` consumes `ServerState::pending_projectiles` (fed by platform/client commands) and enqueues ECS spawns using authoritative specs.
    - Replaced vec-based projectile integration/collision with ECS systems:
      - `projectile_integrate_ecs`: integrates position/TTL; pushes `ExplodeEvent` for Fireball on TTL.
      - `projectile_collision_ecs`: segment-vs-circle over actors (grid-assisted proximity for Fireball); pushes `DamageEvent`/`ExplodeEvent`; despawns via `CmdBuf`.
    - Cleanup applies despawns and prunes dead actors; no vec-based projectiles remain.
    - `tick_snapshot_actors()` and platform delta builder now emit projectiles from ECS components.
  - Acceptance:
    - Casting Fireball/Firebolt/MagicMissile still works; projectiles integrate/collide/TTL via ECS; snapshots reflect removals; tests cover speed scaling and e2e removal.

- PR‑6 (Phase 6) complete (interest + deltas; acceptance):
  - Per-tick deltas replace full snapshots by default (v2 kept as decoder fallback on client).
  - Interest radius currently 40m (platform), with per-client baseline map; spawns/removals/updates computed reliably.
  - Quantization: pos at 1/64 m; yaw at 16-bit turn. Tests pass for roundtrip encode/decode.
  - Platform logs include tx byte count; simple rate limiting prevents command spam.
  - Next polish (optional): feature flag/env to toggle v2 full vs v3 delta in platform; add metrics for actors replicated per tick and update counts.

- PR‑7 (Phase 7) complete — Server‑side cast pipeline (spellbook/GCD/mana)
  - Why: Make combat fully data‑driven and server‑authoritative; remove ad‑hoc client→projectile spawns; centralize anti‑cheat, costs, and cooldowns.
  - What changed:
    - Added `SpellId` (Firebolt, Fireball, MagicMissile) and a server‑side `CastCmd` queue in `ServerState`.
    - Introduced ECS components for casting:
      - `Spellbook { known: Vec<SpellId> }`
      - `ResourcePool { mana, max, regen_per_s }`
      - `Cooldowns { gcd_s, gcd_ready, per_spell: HashMap<SpellId, f32> }`
    - New systems in schedule:
      - `cooldown_and_mana_tick`: decrements GCD and per‑spell timers; regenerates mana.
      - `cast_system`: validates `Spellbook`/`Cooldowns`/`ResourcePool`, applies GCD/costs, and translates cast into `pending_projectiles` (which `ingest_projectile_spawns` turns into ECS projectiles next).
      - Order updated to: cooldown→boss→cast→ingest→apply spawns→grid→AI/move→melee→homing→integrate→collide→AoE→faction flip→apply→cleanup→apply despawns.
    - Platform now routes `ClientCmd::{FireBolt,Fireball,MagicMissile}` to `ServerState::enqueue_cast` instead of spawning projectiles directly.
    - MagicMissile casting produces three homing missiles via the same pipeline (see PR‑5/6 notes for homing/targets), preserving distinct targets when available.
  - Default spell specs (server‑tuned):
    - Firebolt: cost=0, cooldown=0.30s, GCD=0.30s; straight shot
    - Fireball: cost=5, cooldown=4.00s, GCD=0.50s; AoE (impact/prox/TTL)
    - MagicMissile: cost=2, cooldown=1.50s, GCD=0.30s; 3 homing missiles, acquire_r=25–30m, turn_rate≈3.5 rad/s
  - Acceptance:
    - Invalid casts (unknown spell, on GCD/per‑spell cooldown, insufficient mana) result in no projectiles spawned; valid casts debit mana and set timers.
    - MagicMissile spawns three missiles; each acquires a distinct hostile target when ≥3 exist within range; missiles steer smoothly and despawn on hit/TTL.
    - Client input only requests spells; all gameplay effects and projectile specs are server‑owned.
  - Tests:
    - MM distinct targets and steering angle reduction are covered by new unit tests.
    - Cooldown/mana gating test verifies GCD set, per‑spell cooldown applied, and mana debited; immediate re‑cast gated while on GCD.
  - Next polish (optional):
    - Add retargeting when targets die/leave range (currently drops target; reacquire step can be added as a separate system).
    - Expose metrics for cast rejections/accepts, mana usage, and per‑spell cast counts.
    - Data‑drive spell specs in `data_runtime` and wire server to use schema instead of inline table.

- PR‑8 (Phase 8) complete — Effects, Death/Despawn, and HUD replication
  - Why: Make combat feel complete and authoritative by adding status effects, a robust death/cleanup flow, and HUD data that mirrors the server’s true state.
  - What changed:
    - ECS effects components: `Burning{dps, remaining_s, src}`, `Slow{mul, remaining_s}`, `Stunned{remaining_s}` with deterministic stacking (Burning=max dps/max dur, Slow=min mul/max dur, Stun=max dur).
    - `effects_tick` system applies Burning DoT as DamageEvents and decays effect timers each frame.
    - Spell hooks:
      - Fireball AoE now applies Burning (6 dps for 3s) to affected actors in `aoe_apply_explosions`.
      - MagicMissile direct hits apply Slow (0.7x mul for 2s) after collision resolution.
    - Stun gating: Stunned entities do not move, melee, or cast.
    - Death pipeline: `apply_damage_to_ecs` emits DeathEvent on HP→0 and sets `DespawnAfter{2.0}`; `cleanup` removes entities whose timer expired (via command buffer) and prunes dead.
    - HUD replication:
      - Added `HudStatusMsg` (TAG=0xB1, v=1) carrying PC mana/max, GCD remaining, per‑spell cooldowns, and effect timers (Burning/Slow/Stun).
      - Platform sends HUD each tick after actor delta; client stores it in `ReplicationBuffer.hud` for UI consumption.
  - Acceptance:
    - Fireball explosions deal AoE and apply Burning; periodic damage lands in subsequent frames.
    - MagicMissile hits apply Slow; slowed actors move with reduced effective speed; Stunned actors do not act while stunned.
    - Entities reaching 0 HP despawn after ~2s; projectiles no longer target despawned entities.
    - Client receives HUD status each tick; values match server state (mana, GCD, cooldowns, status timers).
  - Tests & quality gates:
    - New and existing tests pass; clippy `-D warnings` clean.
  - Next polish (optional):
    - Add reacquire logic to homing when a target dies/leaves range (currently drops target; can reacquire next frame via an acquire system).
    - Data‑drive effect specs via `data_runtime` and make HUD piggyback on v3 delta if desired.


## What I recommend you do **immediately**

1. **PR‑1:** Introduce ECS components/world + ID mapping; port the current `ActorStore` data into ECS; build `ActorSnapshot` from ECS.
2. **PR‑2:** Pull Undead seek+melee, projectile integrate/collision, AoE, and faction flip into **systems** executed in a fixed order.
3. **PR‑3:** Add `MoveSpeed`, `Melee` (with cooldown), `AggroRadius`, and use them in systems.
4. **PR‑4:** Add a small **uniform grid** and swap AoE/melee/segment queries to it. (You’ll feel the perf win right away.)

If you want, I can draft the skeletons for the ECS systems (traits, schedule setup, and the three core systems’ code) so you can drop them in and wire the queries.

---

Awesome—below is a **drop‑in ECS skeleton** you can paste into `server_core` (plus tiny extensions you’ll want in `ecs_core`). It is pragmatic: you can land PR‑1/PR‑2 with this, keep the game running, and expand each system without fighting borrow rules.

---

## 0) Names & mapping (keep the wire stable)

```rust
// server_core/src/types.rs
use glam::Vec3;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActorId(pub u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ActorKind { Wizard, Zombie, Boss }

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Team { Pc, Wizards, Undead, Neutral }

impl ActorKind {
    #[inline] pub fn to_u8(self) -> u8 { match self { Self::Wizard=>0, Self::Zombie=>1, Self::Boss=>2 } }
    #[inline] pub fn from_u8(x:u8)->Self { match x {0=>Self::Wizard,1=>Self::Zombie,_=>Self::Boss} }
}
impl Team {
    #[inline] pub fn to_u8(self) -> u8 { match self { Self::Pc=>0, Self::Wizards=>1, Self::Undead=>2, Self::Neutral=>3 } }
    #[inline] pub fn from_u8(x:u8)->Self { match x {0=>Self::Pc,1=>Self::Wizards,2=>Self::Undead,_=>Self::Neutral} }
}
```

---

## 1) Minimal ECS components (extend your `ecs_core::components`)

> If any of these already exist, **reuse** them; otherwise add.

```rust
// ecs_core/src/components.rs  (add these if missing)
use glam::{Vec3, Vec2};

#[derive(Copy, Clone, Debug)]
pub struct Transform { pub pos: Vec3, pub yaw: f32 }

#[derive(Copy, Clone, Debug)]
pub struct Radius(pub f32);

#[derive(Copy, Clone, Debug)]
pub struct Health { pub hp: i32, pub max: i32 }
impl Health { #[inline] pub fn alive(&self)->bool{ self.hp>0 } }

#[derive(Copy, Clone, Debug)]
pub struct Kind(pub crate::types::ActorKind);

#[derive(Copy, Clone, Debug)]
pub struct TeamC(pub crate::types::Team);

#[derive(Copy, Clone, Debug)]
pub struct MoveSpeed(pub f32);          // m/s

#[derive(Copy, Clone, Debug)]
pub struct AggroRadius(pub f32);        // meters

#[derive(Copy, Clone, Debug)]
pub struct AttackRadius(pub f32);       // meters

#[derive(Copy, Clone, Debug)]
pub struct Target(pub Option<ecs_core::Entity>);

#[derive(Copy, Clone, Debug)]
pub struct Melee { pub damage: i32, pub cooldown_s: f32, pub next_ready_t: f32 }

#[derive(Copy, Clone, Debug)]
pub enum ProjKind { Firebolt, Fireball, MagicMissile }

#[derive(Copy, Clone, Debug)]
pub struct Projectile {
    pub kind: ProjKind,
    pub vel: Vec3,
    pub ttl: f32,
    pub owner: Option<ecs_core::Entity>,
}

#[derive(Copy, Clone, Debug)]
pub struct Homing { pub turn_rate: f32, pub target: Option<ecs_core::Entity> }

#[derive(Copy, Clone, Debug, Default)]
pub struct Despawn; // tag component for cleanup
```

---

## 2) World wrapper, ID map, faction matrix, event buses, spatial grid

```rust
// server_core/src/ecs/world.rs
use std::collections::HashMap;
use ecs_core::{World, Entity};
use glam::{Vec2, Vec3};
use crate::types::{ActorId, ActorKind, Team};
use ecs_core::components as ec;

#[derive(Clone)]
pub struct Factions([[bool; 4]; 4]); // indexed by Team::to_u8()
impl Default for Factions {
    fn default() -> Self {
        let mut m=[[false;4];4];
        let (pc,wiz,und,neu)=(0,1,2,3);
        m[pc][und]=true; m[und][pc]=true;
        m[wiz][und]=true; m[und][wiz]=true;
        Self(m)
    }
}
impl Factions {
    #[inline] pub fn hostile(&self, a: Team, b: Team)->bool{ self.0[a.to_u8() as usize][b.to_u8() as usize] }
    #[inline] pub fn set_hostile(&mut self, a: Team, b: Team, v: bool){
        let (ai,bi)=(a.to_u8() as usize,b.to_u8() as usize); self.0[ai][bi]=v; self.0[bi][ai]=v;
    }
}

#[derive(Default)]
pub struct NetMap {
    next: u32,
    e2id: HashMap<Entity, ActorId>,
    id2e: HashMap<ActorId, Entity>,
}
impl NetMap {
    pub fn alloc(&mut self, e: Entity) -> ActorId {
        let id=ActorId(self.next); self.next=self.next.wrapping_add(1);
        self.e2id.insert(e,id); self.id2e.insert(id,e); id
    }
    #[inline] pub fn entity(&self, id: ActorId)->Option<Entity>{ self.id2e.get(&id).copied() }
    #[inline] pub fn id(&self, e: Entity)->Option<ActorId>{ self.e2id.get(&e).copied() }
    pub fn remove(&mut self, e: Entity){ if let Some(id)=self.e2id.remove(&e){ self.id2e.remove(&id);} }
}

#[derive(Default)]
pub struct DamageEvent { pub src: Option<Entity>, pub dst: Entity, pub amount: i32 }
#[derive(Default)]
pub struct ExplodeEvent { pub center: Vec2, pub r2: f32, pub src: Option<Entity> }

#[derive(Default)]
pub struct DamageEvents(pub Vec<DamageEvent>);
#[derive(Default)]
pub struct ExplodeEvents(pub Vec<ExplodeEvent>);
impl DamageEvents { #[inline] pub fn push(&mut self, ev: DamageEvent){ self.0.push(ev);} #[inline] pub fn drain(&mut self)->Vec<DamageEvent>{ std::mem::take(&mut self.0) } }
impl ExplodeEvents { #[inline] pub fn push(&mut self, ev: ExplodeEvent){ self.0.push(ev);} #[inline] pub fn drain(&mut self)->Vec<ExplodeEvent>{ std::mem::take(&mut self.0) } }

/// Simple uniform grid (XZ). Start naïve (rebuild each tick); upgrade to incremental later.
pub struct SpatialGrid {
    pub cell: f32,
    pub cells: HashMap<(i32,i32), Vec<Entity>>,
}
impl Default for SpatialGrid { fn default()->Self{ Self{ cell: 3.0, cells: HashMap::new() } } }
impl SpatialGrid {
    #[inline] fn key(&self, p: Vec3)->(i32,i32){
        let i=(p.x/self.cell).floor() as i32; let k=(p.z/self.cell).floor() as i32; (i,k)
    }
    pub fn rebuild(&mut self, world: &World){
        self.cells.clear();
        for (e, (t, _r)) in world.view::<(&ec::Transform, Option<&ec::Radius>)>().iter(){
            self.cells.entry(self.key(t.pos)).or_default().push(e);
        }
    }
    pub fn query_circle<'a>(&'a self, center: Vec2, r: f32) -> impl Iterator<Item=&'a Entity> {
        let cell_r=(r/self.cell).ceil() as i32;
        let (cx,cz)=((center.x/self.cell).floor() as i32, (center.y/self.cell).floor() as i32);
        let mut out: Vec<&Entity>=Vec::new();
        for dx in -cell_r..=cell_r {
            for dz in -cell_r..=cell_r {
                if let Some(bucket)=self.cells.get(&(cx+dx,cz+dz)){
                    for e in bucket { out.push(e); }
                }
            }
        }
        out.into_iter()
    }
}
```

---

## 3) Schedule, context, trait `System`, and the fixed‑order run

```rust
// server_core/src/ecs/schedule.rs
use ecs_core::{World, Entity};
use glam::{Vec2, Vec3};
use crate::types::{ActorKind, Team};
use ecs_core::components as ec;
use super::world::{Factions, NetMap, SpatialGrid, DamageEvents, ExplodeEvents, DamageEvent, ExplodeEvent};

pub struct Ctx {
    pub dt: f32,
    pub time_s: f32,
    pub factions: Factions,
    pub net: NetMap,
    pub spatial: SpatialGrid,
    pub dmg: DamageEvents,
    pub boom: ExplodeEvents,
    // Optional: inbound client commands, rng, config, etc.
}
impl Default for Ctx {
    fn default()->Self{ Self{ dt:0.016, time_s:0.0, factions:Factions::default(), net:NetMap::default(), spatial:SpatialGrid::default(), dmg:DamageEvents::default(), boom:ExplodeEvents::default() } }
}

pub trait System { fn run(&mut self, world: &mut World, ctx: &mut Ctx); }

pub struct Schedule {
    pub input: InputSystem,
    pub ai_select: AISelectTargetSystem,
    pub ai_move: AIMoveSystem,
    pub melee: MeleeSystem,
    pub proj_integrate: ProjectileIntegrateSystem,
    pub proj_collision: ProjectileCollisionSystem,
    pub aoe: AoESystem,
    pub faction: FactionSystem,
    pub apply_damage: ApplyDamageSystem,
    pub cleanup: CleanupSystem,
    pub snapshot: SnapshotSystem, // builds ActorSnapshot v2
}
impl Default for Schedule {
    fn default()->Self { Self{
        input: InputSystem::default(),
        ai_select: AISelectTargetSystem,
        ai_move: AIMoveSystem,
        melee: MeleeSystem,
        proj_integrate: ProjectileIntegrateSystem,
        proj_collision: ProjectileCollisionSystem,
        aoe: AoESystem,
        faction: FactionSystem,
        apply_damage: ApplyDamageSystem,
        cleanup: CleanupSystem,
        snapshot: SnapshotSystem,
    }}
}
impl Schedule {
    pub fn run(&mut self, world:&mut World, ctx:&mut Ctx) {
        self.input.run(world, ctx);
        ctx.spatial.rebuild(world);
        self.ai_select.run(world, ctx);
        self.ai_move.run(world, ctx);
        self.melee.run(world, ctx);
        self.proj_integrate.run(world, ctx);
        self.proj_collision.run(world, ctx);
        self.aoe.run(world, ctx);
        self.faction.run(world, ctx);
        self.apply_damage.run(world, ctx);
        self.cleanup.run(world, ctx);
        self.snapshot.run(world, ctx);
    }
}
```

---

## 4) Systems (skeletons)

### 4.1 Input → spawn projectiles

```rust
// server_core/src/ecs/systems/input.rs
use super::{System};
use ecs_core::{World};
use glam::Vec3;
use crate::types::{Team, ActorKind};
use ecs_core::components as ec;
use crate::server_spawn::spawn_projectile; // we'll add this helper

#[derive(Default)]
pub struct InputSystem;

impl System for InputSystem {
    fn run(&mut self, world:&mut World, ctx:&mut super::Ctx) {
        // TODO: read and drain your server-side input queue here.
        // Example: for each queued ClientCmd::Cast { owner_id, kind, pos, dir }:
        //
        // let owner_e = ctx.net.entity(owner_id).expect("owner exists");
        // spawn_projectile(world, &mut ctx.net, owner_e, pos, dir, kind);
    }
}
```

### 4.2 AI select target

```rust
// server_core/src/ecs/systems/ai_select.rs
use super::{System};
use ecs_core::{World, Entity};
use ecs_core::components as ec;

pub struct AISelectTargetSystem;
impl System for AISelectTargetSystem {
    fn run(&mut self, world:&mut World, ctx:&mut super::Ctx) {
        // Gather all potential targets (alive)
        let mut candidates: Vec<(Entity, glam::Vec3)> = Vec::new();
        for (e, (t, h)) in world.view::<(&ec::Transform, &ec::Health)>().iter() {
            if h.alive() { candidates.push((e, t.pos)); }
        }
        // For each AI needing a target, choose nearest hostile
        for (_e, (kind, team, tr, _aggro, tgt)) in world.view::<(&ec::Kind, &ec::TeamC, &ec::Transform, Option<&ec::AggroRadius>, &mut ec::Target)>().iter_mut() {
            // Skip Wizards for now (they are player/NPC casters)
            if !matches!(kind.0, crate::types::ActorKind::Zombie | crate::types::ActorKind::Boss) { continue; }
            let my_team = team.0;
            let my_pos = tr.pos;
            let mut best: Option<(f32, Entity)> = None;
            for (cand_e, cand_pos) in &candidates {
                // Hostility check
                let ct = world.get::<ec::TeamC>(*cand_e).map(|c| c.0).unwrap_or(crate::types::Team::Neutral);
                if !ctx.factions.hostile(my_team, ct) { continue; }
                let d2 = (cand_pos.x-my_pos.x).powi(2) + (cand_pos.z-my_pos.z).powi(2);
                if best.map(|(b,_)| d2 < b).unwrap_or(true) { best = Some((d2, *cand_e)); }
            }
            tgt.0 = best.map(|(_,e)| e);
        }
    }
}
```

### 4.3 Move toward target

```rust
// server_core/src/ecs/systems/ai_move.rs
use super::System;
use ecs_core::World;
use ecs_core::components as ec;

pub struct AIMoveSystem;
impl System for AIMoveSystem {
    fn run(&mut self, world:&mut World, ctx:&mut super::Ctx) {
        let dt = ctx.dt;
        for (_e, (t, spd, tgt)) in world.view::<(&mut ec::Transform, Option<&ec::MoveSpeed>, &ec::Target)>().iter_mut() {
            let Some(target_e) = tgt.0 else { continue; };
            let Ok(target_t) = world.get::<ec::Transform>(target_e) else { continue; };
            let to = glam::vec3(target_t.pos.x - t.pos.x, 0.0, target_t.pos.z - t.pos.z);
            let dist = to.length();
            if dist > 1e-4 {
                let step = spd.map(|s| s.0).unwrap_or(2.0) * dt;
                let d = step.min(dist);
                t.pos += to.normalize() * d;
                t.yaw = to.x.atan2(to.z); // face movement direction
            }
        }
    }
}
```

### 4.4 Melee (with cooldown)

```rust
// server_core/src/ecs/systems/melee.rs
use super::System;
use ecs_core::World;
use ecs_core::components as ec;

pub struct MeleeSystem;
impl System for MeleeSystem {
    fn run(&mut self, world:&mut World, ctx:&mut super::Ctx) {
        let now = ctx.time_s;
        let dt = ctx.dt;
        for (src, (t, r, atk, tgt)) in world.view::<(&ec::Transform, Option<&ec::AttackRadius>, &mut ec::Melee, &ec::Target)>().entities().iter_mut() {
            let Some(dst_e) = tgt.0 else { continue; };
            let Ok(dst_t) = world.get::<ec::Transform>(dst_e) else { continue; };
            let reach = r.map(|rr| rr.0).unwrap_or(1.0);
            let to = glam::vec2(dst_t.pos.x - t.pos.x, dst_t.pos.z - t.pos.z);
            if to.length_squared() <= reach*reach && now >= atk.next_ready_t {
                ctx.dmg.push(super::world::DamageEvent { src: Some(src), dst: dst_e, amount: atk.damage });
                atk.next_ready_t = now + atk.cooldown_s.max(0.05);
            }
        }
        let _ = dt; // silence warnings if not used yet
    }
}
```

### 4.5 Projectiles integrate

```rust
// server_core/src/ecs/systems/proj_integrate.rs
use super::System;
use ecs_core::World;
use ecs_core::components as ec;

pub struct ProjectileIntegrateSystem;
impl System for ProjectileIntegrateSystem {
    fn run(&mut self, world:&mut World, ctx:&mut super::Ctx) {
        let dt = ctx.dt;
        for (_e, (t, p)) in world.view::<(&mut ec::Transform, &mut ec::Projectile)>().iter_mut() {
            t.pos += p.vel * dt;
            p.ttl -= dt;
            if p.ttl <= 0.0 {
                // Fireball explodes on TTL; Firebolt just despawns
                if matches!(p.kind, ec::ProjKind::Fireball) {
                    ctx.boom.push(super::world::ExplodeEvent {
                        center: glam::vec2(t.pos.x, t.pos.z),
                        r2: 6.0*6.0, // TODO: data-drive
                        src: p.owner,
                    });
                }
                // mark for cleanup
                world.insert(_e, ec::Despawn).ok();
            }
        }
    }
}
```

### 4.6 Projectile collision (segment vs actors; Firebolt hit or Fireball explode)

```rust
// server_core/src/ecs/systems/proj_collision.rs
use super::System;
use ecs_core::{World, Entity};
use ecs_core::components as ec;

pub struct ProjectileCollisionSystem;
impl System for ProjectileCollisionSystem {
    fn run(&mut self, world:&mut World, ctx:&mut super::Ctx) {
        let dt = ctx.dt;
        // Copy needed projectile data to avoid aliasing
        let mut projs: Vec<(Entity, glam::Vec3, glam::Vec3, ec::ProjKind, Option<Entity>)> = Vec::new();
        for (e, (t, p)) in world.view::<(&ec::Transform, &ec::Projectile)>().iter() {
            projs.push((e, t.pos, p.vel, p.kind, p.owner));
        }
        for (e, p0, v, kind, owner) in projs {
            let p1 = p0 + v*dt;
            // broad phase: use spatial cells around segment
            // naive: scan all; upgrade to grid.query_segment
            let mut best: Option<(f32, Entity)> = None;
            for (tgt, (tt, rad, hp, team)) in world.view::<(&ec::Transform, Option<&ec::Radius>, &ec::Health, &ec::TeamC)>().entities().iter() {
                if !hp.alive() { continue; }
                // skip owner
                if Some(tgt) == owner { continue; }
                // hostile?
                let src_team = owner.and_then(|o| world.get::<ec::TeamC>(o).ok()).map(|c| c.0).unwrap_or(crate::types::Team::Neutral);
                if !ctx.factions.hostile(src_team, team.0) { continue; }
                // segment vs cylinder on XZ
                let r = rad.map(|r| r.0).unwrap_or(0.6);
                let a=glam::vec2(p0.x,p0.z); let b=glam::vec2(p1.x,p1.z); let c=glam::vec2(tt.pos.x,tt.pos.z);
                let ab=b-a; let ab2=ab.length_squared();
                let t = if ab2<=1e-6 {0.0} else { ((c-a).dot(ab)/ab2).clamp(0.0,1.0) };
                let q = a + ab*t;
                let d2 = (c-q).length_squared();
                if d2 <= r*r {
                    if best.map(|(bt,_)| t < bt).unwrap_or(true) { best = Some((t, tgt)); }
                }
            }
            if let Some((_t, hit)) = best {
                match kind {
                    ec::ProjKind::Firebolt => {
                        ctx.dmg.push(super::world::DamageEvent { src: owner, dst: hit, amount: 12 }); // TODO: data-drive
                        world.insert(e, ec::Despawn).ok();
                    }
                    ec::ProjKind::Fireball => {
                        let center = glam::vec2(p0.x, p0.z); // explode near start or interpolate by t
                        ctx.boom.push(super::world::ExplodeEvent{ center, r2: 6.0*6.0, src: owner });
                        world.insert(e, ec::Despawn).ok();
                    }
                    ec::ProjKind::MagicMissile => {
                        ctx.dmg.push(super::world::DamageEvent { src: owner, dst: hit, amount: 8 });
                        world.insert(e, ec::Despawn).ok();
                    }
                }
            }
        }
    }
}
```

### 4.7 AoE consume → emit DamageEvents

```rust
// server_core/src/ecs/systems/aoe.rs
use super::System;
use ecs_core::World;
use ecs_core::components as ec;

pub struct AoESystem;
impl System for AoESystem {
    fn run(&mut self, world:&mut World, ctx:&mut super::Ctx) {
        for ev in ctx.boom.drain() {
            // Optionally: skip self-damage for PC-owned AoE
            let src_team = ev.src.and_then(|e| world.get::<ec::TeamC>(e).ok()).map(|t| t.0);
            // Query candidates by grid (naïve: whole world)
            for (e, (t, hp, team)) in world.view::<(&ec::Transform, &ec::Health, &ec::TeamC)>().entities().iter() {
                if !hp.alive() { continue; }
                if let Some(own) = ev.src { if own==e { continue; } } // no self-damage
                if let Some(st) = src_team { if !ctx.factions.hostile(st, team.0) { continue; } }
                let dx=t.pos.x-ev.center.x; let dz=t.pos.z-ev.center.y;
                if dx*dx + dz*dz <= ev.r2 {
                    ctx.dmg.push(super::world::DamageEvent { src: ev.src, dst: e, amount: 20 }); // TODO: data-drive
                }
            }
        }
    }
}
```

### 4.8 Faction flips from damage (general rule)

```rust
// server_core/src/ecs/systems/faction.rs
use super::System;
use ecs_core::{World};
use ecs_core::components as ec;
use crate::types::Team;

pub struct FactionSystem;
impl System for FactionSystem {
    fn run(&mut self, world:&mut World, ctx:&mut super::Ctx) {
        for ev in ctx.dmg.0.iter() {
            let Some(src) = ev.src else { continue; };
            let Ok(dst_team) = world.get::<ec::TeamC>(ev.dst) else { continue; };
            let Ok(src_team) = world.get::<ec::TeamC>(src) else { continue; };
            // Example rule: any PC damaging a Wizard flips Pc<->Wizards hostility on
            if matches!(src_team.0, Team::Pc) && matches!(dst_team.0, Team::Wizards) {
                ctx.factions.set_hostile(Team::Pc, Team::Wizards, true);
            }
        }
    }
}
```

### 4.9 Apply damage & deaths

```rust
// server_core/src/ecs/systems/apply_damage.rs
use super::System;
use ecs_core::{World};
use ecs_core::components as ec;

pub struct ApplyDamageSystem;
impl System for ApplyDamageSystem {
    fn run(&mut self, world:&mut World, ctx:&mut super::Ctx) {
        let events = ctx.dmg.drain();
        for ev in events {
            if let Ok(mut hp) = world.get_mut::<ec::Health>(ev.dst) {
                hp.hp = (hp.hp - ev.amount).max(0);
                if hp.hp == 0 {
                    // Mark for despawn (or attach Dead tag, play animation, etc.)
                    world.insert(ev.dst, ec::Despawn).ok();
                }
            }
        }
    }
}
```

### 4.10 Cleanup

```rust
// server_core/src/ecs/systems/cleanup.rs
use super::System;
use ecs_core::{World, Entity};
use ecs_core::components as ec;

pub struct CleanupSystem;
impl System for CleanupSystem {
    fn run(&mut self, world:&mut World, _ctx:&mut super::Ctx) {
        // Despawn all Despawn-tagged entities
        let mut to_kill: Vec<Entity> = Vec::new();
        for (e, _) in world.view::<&ec::Despawn>().iter() { to_kill.push(e); }
        for e in to_kill { world.despawn(e).ok(); }
    }
}
```

### 4.11 Snapshot build (v2)

```rust
// server_core/src/ecs/systems/snapshot.rs
use super::System;
use ecs_core::World;
use ecs_core::components as ec;
use crate::types::{ActorKind, Team};
use net_core::snapshot::{ActorRep, ActorSnapshot, ProjectileRep, SnapshotEncode};

pub struct SnapshotSystem;
impl SnapshotSystem {
    pub fn build(&self, world:&World, tick: u64) -> ActorSnapshot {
        let mut actors = Vec::new();
        for (_e, (t, yaw, r, hp, kind, team)) in world.view::<(&ec::Transform, Option<&ec::Transform>, Option<&ec::Radius>, &ec::Health, &ec::Kind, &ec::TeamC)>().iter() {
            actors.push(ActorRep {
                id: 0, // fill with NetMap mapping in Server wrapper (below)
                kind: kind.0.to_u8(),
                team: team.0.to_u8(),
                pos: [t.pos.x, t.pos.y, t.pos.z],
                yaw: t.yaw,
                radius: r.map(|x| x.0).unwrap_or(0.6),
                hp: hp.hp, max: hp.max, alive: hp.alive(),
            });
        }
        let mut projectiles=Vec::new();
        for (_e, (t, p)) in world.view::<(&ec::Transform, &ec::Projectile)>().iter() {
            projectiles.push(ProjectileRep {
                id: 0, kind: match p.kind { ec::ProjKind::Firebolt=>0, ec::ProjKind::Fireball=>1, ec::ProjKind::MagicMissile=>2 },
                pos: [t.pos.x, t.pos.y, t.pos.z], vel: [p.vel.x, p.vel.y, p.vel.z],
            });
        }
        ActorSnapshot { v: 2, tick, actors, projectiles }
    }
}
impl System for SnapshotSystem {
    fn run(&mut self, _world:&mut World, _ctx:&mut super::Ctx) {
        // No-op here; call build() from your platform integration where you frame + send
    }
}
```

---

## 5) Spawning helpers (keep spawn surface stable)

```rust
// server_core/src/server_spawn.rs
use ecs_core::{World, Entity};
use glam::Vec3;
use crate::types::{ActorId, ActorKind, Team};
use ecs_core::components as ec;
use super::ecs::world::NetMap;

pub fn spawn_wizard_pc(world:&mut World, net: &mut NetMap, pos:Vec3) -> (Entity, ActorId) {
    let e = world.spawn((ec::Transform{ pos, yaw:0.0 }, ec::Radius(0.7), ec::Health{hp:100,max:100},
                         ec::Kind(ActorKind::Wizard), ec::TeamC(Team::Pc)));
    let id = net.alloc(e);
    (e,id)
}

pub fn spawn_wizard_npc(world:&mut World, net: &mut NetMap, pos:Vec3) -> (Entity, ActorId) {
    let e = world.spawn((ec::Transform{ pos, yaw:0.0 }, ec::Radius(0.7), ec::Health{hp:80,max:80},
                         ec::Kind(ActorKind::Wizard), ec::TeamC(Team::Wizards)));
    let id = net.alloc(e);
    (e,id)
}

pub fn spawn_undead(world:&mut World, net: &mut NetMap, pos:Vec3, radius:f32, hp:i32) -> (Entity, ActorId) {
    let e = world.spawn((ec::Transform{ pos, yaw:0.0 }, ec::Radius(radius), ec::Health{hp, max:hp},
                         ec::Kind(ActorKind::Zombie), ec::TeamC(Team::Undead),
                         ec::MoveSpeed(2.0), ec::AggroRadius(18.0), ec::AttackRadius(radius+0.4),
                         ec::Target(None), ec::Melee{damage:5,cooldown_s:0.6,next_ready_t:0.0}));
    let id = net.alloc(e);
    (e,id)
}

pub fn spawn_projectile(world:&mut World, _net:&mut NetMap, owner:Entity, pos:Vec3, dir:Vec3, kind:ec::ProjKind) -> Entity {
    let speed = match kind { ec::ProjKind::Firebolt=>45.0, ec::ProjKind::Fireball=>24.0, ec::ProjKind::MagicMissile=>30.0 };
    let ttl   = match kind { ec::ProjKind::Firebolt=>1.2,  ec::ProjKind::Fireball=>2.4,  ec::ProjKind::MagicMissile=>0.9  };
    world.spawn((ec::Transform{ pos, yaw: dir.x.atan2(dir.z) }, ec::Projectile{ kind, vel:dir.normalize_or_zero()*speed, ttl, owner:Some(owner) }))
}
```

---

## 6) Server wrapper: integrate schedule into your existing `ServerState`

```rust
// server_core/src/server.rs  (or your existing ServerState)
use ecs_core::World;
use crate::ecs::schedule::{Schedule, Ctx};
use crate::ecs::world::NetMap;
use crate::types::ActorId;
use crate::server_spawn::*;

pub struct Server {
    pub world: World,
    pub sched: Schedule,
    pub ctx: Ctx,
    pub tick: u64,
    pub pc_actor: Option<ActorId>,
}

impl Server {
    pub fn new() -> Self {
        let mut world = World::default();
        let mut ctx = Ctx::default();
        let (pc_e, pc_id) = spawn_wizard_pc(&mut world, &mut ctx.net, glam::vec3(0.0,0.6,0.0));
        Self { world, sched: Schedule::default(), ctx, tick: 0, pc_actor: Some(pc_id) }
    }

    pub fn step(&mut self, dt: f32) {
        self.ctx.dt = dt;
        self.ctx.time_s += dt;
        self.sched.run(&mut self.world, &mut self.ctx);
        self.tick = self.tick.wrapping_add(1);
    }

    pub fn snapshot_v2(&self) -> net_core::snapshot::ActorSnapshot {
        // Build from ECS; fill network IDs using NetMap
        use net_core::snapshot::{ActorRep, ProjectileRep, ActorSnapshot};
        let mut actors = Vec::new();
        for (e, (t, r, hp, kind, team)) in self.world.view::<(&ecs_core::components::Transform, Option<&ecs_core::components::Radius>, &ecs_core::components::Health, &ecs_core::components::Kind, &ecs_core::components::TeamC)>().entities().iter() {
            let id = self.ctx.net.id(e).map(|id| id.0).unwrap_or(0);
            actors.push(ActorRep {
                id,
                kind: kind.0.to_u8(),
                team: team.0.to_u8(),
                pos: [t.pos.x, t.pos.y, t.pos.z],
                yaw: t.yaw,
                radius: r.map(|x| x.0).unwrap_or(0.6),
                hp: hp.hp, max: hp.max, alive: hp.alive(),
            });
        }
        let mut projectiles=Vec::new();
        for (_e, (t, p)) in self.world.view::<(&ecs_core::components::Transform, &ecs_core::components::Projectile)>().iter() {
            projectiles.push(ProjectileRep {
                id: 0, // projectiles are ephemeral; you can map if needed
                kind: match p.kind { ecs_core::components::ProjKind::Firebolt=>0, ecs_core::components::Fireball=>1, ecs_core::components::MagicMissile=>2 },
                pos: [t.pos.x, t.pos.y, t.pos.z],
                vel: [p.vel.x, p.vel.y, p.vel.z],
            });
        }
        ActorSnapshot { v: 2, tick: self.tick, actors, projectiles }
    }
}
```

> **Note:** replace `view::<...>()` and `spawn()` calls with the exact API your `ecs_core` exposes (hecs‑like, bevy‑like, or your own). The structure is deliberately straightforward to map.

---

## 7) How to wire quickly (no yak‑shaving)

1. **Add the files** above under `server_core/src/ecs/{world.rs,schedule.rs}` and `server_core/src/ecs/systems/*.rs`, plus `server_core/src/types.rs` and `server_core/src/server_spawn.rs`.
2. In your current `ServerState`, **swap** in the `Server` wrapper or merge into it:

   * Replace old `step_authoritative` body with `self.step(dt)` calling the schedule.
   * Replace custom AoE & projectile loops with system versions (you can keep the constants the same for now).
3. In `platform_winit`, keep the **ordering** you already fixed:

   * Build snapshot first (`server.snapshot_v2()`), frame & send, then call `server.step(dt)`.
4. Confirm **actor IDs** remain stable for HUD: use `ctx.net.alloc(e)` on spawn; use `ctx.net.id(e)` during snapshot.

---

## 8) Optional helpers you’ll want very soon

* **Quantization utils** (to shrink snapshots):

  ```rust
  #[inline] pub fn qpos(x:f32)->i32 { (x*64.0).round() as i32 }
  #[inline] pub fn dqpos(x:i32)->f32 { (x as f32)/64.0 }
  #[inline] pub fn qyaw(y:f32)->u16 { (((y%std::f32::consts::TAU)+std::f32::consts::TAU) % std::f32::consts::TAU * (65535.0/std::f32::consts::TAU)) as u16 }
  #[inline] pub fn dqyaw(y:u16)->f32 { (y as f32) * (std::f32::consts::TAU/65535.0) }
  ```
* **Segment vs circle util** (reuse in projectile collisions):

  ```rust
  pub fn seg_circle_hit(a: glam::Vec2, b: glam::Vec2, c: glam::Vec2, r: f32) -> Option<f32> {
      let ab = b - a; let ab2 = ab.length_squared();
      let t = if ab2<=1e-6 { 0.0 } else { ((c-a).dot(ab)/ab2).clamp(0.0,1.0) };
      let q = a + ab*t; let d2 = (c-q).length_squared();
      if d2 <= r*r { Some(t) } else { None }
  }
  ```

---

## 9) What to fill in next (fast wins)

* **InputSystem**: wire your `ClientCmd` queue (already exists on the server) → call `spawn_projectile`.
* **ProjectileCollisionSystem**: switch from full scan to `SpatialGrid.query_circle` once the naïve path is working.
* **Melee cooldowns**: set `next_ready_t` at spawn; it’s already enforced.
* **Faction rules**: if you need more than “PC hits Wizard flips”, expand `FactionSystem` with a small rules table.

---

If you want, I can tailor the system `view::<…>()` calls to *your actual* `ecs_core` query API—just drop a snippet of `ecs_core::World`’s query/spawn signatures and I’ll align the code to compile on first try.
