# Combat Simulation — Quickstart and Structure

This document explains the scaffolding added for the deterministic combat simulator, how to run it, where data lives, and how to extend it. It complements the design doc in `docs/combat_sim_ecs.md` by focusing on practical usage.

## What’s Included

- Core (production types to be shared across client/server/sim)
  - `src/core/data/*`: serde models and loaders for game data (spells, scenarios, …)
  - `src/core/rules/*`: SRD math scaffolding (dice/attack/saves)
  - `src/core/combat/*`: combat FSM, damage/conditions enums
- Sim runtime (deterministic engine; no rendering)
  - `src/sim/*`: events, RNG, fixed-tick scheduler, components, and system stubs
- Harness (CLI to run scenarios)
  - `src/bin/sim_harness.rs`: parses a scenario, loads data (e.g., Fire Bolt), and will execute the sim loop as systems are filled in
- Authorable data (checked in, shared)
  - `data/spells/fire_bolt.json`: SRD-accurate Fire Bolt spec (see `docs/fire_bolt.md`)
  - `data/scenarios/example.yaml`: sample scenario for the harness

## Run the Simulator

- Single run: `cargo run --bin sim_harness -- --scenario data/scenarios/example.yaml`
- The harness prints a summary of the scenario and demonstrates loading Fire Bolt from `data/spells/`.

File references
- Harness entry: `src/bin/sim_harness.rs:1`
- Scenario example: `data/scenarios/example.yaml`
- Fire Bolt data: `data/spells/fire_bolt.json`

## Scenario Format (YAML)

Minimal shape (see `data/scenarios/example.yaml`):

```yaml
name: Aboleth Demo
tick_ms: 50
seed: 42
map: flooded_ruin
underwater: true
actors:
  - id: wizard_ctrl
    role: dps
    class: wizard
    abilities: ["wiz.fire_bolt.srd521", "grease"]
```

- `tick_ms`: fixed tick for determinism (default 50 ms)
- `seed`: RNG seed for reproducible runs
- `underwater`: zone flag for SRD underwater modifiers
- `actors`: list of participants with IDs, roles, and ability IDs

Loader: `src/core/data/scenario.rs:1`

## Game Data and Loaders

- SpellSpec (serde model): `src/core/data/spell.rs:1`
- Loader helper: `src/core/data/loader.rs:1` (e.g., `load_spell_spec("spells/fire_bolt.json")`)
- Example data: `data/spells/fire_bolt.json` (matches `docs/fire_bolt.md`)

Notes
- Keep authorable, shared data under top-level `data/` so client/server/sim read the same sources.
- IDs should be stable (e.g., `wiz.fire_bolt.srd521`).

## FSM Basics (production model)

- Types: `src/core/combat/fsm.rs:1`
  - `ActionState` (Idle, Casting, Channeling, Recovery)
  - `Gcd` (global cooldown budget)
  - `ReactionWindow` (e.g., Shield, Opportunity Attacks)
- Helpers
  - `ActionState::tick(dt_ms)` → advances timers and reports completions
  - `ActionState::try_start_cast(ability, cast_time_ms, gcd, gcd_ms)` → enforces GCD/Busy

This FSM is owned by `core` so production logic and sim share one source of truth.

## Sim Runtime (engine)

- Events: `src/sim/events.rs:1`
- RNG streams: `src/sim/rng.rs:1` (to be fleshed out for per-actor streams)
- Fixed-tick scheduler: `src/sim/scheduler.rs:1`
- Components: `src/sim/components/*` (runtime-only: CastBar, AbilityBook, Statuses, Projectile, Threat, Controller)
- Systems (stubs): `src/sim/systems/*` (input/AI, cast begin/progress, attack roll, damage, projectiles)

These will implement the pipeline described in `docs/combat_sim_ecs.md`.

## Allies & Targeting

- Ally immunity: hostile actions (attacks, harmful spells, conditions) do not affect allies in your party/guild/raid (modeled as `team`).
- AoE: area effects ignore allied members by default.
- Overrides: duels/wars can explicitly lift immunity for scoped participants.
- Sim: actors can specify `team` in scenarios; the engine skips hostile resolution when `are_allies()` is true.

## Extending the System

Add a new spell/ability
- Author data at `data/spells/<id>.json` following the Fire Bolt schema
- Load with `core::data::loader::load_spell_spec("spells/<id>.json")`
- Reference the ability ID in a scenario’s `actors[].abilities`

Implement combat logic
- Fill out systems in `src/sim/systems/` per the pipeline (cast → projectile/attack → damage → statuses/threat)
- Use `core::rules::*` for SRD math and `core::combat::fsm` for state
- Keep all randomness via per-actor RNG streams for determinism

Iterate on scenarios
- Copy `data/scenarios/example.yaml` and adjust actors/abilities/flags
- Run multiple seeds or Monte Carlo later (planned in harness)

## Determinism Notes

- Fixed tick (default 50 ms) snaps all timing (cast completion, projectile impacts)
- RNG streams will be per-actor + environment to ensure identical outcomes given the same seed
- Event ordering stays stable by using a static system schedule

## Roadmap (short)

- Implement real RNG streams and the event bus
- Wire cast/attack/damage/condition systems with metrics
- Add more data specs (Bless, Shield, Grease) and a few monsters
- Extend scenario format with initial positions and policy configs

## Dev Commands

- Build: `cargo check`
- Run client (renderer): `cargo run`
- Run sim harness: `cargo run --bin sim_harness -- --scenario data/scenarios/example.yaml`
- Format/lint: `cargo fmt` and `cargo clippy -- -D warnings`
