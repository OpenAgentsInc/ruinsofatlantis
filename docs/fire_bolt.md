# Fire Bolt (SRD 5.2.1) — Implementation Spec

Scope: maps the SRD 5.2.1 Fire Bolt cantrip into deterministic, simulation‑friendly data and events for the planned sim‑core/sim‑data crates. Use this as a reference for implementing other spells.

—

## A) Rules Facts (SRD 5.2.1)

- Name: Fire Bolt
- School/Level: Evocation cantrip (Wizard, Sorcerer)
- Casting time: Action
- Range: 120 ft (36.576 m)
- Components: V, S
- Duration: Instantaneous
- Effect: Make a ranged spell attack against a creature or object in range; on hit, 1d10 fire damage; flammable objects not worn/carried ignite.
- Scaling (cantrip upgrade): damage increases by +1d10 at character levels 5 / 11 / 17 → 2d10 / 3d10 / 4d10.

Note: Facts above are restated from SRD 5.2.1 for implementation only; see NOTICE for attribution.

—

## B) Sim‑Engine Contract (proposed)

Identity & tags
- `id`: `"wiz.fire_bolt.srd521"` (include provenance/versioning)
- `name`: `"Fire Bolt"`
- `source`: `"SRD 5.2.1"`
- `school`: `"evocation"`, `level`: `0`
- `classes`: `["wizard","sorcerer"]`
- `tags`: `["projectile","direct-damage","fire","ranged-spell-attack"]`

Cast, queueing, cooldowns
- `cast_time_s`: `0.0` (prototype: instant cast for forward‑shot gameplay feel)
 - `gcd_s`: `1.0` (optional; set `0` if no GCD model)
 - `cooldown_s`: `2.0` (prototype: 2 seconds between instant casts)
- `cooldown_s`: `0.0`
- `resource_cost`: `none`
- `can_move_while_casting`: `false` (tunable)

Targeting & LoS
- `targeting`: `"unit-or-object"`
- `requires_line_of_sight`: `true`
- `range_ft`: `120`, `minimum_range_ft`: `0`, `firing_arc_deg`: `180`

Attack resolution (deterministic)
- `attack_type`: `"ranged_spell_attack"` (d20 + spell attack mod vs AC)
- `uses_rng_stream`: `"attack"` (per‑actor stream)
- `crit_rule`: `"nat20_double_dice"` (if crits are modeled)
- On hit: roll damage and apply type; on miss: no effect; no save.

Damage & scaling
- `damage_type`: `"fire"`
- `add_spell_mod_to_damage`: `false`
- `damage_dice_by_level_band`:
  - `1..4:  "1d10"`
  - `5..10: "2d10"`
  - `11..16:"3d10"`
  - `17..20:"4d10"`

Projectile model (viz + travel time)
- `projectile.enabled`: `true`
- `speed_mps`: `40.0`, `radius_m`: `0.1`, `gravity`: `0.0`
- `collide_with`: `["first_target_hit"]`
- `spawn_offset_m`: `{x:0.0,y:1.6,z:0.5}`
- Impact time = `cast_end + flight_time`, snapped to server tick.
- Renderer behavior (prototype): forward‑shot only (no explicit targeting yet). Projectiles are clamped to the SRD range (120 ft) and visually fade out as they approach the max range.

Note: While Fire Bolt is an Action in SRD, our prototype maps it to an instant cast for responsiveness. Magic Missile retains a 1.0 s cast bar.

Secondary effects
- `ignite_on_flammable_object`: `true` (objects only; not worn/carried)
- `statuses_applied`: `[]`

Networking/latency hooks (optional)
- `uses_actor_input_delay`: true; `server_tick_ms`: 50; `queue_window_ms`: 100

Event model (sim‑core bus)
- `CastStarted(actor, spell_id, t)`
- `CastCompleted(actor, spell_id, t)`
- `ProjectileSpawned(actor, spell_id, proj_id, origin, dir, t)`
- `HitResolved(proj_id, target, hit, crit, roll, t)`
- `DamageApplied(source, target, amount, type, t)`
- `ObjectIgnited(target, t)`

Metrics hooks
- Count: casts, hits, misses, crits
- Damage totals/avg; projectile travel time distribution

Policy hints (AI)
- `role`: `"dps"`; `priority_index`: order within default rotation
- `range_preference_ft`: `>= 60`

