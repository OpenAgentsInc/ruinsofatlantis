Got it. You want the **old paths gone** and the new ECS path to be the **only** way anything moves, fights, or renders—even if we break some things while we converge. Below is a set of **explicit work orders** for the coding agent: small, atomic PRs with the *exact* files to touch, what to delete, what to keep, sanity checks to run, and “done” criteria. I’ve included an **immediate hot‑fix** for “PC spells don’t hit Wizards,” then the cut‑over and cleanup plan.

---

## 0) IMMEDIATE HOT‑FIX — PC spells must damage Wizards

### Problem

Projectiles collide, but **faction gating** blocks damage because `Pc↔Wizards` hostility is false until it flips on damage—classic chicken‑and‑egg.

### Fix (pick one; do both if fast)

**A. Default the matrix to hostile for demo**

* **File:** `crates/server_core/src/ecs/world.rs` (or wherever `Factions::default()` lives)
* **Change:** set Pc↔Wizards `true` in the default matrix.

```rust
impl Default for Factions {
    fn default() -> Self {
        let mut m = [[false; 4]; 4];
        let (pc, wiz, und, neu) = (0, 1, 2, 3);
        m[pc][und] = true; m[und][pc] = true;
        m[wiz][und] = true; m[und][wiz] = true;
        // HOTFIX: allow PC to hit Wizards immediately
        m[pc][wiz] = true; m[wiz][pc] = true;
        Self(m)
    }
}
```

**B. Bypass hostility check for PC‑owned projectiles hitting Wizards (until threat rules land)**

* **File:** `crates/server_core/src/ecs/schedule.rs` (`projectile_collision_*`)
* **Change:** when `owner_team == Team::Pc && target_team == Team::Wizards`, treat as hostile.

```rust
let hostile = ctx.factions.hostile(owner_team, team.0)
    || (owner_team == Team::Pc && team.0 == Team::Wizards);
if !hostile { continue; }
```

**Acceptance**

* Log should show: `snapshot_v2: ... projectiles>0`.
* PCs can **hit Wizards** reliably; MM/FB/FBolt reduce Wizard HP.

---

## 1) HARD CUT: remove legacy client‑side AI/combat/replication

**Goal:** Only ECS/server authority drives gameplay; client renders from replication only.

### 1.1 Delete legacy features and code paths

* **Files to edit:**

  * `crates/render_wgpu/Cargo.toml`: remove `legacy_client_ai`, `legacy_client_combat`, `legacy_client_carve` features and any feature gates.
  * `crates/render_wgpu/src/gfx/renderer/update.rs`: **delete** all `#[cfg(feature = "legacy_*")]` blocks (collision, AI, carve).
  * `crates/client_core/src/replication.rs`: remove decoders for `NpcListMsg` and `BossStatusMsg`.
* **Repo hygiene guard:**

  * Add a CI grep in your xtask or simple script: fail build if `git grep -n "legacy_client_"` returns anything.

**Acceptance**

* Build succeeds with **no** `legacy_client_*` in tree.
* Game still runs: projectiles/actors come **only** from actor snapshots/deltas.

---

## 2) Make v3 deltas the **only** replication mode

**Goal:** One network path, fewer edge cases.

* **File:** `crates/platform_winit/src/lib.rs`

  * Remove env `RA_SEND_V3` branching; **always** build and send `ActorSnapshotDelta v3` + full projectile list each tick.
  * Keep `v2` encoder only in `net_core` behind a **dev/test** feature (if you need tooling), not used in runtime.
* **File:** `crates/client_core/src/replication.rs`

  * Keep v3 decoder and HUD decode; delete v2 full‑snapshot apply path if you don’t need it anymore.
* **Sanity log:** one‑line on client: `repl: v3 tick=..., spawns=..., updates=..., removals=..., projectiles=...`.

**Acceptance**

* With casting, client prints `projectiles>0` consistently; zombies continue to animate.

---

## 3) Delete the pre‑ECS `ActorStore` and any “bridge” comments

* **Files:**

  * `crates/server_core/src/actor.rs` (remove `ActorStore` and helpers)
  * `crates/server_core/src/lib.rs` (delete comments and dead methods referencing legacy store)
* **Search & destroy:**

  * `git grep -n "ActorStore"` → remove all references/imports.

**Acceptance**

* No references to `ActorStore` remain; only `ecs::WorldEcs` is used.

---

## 4) Replace `sync_wizards()` mirroring with **authoritative intents**

**Goal:** Server owns player transforms; client sends inputs only.

### 4.1 Server components

