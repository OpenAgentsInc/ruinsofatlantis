# 100: Nivita, Lady of Undertide — Unique Boss NPC

Status: In Progress (MVP wired; server spawn + stats + log)

Owners: Gameplay/Systems (server_core, sim_core, data_runtime), Graphics (render_wgpu integration)

Summary
- Define and introduce a unique raid boss NPC, “Nivita, Lady of Undertide” (Nivita), as a single-instance named enemy with configured stats, resistances, saves, action economy, spells, and legendary actions.
- MVP goal: spawn and track a unique Sorceress boss entity with correct HP/AC/resistances and a minimal spellbook stub; basic movement already exists client-side for the Sorceress visual — fold this into an ECS/server definition and data-driven config.

Motivation
- We currently spawn a Sorceress model on the client for a demo. This needs to become a proper, authoritative boss in our ECS/server pathway with deterministic stats and a path toward real spellcasting and legendary actions.

Scope (MVP → Follow-ups)
- In-scope (MVP):
  - Data model for Nivita’s core stats (HP, AC, saves, resistances, immunities, legendary resistance charges).
  - Unique identity and spawn (only one). Tagging so systems can treat her specially.
  - Minimal spellbook representation and action economy scaffolding (cast slot + bonus-action cantrip placeholder hooks; no full spell effects yet).
  - Server-side spawn in scene build; replication-ready components (as applicable) without renderer mutation.
  - Integration with existing Sorceress visual path (renderer) with clear separation of authority.
- Out-of-scope (later issues):
  - Full implementation for each spell’s behavior/effects.
  - Legendary action scheduler and per-round action economy.
  - Boss AI phases, targeting logic, and advanced navigation/avoidance.
  - Client VFX/animation blending per ability.

Design

1) Data Schema (data_runtime)
- New model for named unique NPCs with boss stats.
- File: data_runtime (new) `configs/npc_unique.rs`; `nivita.toml` under `data/config/`.
- Fields (initial):
  - `name` (string), `id` (string, key, e.g., `boss_nivita`)
  - `level` (u8)
  - `hp_range` (tuple `[min, max]`, e.g., `[200, 250]`)
  - `ac` (u8, e.g., 17–18)
  - `saves` (struct): `str, dex, con, int, wis, cha` modifiers; strong INT/WIS, weak STR/CON
  - `resistances` (list of DamageType): `necrotic`, `psychic`
  - `immunities` (list of Condition): `charm`, `fear`
  - `legendary_resistances_per_day` (u8: 2–3)
  - `speed_mps` (f32)
  - `radius_m` (f32)
  - `spellbook` (list of spell ids; see below)
  - `legendary_actions` (list of ids/costs)

Example `data/config/nivita.toml` (MVP skeleton):
```toml
[npc]
id = "boss_nivita"
name = "Nivita, Lady of Undertide"
level = 10
hp_range = [200, 250]
ac = 18

[saves]
str = -1
dex = 1
con = 0
int = 5
wis = 4
cha = 3

resistances = ["necrotic", "psychic"]
immunities  = ["charm", "fear"]
legendary_resistances_per_day = 3

speed_mps = 1.2
radius_m = 0.9

[spellbook]
cantrips = ["chill_touch", "eldritch_blast", "minor_illusion", "toll_the_dead"]
level_1_3 = ["counterspell", "fireball", "animate_dead", "fear", "fly"]
level_4_5 = ["blight", "greater_invisibility", "wall_of_force", "dominate_person"]
signature = ["circle_of_death", "finger_of_death", "soul_flay"]

# Legendary actions with action point cost per use
[[legendary_actions]]
id = "grave_pulse"
cost = 1

[[legendary_actions]]
id = "command_undead"
cost = 1

[[legendary_actions]]
id = "shatter_reality"
cost = 2

[[legendary_actions]]
id = "soul_drain"
cost = 3
```

2) ECS Components (ecs_core)
- New components (replication-friendly):
  - `Name` (string) or `Named` wrapper to surface display name.
  - `Unique` (unit tag) to assert single-instance.
  - `ArmorClass { ac: i32 }`.
  - `SavingThrows { str, dex, con, int, wis, cha: i8 }`.
  - `Resistances { dmg: SmallVec<DamageType>, immune: SmallVec<Condition> }` (or two separate components).
  - `LegendaryResistances { per_day: u8, remaining: u8 }`.
  - `Spellbook { /* buckets: cantrips, 1-3, 4-5, signature */ }` minimal identifiers.
  - Reuse existing `Health`, `Team`, `Velocity`, and `Npc` (radius/speed) for movement.
- Supporting enums:
  - `DamageType` (necrotic, psychic, fire, force, etc.)
  - `Condition` (charmed, frightened, etc.)

