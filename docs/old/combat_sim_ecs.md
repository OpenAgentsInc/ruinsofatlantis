# Combat Simulation System — ECS Design and SRD Mapping

Scope: expands the GDD’s Combat Simulator & Harness into a concrete ECS‑oriented plan. Targets a deterministic, fixed‑timestep simulation suitable for MMO server authority and offline balance sweeps. Assumes a rudimentary in‑house ECS (entity IDs + component stores + a scheduled system pipeline).

Goals
- Deterministic outcomes across machines given identical seeds and scenarios.
- Faithful SRD 5.2.1 combat mapping (attacks, saves, conditions, concentration, damage types) with MMO real‑time pacing.
- Data‑driven abilities (see `docs/design/spells/fire_bolt.md`) and pluggable AI policies.
- Clear separation: sim‑core (no rendering) vs. client/viz.

—

## Architecture Overview

- Fixed timestep: e.g., `TICK = 50 ms` (20 Hz). All time math quantizes to ticks.
- Event bus: append‑only per‑tick; systems publish/consume typed events (`CastStarted`, `HitResolved`, `DamageApplied`, ...).
- RNG: per‑actor named streams (e.g., `attack`, `save`) + one environment stream to ensure determinism.
- Data: `sim-data` (JSON/TOML) for abilities, conditions, and items; stable IDs + provenance.
- Harness: headless runner loads a scenario, executes N ticks, emits metrics/logs, optional TUI/viz.

Recommended crate split (future workspace)
- `core/game-data`: serde models + loaders for spells/abilities, conditions, items, classes, monsters (consumes `data/`).
- `core/rules`: SRD math/constants (dice, advantage/disadvantage, crits, save DCs, underwater modifiers).
- `core/combat`: production combat model (FSM for actions/casts/channels/reactions, damage model, condition application, concentration links).
- `core/ecs` (optional): shared ECS infra if we outgrow the current lightweight `src/ecs`.
- `sim-core`: deterministic systems over ECS (events bus, scheduler, RNG streams, attack/save/damage/conditions/projectiles/threat/metrics).
- `sim-policies`: AI policies/behavior trees and utilities.
- `sim-harness`: CLI runner + scenarios + exporters.
- `sim-viz` (optional): simple orthographic debug renderer or TUI.

—

## ECS Model

Entities
- Opaque `EntityId` for players, NPCs, projectiles, and objects.

Core components
- `Transform`: position (2D/3D), facing, optional velocity.
- `Team`: faction/relationship for targeting/aggro.
- `Stats`: ability scores, proficiency bonus, armor class base, movement speeds (walk/swim/fly).
- `Resources`: hit points, temp HP, class resources (e.g., slots), stamina/mana (if modeled).
- `Resistances`: damage resist/vuln/immune tables.
- `Inventory/Equipment`: weapon/armor properties (reach, range, underwater flags).
- `AbilityBook`: list of known abilities with runtime cooldowns/charges.
- `Controller`: player input or AI policy handle.
- `CastBar`: active cast/channel, time remaining, interrupt flags.
- `ThreatTable`: per‑target threat values.
- `Statuses`: active conditions with stacks, sources, durations; marks concentration link.
- `Projectile`: kinematics + source/ability metadata (for projectile entities).
- `LifeState`: alive/downed/dead; death processing flags.

Event types (bus)
- `CastStarted/Completed/Interrupted`
- `ProjectileSpawned/Updated/Despawned`
- `AttackRollRequested/Resolved`
- `SaveRequested/Resolved`
- `HitResolved`
- `DamageApplied`
- `ConditionApplied/Removed`
- `ConcentrationBroke`
- `ObjectIgnited` (see Fire Bolt)
- `Died`

—

## System Pipeline (per tick)

Stage A — Inputs & AI
- `InputCollectSystem`: read player inputs (or scenario scripts).
- `AIPolicySystem`: generate intents (target select, move, cast) for AI actors.

Stage B — Intents → Actions
- `ActionValidationSystem`: clamp ranges/LoS, check resources, verify cooldowns/GCD.
- `CastBeginSystem`: start casts; write `CastStarted`; set `CastBar`.
- `MovementSystem`: integrate velocities; pathing/steering (simple for harness).

Stage C — Cast/Cooldown/Timers
- `CastProgressSystem`: decrement cast bars; on completion, emit `CastCompleted` and enqueue ability effects (e.g., spawn projectile, schedule attack roll) at tick boundary.
- `CooldownSystem`: decrement CDs/GCD; handle charges if any.
- `DurationSystem`: tick statuses; expire/remove as needed.