* **Files:** `crates/server_core/src/ecs/components.rs` or your server ECS module

  * Add: `IntentMove { dir: glam::Vec2, run: bool }`, `IntentAim { yaw: f32 }`.
  * Add: `RespawnPolicy { graveyard: Vec3, delay_s: f32 }` (PC only).

### 4.2 Input system

* **File:** `crates/server_core/src/ecs/schedule.rs`

  * Add `input_apply_intents(srv, ctx)` first in the schedule: integrates `IntentMove` (speed * dt), sets `yaw` from `IntentAim`.
  * Remove `sync_wizards()` from `step_authoritative`; platform sends intents instead of absolute positions.

### 4.3 Platform plumbing

* **File:** `crates/platform_winit/src/lib.rs`

  * Build `ClientCmd::Move{dir,run}` + `ClientCmd::Aim{yaw}` each frame.
  * Server receives and writes to the PC entity’s intent components.
  * Remove all calls to `srv.sync_wizards()`.

**Acceptance**

* PC still moves; server logs positions changing without any `sync_wizards()` call.
* Respawn (temp): if HP==0, `RespawnSystem` sets a timer and moves PC to graveyard after `delay_s`.

---

## 5) ECS schedule order (lock it down in one place)

**File:** `crates/server_core/src/ecs/schedule.rs`

**Order must be**:

1. `input_apply_intents`
2. `cooldown_and_mana_tick`
3. `cast_system` (drain `pending_casts` → `pending_projectiles`)
4. `ingest_projectile_spawns`
5. `effects_tick` (burn/slow/stun timers + DoT → `DamageEvent`)
6. `spatial.rebuild` (until incrementalized; see §7)
7. `ai_select_targets` (optional), `ai_move`
8. `melee_apply_when_contact` (→ `DamageEvent`, respects cooldowns)
9. `homing_acquire_targets` (if enabled), then `homing_update`
10. `projectile_integrate_ecs`
11. `projectile_collision_ecs` (segment vs grid candidates → `DamageEvent`/`ExplodeEvent`)
12. `aoe_apply_explosions` (→ `DamageEvent`s)
13. `faction_on_damage` (flip rules)
14. `apply_damage_to_ecs` (→ `DeathEvent`, `DespawnAfter{…}`)
15. `cleanup` (honor timers; never blanket purge)
16. (optional) `snapshot_metrics` (do not build network data here; platform handles it)

**Acceptance**

* A single `Schedule::run` controls all gameplay updates; no stray mutation outside.

---

## 6) Projectile collision must include Wizards (and skip self)

* **File:** `crates/server_core/src/ecs/schedule.rs` (`projectile_collision_ecs`)

  * **Skip owner**: already present.
  * **Hostility**: apply the hot‑fix from §0; ensure Wizards pass the hostile gate for PC‑owned projectiles.
  * **Radius**: use target `Radius` + projectile shape radius (if any) for cylinder hit on XZ.
  * **Tie‑break**: for multi hits, pick minimum `t` and then tie‑break on `(t, ActorId)` (you already added this, keep it).

**Acceptance**

* Add/enable a test: `server_core/tests/firebolt_hits_wizard.rs` (PC casts Firebolt at Wizard 5m away → Wizard HP reduced).

---

## 7) Spatial grid: incremental + used for projectile broad‑phase

**Goal:** No O(N) scans per projectile.

### 7.1 Move grid into ECS world and update on move

* **File:** `crates/server_core/src/ecs/world.rs`

  * Add `SpatialGrid { cell: f32, buckets: HashMap<Cell, SmallVec<ActorId, N>> }`.
  * On any `Transform` write, mark dirty → `grid.update_entity(id, old_pos, new_pos)`.
  * Expose queries: `query_circle(center, r)`, `query_segment(a, b, pad)` (iterate cells overlapped by the segment’s AABB padded by target radius).

### 7.2 Use it in collision

* **File:** `projectile_collision_ecs`

  * Gather candidates from `query_segment` instead of scanning all actors.
  * Keep final precise test (segment‑vs‑cylinder) per candidate.

**Acceptance**

* With 100+ actors and 20+ projectiles, per‑tick time is stable; log grid candidate counts vs total actors for verification.

---

## 8) Renderer: make animation state entirely replication‑driven

**Goal:** “Some zombies not animated” usually means **we are not updating state for all replicated actors** or using multiple state machines.

