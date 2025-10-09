# Magic Missile (SRD 5.2.1) — Implementation Spec

Scope: map the SRD 5.2.1 Magic Missile spell into deterministic, simulation‑friendly data and events aligned with our `sim_core` and `data_runtime` patterns (see `docs/fire_bolt.md` for the template). This document proposes schema and system updates needed for multi‑hit, auto‑hit spells and slot‑level scaling.

—

## A) Rules Facts (SRD 5.2.1)

- Name: Magic Missile
- School/Level: Evocation 1 (Wizard, Sorcerer)
- Casting Time: Action
- Range: 120 ft
- Components: V, S
- Duration: Instantaneous
- Effect: Create three glowing darts of magical force. Each dart hits a creature you can see within range, dealing 1d4 + 1 Force damage. The darts strike simultaneously. You can direct them to hit one creature or several.
- Higher Levels: +1 dart for each slot level above 1.

Note: Facts above restated from SRD 5.2.1 for implementation; attribution lives in `NOTICE`.

—

## B) Sim‑Engine Contract (proposed)

Identity & tags
- `id`: `"wiz.magic_missile.srd521"`
- `name`: `"Magic Missile"`
- `source`: `"SRD 5.2.1"`
- `school`: `"evocation"`, `level`: `1`
- `classes`: `["wizard","sorcerer"]`
- `tags`: `["auto-hit","multi-hit","force","projectile-optional","negated-by-shield"]`

Cast, queueing, cooldowns
- `cast_time_s`: `1.0` (Action; 1.0 s pacing)
- `gcd_s`: `1.0`
- `cooldown_s`: `0.0`
- `resource_cost`: `none`
- `can_move_while_casting`: `false`

Targeting & LoS
- `targeting`: `"unit"` (single selection in current HUD); engine will support multi‑target distribution policy for darts (see below).
- `requires_line_of_sight`: `true`
- `range_ft`: `120`, `minimum_range_ft`: `0`, `firing_arc_deg`: `180`

Resolution model
- `attack`: `none` (auto‑hit; no attack roll)
- `save`: `none`
- Multi‑hit instances: per SRD: 3 darts at slot level 1; +1 dart per slot above 1.
- Each instance (dart) deals `1d4 + 1` Force. Darts hit simultaneously; apply all damage in the same tick.
- Shield interaction: If the target has an available Shield reaction, it negates Magic Missile. We should model this as a reaction hook at damage time for abilities with a `negated_by_reaction = "shield"` hint (details below).

Damage & scaling (proposed schema changes)
- Extend `DamageSpec` or add `MultiHitSpec` to express per‑instance dice and a flat bonus:
  - `per_instance.dice`: `"1d4"`
  - `per_instance.flat_bonus`: `1`
  - `instances`: `{ base: 3, per_slot_above: 1 }`
  - `type`: `"force"`
- Alternatively, as a stop‑gap, support `NdM+K` dice grammar so a single pending damage can use `"3d4+3"` at slot 1, `"4d4+4"` at slot 2, etc. This is less expressive (can’t distribute hits across targets) but minimal.

Slot level source
- Add an optional `slot_level` to the cast context. Until we pass that through the sim, default `slot_level = level` (1) for Magic Missile.
- For AI/autoplay, add a simple policy: prefer lowest slot sufficient (L1 by default), unless test harness overrides.

Distribution policy (no new UI yet)
- Default: focus current target; assign all darts to `actor.target` if present.
- Optional auto‑spread: if `allow_multi_target = true` and there are ≥2 hostile valid targets, fill darts as: 1 → current target; remaining darts to nearest alive hostiles by distance. Deterministic tie‑break by actor id.
- Future HUD: explicit dart allocation UI could set a per‑cast distribution vector.

Event model (sim‑core bus)
- `CastStarted(actor, spell_id, t)`
- `CastCompleted(actor, spell_id, t)`
- If we visualize darts: `ProjectileSpawned(actor, spell_id, dart_id, origin, dir, t)` repeated `instances` times; otherwise skip.
- `HitResolved(dart_id?, target, hit=true, crit=false, roll=auto, t)` per dart
- `DamageApplied(source, target, amount, type, t)` per dart
- `ReactionUsed(target, "shield", t)` when Shield negates all incoming MM instances to that target

VFX/SFX (viz stubs)
- `vfx.dart`: `"magic_missile_dart"`, `vfx.impact`: `"magic_missile_pop"`
- `sfx.cast`: `"magic_missile_cast"`, `sfx.impact`: `"magic_missile_hit"`

—

## C) JSON (recommended, requires schema additions)

This format introduces `multihit` and per‑instance damage description. It keeps backward‑compatibility for existing spells.

```json
{
  "id": "wiz.magic_missile.srd521",
  "name": "Magic Missile",
  "version": "1.0.0",
  "source": "SRD 5.2.1",
  "school": "evocation",
  "level": 1,
  "classes": ["wizard", "sorcerer"],
  "tags": ["auto-hit", "multi-hit", "force", "negated-by-shield"],

  "cast_time_s": 1.0,
  "gcd_s": 1.0,
  "cooldown_s": 0.0,
  "resource_cost": null,
  "can_move_while_casting": false,

  "targeting": "unit",
  "requires_line_of_sight": true,
  "range_ft": 120,
  "minimum_range_ft": 0,
  "firing_arc_deg": 180,

  "attack": null,
  "save": null,

  "damage": {
    "type": "force",
    "add_spell_mod_to_damage": false
  },
  "multihit": {
    "per_instance": { "dice": "1d4", "flat_bonus": 1 },
    "instances": { "base": 3, "per_slot_above": 1 },
    "distribution": { "mode": "focus-current", "allow_multi_target": true },
    "negated_by_reaction": "shield"
  },

  "projectile": { "enabled": false, "speed_mps": 0.0, "radius_m": 0.0, "gravity": 0.0, "collide_with": [], "spawn_offset_m": {"x":0,"y":0,"z":0} },

  "events": ["CastStarted","CastCompleted","HitResolved","DamageApplied","ReactionUsed"],
  "metrics": { "collect": ["casts","instances","damage_total","damage_mean"] },
  "policy": { "role": "dps", "priority_index": 1 }
}
```