Stage D — Spatial & Targeting
- `LineOfSightSystem`: resolve LoS blocks vs. simple colliders/geometry.
- `TargetLockSystem`: maintain valid targets; drop if invalid (range, LoS, death).

Stage E — Combat Resolution
- `AttackRollSystem`: consume RNG from actor stream; compute adv/disadv; emit `AttackResolved`.
- `SavingThrowSystem`: for save‑based effects; consume RNG; emit `SaveResolved`.
- `ProjectileSystem`: step projectiles; detect collisions; emit `HitResolved`.
- `DamageSystem`: compute crits, resistances, vulns, immunities; apply HP deltas; emit `DamageApplied` (snap to tick).
- `ConditionSystem`: apply/remove conditions; concentration checks on damage (DC 10 or half damage, rounded down; SRD).
- `DeathSystem`: transition to `dead`; clear concentration; drop aggro.

Stage F — Post‑Combat
- `ThreatSystem`: update per‑target threat from damage/heal/taunt.
- `MetricsSystem`: accumulate counters/histograms.
- `CleanupSystem`: clear per‑tick scratch, retire events.

Order is important for determinism; keep within a static schedule.

—

## SRD Mechanics Mapping (real‑time)

- Attack rolls vs. AC: `AttackRollSystem` handles d20 + modifiers; advantage/disadvantage uses two draws from the same `attack` stream, pick best/worst.
- Critical hits: natural 20 doubles damage dice (configurable per ability; see Fire Bolt doc).
- Saving throws: `SavingThrowSystem` requests target ability saves vs. DC. Advantage/disadvantage handled like attack rolls.
- Conditions: standardized IDs (e.g., `blinded`, `charmed`, `frightened`, `grappled`, `poisoned`, `restrained`, `stunned`). Apply as `Statuses` with sources and durations. Diminishing returns (MMO) can be an overlay in policies, not a rules change.
- Concentration: tracked per caster; taking damage triggers a Con save (DC 10 or half damage); on fail, emit `ConcentrationBroke` and end linked effects.
- Movement: per SRD; underwater modifiers applied via zone/actor flags. Opportunity Attacks modeled via a `ReactionWindow` in `ActionValidationSystem` + `AttackRollSystem`.
- Components/Verbal/Somatic/Material: for sim, validate prerequisites as tags/resources on abilities; defer complex material handling.
- Action economy: map SRD Action/Bonus/Reaction to real‑time via `gcd_s`, `cooldowns`, and `reaction windows` (AoO, Shield) with independent budget.

—

## Geometry, LoS, and Zones

- Sim harness keeps geometry simple: 2D plane with axis‑aligned boxes or navmesh cells. LoS uses ray vs. blockers per tick.
- Zones apply environmental modifiers: underwater flags, light level (affects Perception/Stealth), hazardous tiles (vents, spores), gravity.
- Underwater rules: disadvantage for certain melee/ranged per SRD; resistance to fire damage; baked as zone/weapon tags.

—

## Data‑Driven Abilities

- Abilities live in `sim-data` and are loaded into `AbilityBook` per actor.
- See `docs/design/spells/fire_bolt.md` for a full example (ranged spell attack + projectile + ignition effect).
- MVP set: basic weapon attack, Fire Bolt, Cure Wounds/Healing Word (healing + concentration check interactions), Grease (AoE + prone/save), Shield (reaction), Bless (concentration aura).

—

## Threat & Aggro

- Each NPC maintains a `ThreatTable` keyed by attacker ID.
- Threat inputs: damage (1x), taunts (large snap, diminishing), healing (reduced), proximity (optional leak).
- Target selection: highest effective threat with simple stickiness and melee range bias.

—

## Determinism & RNG

- One fixed system schedule; no parallel mutation.
- All randomness consumes from stable streams: per‑actor `{attack, save, loot}` and one `env` stream.
- Time snaps: cast completion and projectile impacts align to tick boundaries to avoid FP drift.

—

## Metrics & Testing

- Metrics: per‑fight TTK, DPS/HPS, saves made/failed, crit rate, concentration breaks, distance moved, ability usage.
- Unit tests: rollbands, range/LoS validation, condition application/removal, concentration DC math, threat updates.
- Scenario tests: deterministic replay by seed; expect identical logs across runs.

—

## Minimal Roadmap (Phased)