VFX/SFX (debug/viz stubs)
- `vfx.projectile`: `"fire_bolt_trail"`, `vfx.impact`: `"fire_bolt_impact"`
- `sfx.cast`: `"fire_bolt_cast"`, `sfx.impact`: `"fire_bolt_hit"`

—

## C) JSON (drop‑in example for sim‑data)

```json
{
  "id": "wiz.fire_bolt.srd521",
  "name": "Fire Bolt",
  "version": "1.0.0",
  "source": "SRD 5.2.1",
  "school": "evocation",
  "level": 0,
  "classes": ["wizard", "sorcerer"],
  "tags": ["projectile", "direct-damage", "fire", "ranged-spell-attack"],

  "cast_time_s": 1.0,
  "gcd_s": 1.0,
  "cooldown_s": 0.0,
  "resource_cost": null,
  "can_move_while_casting": false,

  "targeting": "unit-or-object",
  "requires_line_of_sight": true,
  "range_ft": 120,
  "minimum_range_ft": 0,
  "firing_arc_deg": 180,

  "attack": {
    "type": "ranged_spell_attack",
    "rng_stream": "attack",
    "crit_rule": "nat20_double_dice"
  },

  "damage": {
    "type": "fire",
    "add_spell_mod_to_damage": false,
    "dice_by_level_band": {
      "1-4": "1d10",
      "5-10": "2d10",
      "11-16": "3d10",
      "17-20": "4d10"
    }
  },

  "projectile": {
    "enabled": true,
    "speed_mps": 40.0,
    "radius_m": 0.1,
    "gravity": 0.0,
    "collide_with": ["first_target_hit"],
    "spawn_offset_m": { "x": 0.0, "y": 1.6, "z": 0.5 }
  },

  "secondary": {
    "ignite_on_flammable_object": true
  },

  "latency": {
    "uses_actor_input_delay": true,
    "server_tick_ms": 50,
    "queue_window_ms": 100
  },

  "events": [
    "CastStarted",
    "CastCompleted",
    "ProjectileSpawned",
    "HitResolved",
    "DamageApplied",
    "ObjectIgnited"
  ],

  "metrics": {
    "collect": ["casts", "hits", "misses", "crits", "damage_total", "damage_mean", "proj_travel_ms"]
  },

  "policy": {
    "role": "dps",
    "priority_index": 1,
    "range_preference_ft": 60
  },

  "vfx": { "projectile": "fire_bolt_trail", "impact": "fire_bolt_impact" },
  "sfx": { "cast": "fire_bolt_cast", "impact": "fire_bolt_hit" },

  "provenance": {
    "notes": [
      "Evocation cantrip, Action, 120 ft, V S, Instantaneous — SRD 5.2.1",
      "Ranged spell attack: on hit 1d10 fire; ignites flammable, unworn objects",
      "Scaling: +1d10 at levels 5, 11, 17"
    ],
    "citations": [ { "ref": "SRD 5.2.1 — Fire Bolt" } ]
  }
}
```

—

## Notes & Defaults

- Units: store canonical `range_ft=120`; engine exposes meters via a constant conversion.
- Crits: if your model supports spell crits on attack rolls, use `nat20_double_dice`; otherwise set `"none"`.
- Projectile speed: 40 m/s is a good visual baseline; tune per ability later.
- Ignition: gate on `flammable=true` and `is_worn_or_carried=false`. SRD does not specify damage‑over‑time; treat as cosmetic or attach a generic object‑burn DoT as a separate system.
- Tick alignment: resolve cast end on the next tick boundary; spawn projectile then; resolve hit at impact time snapped to tick for determinism.
- RNG: document that Fire Bolt consumes one roll from the actor’s `attack` stream per cast in the current model.

## Minimal Unit Tests (deterministic)

1) Range clamp: target at 121 ft → invalid; at 120 ft → valid.
2) Scaling bands: level 1 → 1d10; level 5 → 2d10; level 11 → 3d10; level 17 → 4d10.
3) Ignition rule: object(flammable=true, worn=false) hit ⇒ `ObjectIgnited`; same object if worn=true ⇒ no ignition.
4) Tick determinism: with `tick=50 ms`, `cast_time=1.0 s`, projectile 40 m/s at 30 m → impact exactly 1.75 s (after tick snapping). Same seed ⇒ identical outcomes.