3) Server Integration (server_core)
- Loader: read `nivita.toml` via data_runtime; provide `spawn_nivita()` that instantiates an entity with the above components set, once per scene/session.
- Ensure uniqueness: if already present, don’t respawn; or respawn only on scene reset.
- MVP behavior:
  - Movement: keep current Sorceress slow walk toward wizards (already client-side for demo). For server authority, set `Velocity` toward nearest wizard and integrate position (headless-friendly); client mirrors via replication later.
  - Action economy scaffolding: track “main action cast” vs “bonus-action cantrip” intents per round; no actual spell effects beyond event/logging placeholders.
  - Legendary resistance charges decrement on failed save (hook later when saves are processed).

4) Renderer Bridging (render_wgpu)
- Keep Sorceress visual path, but migrate to read from a `Nivita` entity proxy once available (position + animation state in a future task). For now, retain the demo motion with clear logs that the server is the intended source of truth.

5) Spells/Abilities (sim_core + data_runtime)
- Reconcile spell IDs with `data_runtime` spec DB (existing or new). MVP: identifiers only; actual effects later.
- Add placeholder entries for signatures:
  - `soul_flay`: psychic blast; INT save → confuse/lose control for 1 round (to be implemented later).
  - Legendary actions: define effect stubs and costs.

6) Testing
- Unit tests:
  - Data load from `nivita.toml` with defaults and overrides.
  - ECS construction: components attached as expected; `LegendaryResistances.remaining == per_day` on spawn.
  - Server `spawn_nivita()` idempotence (unique).
- Integration (headless): minimal tick that advances movement toward a target; ensure position changes and remains bounded by speed.

7) Telemetry
- Emit structured logs on spawn and per-round action economy decisions (off by default in CI).
- Counters: `boss.nivita.spawns_total`, `boss.nivita.legendary_resists_used_total`.

Action Items
- data_runtime
  - Add `configs/npc_unique.rs` with loader for `nivita.toml`.
  - Add `data/config/nivita.toml` starter based on the example.
- ecs_core
  - Add components: `Name`, `Unique`, `ArmorClass`, `SavingThrows`, `Resistances`, `LegendaryResistances`, `Spellbook` (+ enums for `DamageType`, `Condition`).
- server_core
  - Add `spawn_nivita()` and call it from scene build at a sensible location (behind Death Knight spawn).
  - Minimal movement system using `Velocity` toward nearest wizard.
- render_wgpu (later follow-up)
  - Bridge Sorceress visual to Nivita entity state (position, animation).

Acceptance Criteria (MVP)
- `cargo test` includes unit tests for config load and spawn uniqueness.
- Running the app shows the Sorceress labeled/logged as Nivita, with HP/AC/resistance data loaded. She walks slowly toward the wizards as before.
- No client-side mutation for gameplay state; server remains source of truth for Nivita’s stats.

Notes
- SRD alignment: spell names reference SRD 5.2.1 terms; continue to keep `NOTICE` accurate. Custom ability “Soul Flay” is non-SRD and should be documented in `GDD.md` when implemented.

Addendum (current change set)
- ECS: added components in `ecs_core` — `Name`, `Unique`, `ArmorClass`, `SavingThrows`, `Resistances`, `Immunities`, `LegendaryResistances`, `Spellbook`, plus enums `DamageType` and `Condition`. Basic unit test for `LegendaryResistances`.
- Data: added loader `data_runtime::configs::npc_unique` and `data/config/nivita.toml` with the proposed stats, saves, resistances, immunities, spellbook, and legendary actions.
- Converters: added `parse_damage_type()` and `parse_condition()` in data_runtime for clean enum mapping.
- Server: `server_core::ServerState::spawn_nivita_unique(pos)` loads config, spawns a single boss NPC with midpoint HP, radius, and speed, and stores a `NivitaStats` snapshot (Abilities/Saves/Defenses/Legendary/Spellbook). Adds `nivita_status()` for HUD/replication and logs a concise spawn line; increments `boss.nivita.spawns_total`.
- Renderer: on init, spawns Nivita server-side at the Sorceress position and logs a one-liner with name/hp/ac. On each frame, if `nivita_status()` is present, the Sorceress visual follows the server position (server-authoritative motion); otherwise it falls back to the previous local demo walk. No gameplay mutation client-side.
- Decoupling: moved string→enum converters into `ecs_core::parse` and removed the temporary `ecs_core` dependency from `data_runtime` to keep loaders ECS‑agnostic.
- Spawn location: moved the actual `spawn_nivita_unique` call out of renderer init and into server bootstrap (`gfx/npcs.rs::build()`), so the renderer only reads status and follows. The server still logs spawn and counts metrics.
- Aliases: `ecs_core::parse::parse_condition` accepts common aliases (e.g., "fear"→Frightened, "charm"→Charmed) so TOML is resilient.
- Saves: default derivation now adds proficiency to INT/WIS/CHA when `[saves]` isn’t provided.

Next Actions
- Replicate `BossStatus` to client (thin snapshot) and surface a HUD label; keep renderer non‑authoritative.
- Map `team` to a numeric/team table where AI expects it; attach a capsule shape when the ECS world is wired.
- Next: optional thin replication to surface name/hp/ac in HUD; later, drive Sorceress visuals from server entity state.