* **Files:** `crates/render_wgpu/src/gfx/renderer/update.rs`

  * Ensure we **diff** `ReplicationBuffer.actors` each frame:

    * Spawn visuals for new actor IDs
    * Update positions/yaws for existing IDs
    * Despawn visuals for removed IDs
  * Drive animation from **computed state**:

    * `moving = (pos - prev_pos).length() > ε` ⇒ `Jog_Fwd_Loop`
    * `!moving && alive` ⇒ `Idle_Loop`
    * `hp == 0` ⇒ play `Death01` once; freeze after end
    * Optional: when `melee_apply_when_contact` fires server‑side, replicate a tiny `MeleeEvent` (or infer from cooldown reset) to trigger `Sword_Attack` clip client‑side.
  * **Delete** any animator logic gated by legacy flags.

**Acceptance**

* Every replicated zombie has an animator; when you kite in a circle, you see them jogging, not T‑posing or frozen.
* No animation depends on client‑side AI or collision.

---

## 9) Effects & HUD fully from server

* **Files:**

  * Server: effects already tick; ensure `HudStatusMsg` includes timers (burn/slow/stun), GCD remaining, per‑spell CDs, and current mana.
  * Platform: send HUD every tick **after** sending the actor delta (so UI is coherent with snapshot).
  * Client: keep only HUD decode that writes to a single `HudState`; UI renders from that struct.
* **Delete** any UI that reads local client combat states.

**Acceptance**

* HUD changes immediately when you cast (GCD bar, cooldown pips) and while burning/slow/stun are applied.

---

## 10) Tests to lock the new path (keep them green)

Add/keep these **server_core** tests:

* `firebolt_hits_wizard.rs` — PC → Wizard damage permitted (hostility / broad‑phase ok)
* `mm_reacquire.rs` — Kill first target; missile re‑targets within range
* `despawn_timer_ticks.rs` — corpse persists until timer elapses
* `effects_and_lifecycle.rs` — burn DoT, slow speed, stun gates actions
* `cast_spawns_projectiles.rs` — enqueue_cast → projectile entity
* `spawn_safety.rs` & `boss_spawn_safety.rs` — respect PC bubble

Add **client_core** tests:

* `replication_sparse_ids.rs` — sparse actor IDs don’t panic
* `replication_projectiles_v3.rs` — v3 delta with projectiles populates buffer

**Acceptance**

* `cargo test --workspace` and `cargo clippy --all-targets -D warnings` pass.

---

## 11) Observability: make diagnosis easy

* Switch ad‑hoc `log::info!` to `tracing` with spans:

  * `schedule.system = "projectile_collision"`, counters: `events.damage`, `events.explode`, `grid.candidates`
  * `replication.tx.bytes`, `replication.actors.sent`, `replication.projectiles.sent`
* Keep a single feature `diagnostics` to toggle extra traces in dev.

**Acceptance**

* One run shows per‑system timings and event counts; you can spot regressions without sprinkling printlns.

---

## 12) Purge list (things to remove now)

* `ActorStore` and related files
* Any `legacy_client_*` feature and code blocks
* Client decoders for legacy messages (`NpcListMsg`, `BossStatusMsg`)
* Any renderer code path that mutates game state or does “client combat”
* `sync_wizards()` + all callsites (after intents are in)
* Env toggles that switch core behavior (`RA_SEND_V3`, legacy gates). Keep only debug toggles.

---

## Sanity checklist the agent should run after each PR

* `cargo clippy --all-targets --workspace -- -D warnings`
* `cargo test --workspace`
* Manual quick‑run:

  * **Cast**: projectiles visible; damage lands on **Wizards** and **Undead**
  * **Effects**: Fireball burns; MM slows; Stun blocks actions
  * **Cleanup**: bodies linger for timer, then despawn
  * **Animation**: all visible zombies animate (idle/jog/death), no frozen clones
  * **Logs**: v3 delta only; projectiles count > 0 after a cast

---

## If projectiles *still* miss Wizards after §0

* Confirm in `projectile_collision_ecs`:

  * We compute `owner_team` from `Owner` correctly
  * We **don’t** filter candidates by team before hostility override
  * Segment‑vs‑cylinder uses correct `Radius` for Wizards
  * We **skip self** and **do not** skip Wizards by mistake (e.g., `kind == Wizard` confusion)
* Run a focused test: spawn PC(0,0), Wizard(0,0,5), cast Firebolt; assert Wizard HP drops.

---

This plan will leave only the ECS/server‑authority path alive, get your PC spells hitting Wizards immediately, and simplify the renderer down to “replicate → animate → draw.” It’s intentionally aggressive about **deleting** old paths to eliminate confusion while we finish the remaining ECS wiring.