Phase 1 — Core loop
- Fixed‑tick scheduler, event bus, RNG streams.
- Components: Transform, Stats, Resources, AbilityBook, CastBar, Statuses, Team.
- Systems: Input/AI, CastBegin/Progress, AttackRoll, Damage, Condition, Death, Metrics.
- Abilities: Weapon Attack, Fire Bolt.

Phase 2 — Mobility & Spatial
- Movement, LoS, simple collision; Projectile system.
- Underwater zone modifiers; opportunity attacks.

Phase 3 — Spells & Concentration
- Bless (aura + concentration), Shield (reaction), Grease (AoE + saves/prone).
- Concentration save on damage; status durations.

Phase 4 — Threat & Bossing
- Threat tables, taunts, basic boss AI policies.
- Harness: Monte Carlo sweeps; CSV exporters.

—

## Example Event Flow (Fire Bolt)

1) `CastBeginSystem`: validates range/LoS; emits `CastStarted`; sets `CastBar{1.0 s}`.
2) `CastProgressSystem`: at end, emits `CastCompleted`; spawns `Projectile` with speed and origin; emits `ProjectileSpawned`.
3) `ProjectileSystem`: advances; on collision, requests attack roll.
4) `AttackRollSystem`: draws from actor `attack` stream; resolves hit/crit; emits `HitResolved`.
5) `DamageSystem`: rolls dice by level band; applies `fire` type; emits `DamageApplied`; if object target and flammable/unworn, emit `ObjectIgnited`.
6) `MetricsSystem`: updates counters; `ThreatSystem` updates threat.

This aligns with the Fire Bolt spec while staying general for other abilities.

—

## Folder Hierarchy (Core naming)

Authoritative data (checked in, shared)
- `data/`
  - `spells/`, `abilities/`, `conditions/`, `items/`, `monsters/`, `classes/`, `feats/`
  - `encounters/` (optional), `scenarios/` (for sim harness inputs)
  - Optional `manifest.(json|toml)` for indexing/versioning

Transitional single-crate layout (now)
- `src/core/`
  - `data/` — serde types + loaders (read from top-level `data/`)
    - `ability.rs`, `spell.rs`, `condition.rs`, `monster.rs`, `item.rs`, `class.rs`, `ids.rs`, `loader.rs`
  - `rules/` — SRD math/constants
    - `dice.rs`, `attack.rs`, `saves.rs`, `mod.rs`
  - `combat/` — production combat types
    - `fsm.rs` (cast/channel/reaction windows, GCD/CD), `damage.rs`, `conditions.rs`, `mod.rs`
- `src/sim/` — deterministic runtime (engine only; consumes `crate::core::*` and `crate::ecs`)
  - `events.rs`, `rng.rs`, `scheduler.rs`, `types.rs`
  - `components/` (CastBar, AbilityBook, Statuses, Projectile, Threat, Controller; reuse `ecs::Transform`)
  - `systems/` (input/AI, cast begin/progress, attack/save, projectile, damage, condition, concentration, death, threat, metrics)
  - `policies/`
  - `data/` (only scenario serde for the harness)
- `src/bin/sim_harness.rs` — CLI runner (loads scenarios; outputs CSV/JSON/NDJSON)

Target workspace layout (later)
- `core/game-data`, `core/rules`, `core/combat`, `core/ecs` (optional)
- `sim-core`, `sim-policies`, `tools/sim-harness`, `sim-viz` (optional)
- `client/`, `server/` consume `core/*` and may embed `sim-core` (server authority/tests)

What goes where
- Game facts/schema + loaders → `core/game-data` (now: `src/core/data/*`)
- SRD math → `core/rules`
- Production combat model (FSM, damage, conditions) → `core/combat`
- Deterministic runtime (systems, events, RNG, scheduler) → `sim-core` (now: `src/sim/*`)
- Author content → `data/*`
- Tool-specific configs (only for a tool) → tool folder; sim scenarios commonly live in `data/scenarios/` so multiple tools can reuse them.

## Allies & Targeting (policy)

- Party/guild/raid membership is represented as a `Team`/`team` label; allied members cannot be targeted by hostile actions.
- Systems enforce ally immunity by skipping harmful resolution (`attack_roll`, `saving_throw`, `damage`) when `are_allies(a, b)` is true.
- Duels/wars can temporarily override by switching team membership or flagging exceptions in scenario/policy.

---