Fallback JSON (works today if we extend dice grammar only)
- Represent total as a single instance with aggregated dice: `3d4+3` at slot 1, `4d4+4` at slot 2, etc. This loses per‑target distribution but is useful as an interim.

—

## D) Engine Changes (small, focused)

- `data_runtime::spell::SpellSpec`
  - Add optional `multihit` struct:
    - `per_instance: { dice: String, flat_bonus: i32 }`
    - `instances: { base: u32, per_slot_above: u32 }`
    - `distribution: { mode: String, allow_multi_target: bool }`
    - `negated_by_reaction: Option<String>`
  - Keep existing `damage` for type and common flags.

- Dice parser in `SimState::roll_dice_str`
  - Extend to support `NdM+K` (and `NdM-K`). This is broadly useful and makes JSON more expressive even outside Magic Missile.

- `sim_core::sim::state::SimState`
  - Add optional `slot_level` to a cast context. Short‑term: default to spell level (1) for Magic Missile.

- `sim_core::systems::attack_roll`
  - If `spec.attack.is_none()` and `spec.multihit.is_some()`: push `instances` entries to `pending_damage` (one per dart), instead of the current single entry. Set `crit=false`.

- `sim_core::systems::damage`
  - When resolving entries for a spell with `multihit`, compute `per_instance` damage as `roll("1d4") + flat_bonus`. Respect `damage.type == "force"` with no underwater halving.
  - Apply all instance damages within the same tick (they already are, since we process the batch for that tick).
  - Reaction hook: if the target has a ready Shield reaction and the incoming ability has `negated_by_reaction == "shield"`, consume reaction and negate the instance. If multiple instances target the same unit in the same tick, one reaction negates all of them (MM clause). Log `reaction_used` once.

- Distribution (no UI):
  - If `distribution.mode == "focus-current"`: all instances use `actor.target`.
  - If `allow_multi_target == true` and there are extra darts with no explicit assignment: pick nearest alive hostile distinct targets until darts are exhausted; deterministic tie‑break by id.

—

## E) Notes & Defaults

- Force damage: not subject to the current underwater fire halving. No resistances modeled yet.
- Simultaneity: ensure no interleaving that could create multiple concentration checks from one JSON aggregate form inadvertently. With per‑instance entries, you will get separate concentration checks; that matches typical 5E rulings for multiple hits.
- VFX: we can visualize darts later. For now, treat as instantaneous hits to keep parity with tests.
- Logging: include `instances=N`, and per‑instance `dmg=X` in `damage_applied` lines for easier debugging.

—

## F) Minimal Unit Tests (deterministic)

1) Base instances: slot 1, caster with target set. After `cast_completed`, `attack_roll::run` pushes 3 entries for `wiz.magic_missile.srd521`; `damage::run` reduces target HP by the sum of three `(1d4+1)` rolls.
2) Slot scaling: slot 3 → 5 entries. Verify `pending_damage.len() == 5` before damage.
3) Distribution (auto): two hostiles in range, no UI allocation. With 3 darts, first goes to current target, remaining to nearest other(s). Assert damage applied to both.
4) Shield interaction: target has `wiz.shield.srd521` and `reaction_ready = true`. On first incoming MM instance, consume reaction, apply `shield_reaction` log, and apply zero damage from all darts to that target in this tick.
5) Determinism: given fixed seed, sum of three `(1d4+1)` rolls is stable across runs.

—

## G) Incremental Path

- Phase 1 (smallest): extend dice grammar to accept `NdM+K`; encode Magic Missile as a single‑target aggregate (`3d4+3` at slot 1). No Shield special‑case yet. This unblocks early playtests.
- Phase 2: add `multihit` schema, per‑instance resolution, and distribution policy; add Shield negation hook.
- Phase 3: HUD support for explicit dart allocation and optional dart VFX.

—

## H) JSON Stub (aggregate fallback)

This version works once `NdM+K` is supported, without any other schema changes. Darts are implicitly focused on the current target.

```json
{
  "id": "wiz.magic_missile.srd521",
  "name": "Magic Missile",
  "source": "SRD 5.2.1",
  "school": "evocation",
  "level": 1,
  "classes": ["wizard", "sorcerer"],
  "tags": ["auto-hit", "force"],

  "cast_time_s": 1.0,
  "gcd_s": 1.0,
  "cooldown_s": 0.0,
  "resource_cost": null,
  "can_move_while_casting": false,

  "targeting": "unit",
  "requires_line_of_sight": true,
  "range_ft": 120,
  "minimum_range_ft": 0,
  "firing_arc_deg": 180,

  "attack": null,
  "save": null,
  "damage": { "type": "force", "add_spell_mod_to_damage": false, "dice_by_level_band": { "1-20": "3d4+3" } },
  "projectile": null
}
```

—

Rationale cross‑refs
- See `docs/fire_bolt.md` for the baseline spec structure mirrored here.
- Current sim code that informs this plan: `crates/sim_core/src/sim/systems/attack_roll.rs` (auto‑hit path when `attack` is `None`), `crates/sim_core/src/sim/systems/damage.rs` (pending damage resolution and concentration checks), and `tests/sim_systems.rs` (Fire Bolt, Bless, Shield interactions).