Here’s a clean hierarchy that keeps “authoritative game data + rules” shared, and lets the
  sim (FSM/engine) pull them in.

  Authoritative Data (checked in, shared)

  - data/
      - spells/ JSON/TOML (e.g., fire_bolt.json)
      - abilities/ (if you keep non‑spell abilities separate)
      - conditions/
      - items/, monsters/, classes/, feats/
      - encounters/ (optional authored sets)
      - scenarios/ (inputs for sim harness; you can also put this under tools/sim-harness/scenarios/ if you want it tool‑local)
      - manifest.(json|toml) (optional index with versions/provenance)

  The sim, server, and client all load from data/ via shared loaders.

  Target Workspace Layout (when we split crates)

  - shared/
      - game-data/ (serde models + loaders)
          - Types: Ability, Spell, Condition, Item, Monster, Class, IDs, provenance
          - Loaders: resolve data/ paths, versioning, validation
      - rules/ (SRD math and constants)
          - Dice, advantage/disadvantage, crit rules, save DC math, underwater modifiers
      - ecs/ (lightweight ECS infra if we outgrow current src/ecs/)
      - combat/ (production combat types)
          - fsm/ Ability/Action state machines (cast/channel/reaction windows, GCD/CD), events
          - Damage model, condition application, concentration links
      - sim-core/ (deterministic systems over ECS)
          - Events bus, scheduler, RNG streams, systems (attack, save, damage, conditions, projectiles, threat)
      - ai/ (policies/behavior trees)
  - tools/
      - sim-harness/ (CLI; consumes shared crates, reads data/scenarios/*)
      - gltf-decompress/ (already present as src/bin/gltf_decompress.rs, later migrate here)
  - client/ (renderer, input, streaming)
  - server/ (authoritative host; uses shared/* and can embed sim-core)

  This puts all production types/data in shared/* and data/. The sim harness depends on them; it owns zero game facts.

  Transitional (single-crate now, minimal churn)
  Until we flip to a workspace, add a shared module tree the sim can import directly:

  - src/shared/
      - mod.rs
      - data/ (serde types + loaders reading data/)
          - ability.rs, spell.rs, condition.rs, monster.rs, item.rs, class.rs, loader.rs, ids.rs
      - rules/ (SRD math/constants; dice, adv/disadv, crits, saves, underwater)
          - mod.rs, dice.rs, attack.rs, saves.rs
      - combat/
          - fsm.rs (cast/channel/interrupt/cooldown state; reaction windows)
          - damage.rs (types, resist/vuln/immune), conditions.rs (apply/remove, concentration links)
  - src/sim/ (engine only; consumes shared)
      - events.rs, rng.rs, scheduler.rs, types.rs
      - components/ (sim runtime: CastBar, AbilityBook, Statuses, Projectile, Threat, Controller; reuse src/ecs/mod.rs:1 Transform)
      - systems/ (input/AI, cast begin/progress, attack/save, projectile, damage, condition, concentration, death, threat, metrics)
      - policies/
      - data/ (only scenario serde, not game facts)
  - src/bin/sim_harness.rs (CLI; loads from data/ + shared::data::loader)

  This keeps production definitions under src/shared/* now, which later maps 1:1 into shared/* crates without rewrites. The sim only imports from crate::shared::*.

  What goes where (rule of thumb)

  - Game facts/schema (serde types, loaders) → shared/game-data (now: src/shared/data/*)
  - SRD math and constants → shared/rules (now: src/shared/rules/*)
  - Production combat model (FSM, damage, conditions) → shared/combat (now: src/shared/combat/*)
  - Deterministic runtime (systems, events, RNG, scheduler) → shared/sim-core (now: src/sim/*)
  - Author content → data/*
  - Tool‑specific configs (only for running a tool) → tool folder; sim scenarios can live under data/scenarios/ to reuse across tools.

  Concrete next steps

  - Create src/shared/data/{ability.rs,spell.rs,condition.rs,loader.rs} with docblocks; point loaders at data/.
  - Add src/shared/rules/{dice.rs,attack.rs,saves.rs} and src/shared/combat/fsm.rs.
  - Have the sim (and later server) import from crate::shared::*.
  - Place Fire Bolt JSON under data/spells/fire_bolt.json per docs/design/spells/fire_bolt.md; wire the loader.
  - Add src/bin/sim_harness.rs that glues shared::data + shared::rules + shared::combat + sim together.

  If you want, I can scaffold the src/shared/* modules and a minimal sim_harness entry so you can start writing the FSM types in their final home.
