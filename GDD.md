# Ruins of Atlantis Game Design Document

Ruins of Atlantis is a fantasy MMORPG under development by Blue Rush Studios, a division of OpenAgents, Inc.

## Contents

- [Philosophy](#philosophy)
- [Game Mechanics](#game-mechanics)
- [SRD Usage and Attribution](#srd-usage-and-attribution)
- [SRD Scope & Implementation](#srd-scope--implementation)
- [Classes](#classes)
- [Races](#races)
- [Class Lore](#class-lore-oceanic-context)
- [Combat](#combat)
- [Player vs. Player (PvP)](#player-vs-player-pvp)
- [Combat Simulator & Harness](#combat-simulator--harness)
- [Zones & Cosmology](#zones--cosmology)
- [Progression Matrix (Zones × Classes, Land Drama)](#progression-matrix-zones--classes-land-drama)
- [Faction Framework](#faction-framework)
- [Technical Overview](#technical-overview)

## Philosophy

RoA embraces old‑school difficulty. Travel and logistics matter, resources are scarce, choices are meaningful, and death is painful.

Diegetic interactions, not toggles. If a thing exists in the world—player, NPC, door, ship—you can use the same verbs on it: look, talk, trade, shove, steal, heal, curse, or strike. We do not add out‑of‑world switches like “PvP Enabled/Disabled” or invulnerability bubbles. Safety and norms are enforced in‑world (laws, wards, factions, guard response, bounties), not by breaking the simulation.

## Game Mechanics

Built on Dungeons & Dragons 5th Edition (SRD): iconic classes, races, spells, monsters, and d20 combat—fully implemented and tuned for a dangerous, persistent MMO world.

## SRD Usage and Attribution

This project uses material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.

This is an unofficial, D&D‑inspired game. You may describe it as "compatible with fifth edition" or "5E compatible." It is not affiliated with, endorsed, or sponsored by Wizards of the Coast. See the NOTICE file for full attribution details.

## SRD Scope & Implementation

Goal: near‑complete implementation of SRD 5.2.1 content, adapted for an MMO.

- Core Rules: abilities, proficiencies, d20 tests, conditions, damage types, movement, combat, rests, spellcasting, and leveling/proficiency progressions.
- Classes & Subclasses: all SRD classes and the subclasses contained in SRD 5.2.1; class features follow SRD rules unless MMO adjustments are explicitly documented.
- Backgrounds & Feats: SRD backgrounds, origin/general feats, and any SRD tables referenced.
- Spells: full SRD spell list (effects, components, durations, scaling, lists by class).
- Equipment & Magic Items: weapons, armor, tools, gear, and SRD magic items with rules‑accurate properties and mastery tags.
- Monsters: complete SRD bestiary with stat blocks, actions, traits, and legendary/lair rules as present.

Implementation notes
- Deviations: use SRD 5.1 term "race" instead of SRD 5.2.1 "species" to align with legacy MMO familiarity.
- Trademarks: avoid non‑SRD proper nouns and Wizards of the Coast trademarks.
- Fidelity first: implement rules verbatim where feasible; MMO‑specific changes (e.g., death penalties, travel weight, matchmaking limits) are documented under Design Differences.
- Data‑driven: represent SRD entities in data (JSON/TOML) with stable IDs and versioning to simplify updates and audits.

## Classes

(Directly from SRD)

- Barbarian
- Bard
- Cleric
- Druid
- Fighter
- Monk
- Paladin
- Ranger
- Rogue
- Sorcerer
- Warlock
- Wizard

## Races

(Directly from SRD)

- Dragonborn
- Dwarf
- Elf
- Gnome
- Goliath
- Halfling
- Human
- Orc
- Tiefling

## Class Lore

Pulled from SRD 5.2.1 classes; original names retained. This section frames how each archetype fits an Atlantis‑ruins, oceanic world and the SRD cosmology without renaming mechanics.

### Martial & Primal Classes
- Barbarian: survivors of storm‑wrecked coasts and open seas; rage as primal fury of currents and crashing waves; often tied to Inner Planes (Water, Storm, Earth).
- Fighter: backbone of naval militias, mercenary crews, and ruin‑delving expeditions; masters of harpoons, boarding pikes, and Atlantean relic‑arms.
- Rogue: smugglers, divers, and treasure‑hunters; adept at navigating wrecks, bypassing Atlantean wards, and thriving in lawless ports.
- Ranger: coastal wardens and beast‑bonded sailors; specialists in sea‑monster hunts across reefs and open water.
- Monk: ascetics who adapt discipline to currents and tides; train near thermal vents or cliff‑side monasteries to channel inner balance.

### Divine & Spiritual Classes
- Cleric: servants of sea gods, storm lords, and drowned ancestors; domains tied to tides, storms, lighthouse light, or stillness of the deep.
- Paladin: holy knights sworn to protect seafarers, coastal settlements, or Atlantean secrets; oaths bind them to Outer Planes patrons of justice or vengeance beneath the waves.
- Druid: stewards of reefs, currents, and the wild ocean; channel balance between sea and land and guide communities toward harmony with nature.

### Arcane & Mystical Classes
- Wizard: scholars of Atlantean relics and rune‑etched ruins; seek forbidden lore in flooded archives or astral tide‑charts.
- Sorcerer: innate casters shaped by planar tides—descended from storm elementals, sirens, or Atlantean bloodlines; magic manifests as waves, lightning, or abyssal whispers.
- Warlock: pact‑bound to beings beyond the Material—drowned gods, abyssal leviathans, or Feywild siren‑queens; gifts pull them toward Shadowfell trenches or Abyssal whirlpools.
- Bard: keepers of seafaring songs and lore; inspire crews, charm spirits, and preserve Atlantean myths through performance.

### Planar Integration
- Material Plane: Fighters, Rogues, Barbarians, Rangers—survivalists and adventurers.
- Feywild: Bards, Druids—music and nature in oceanic reflections.
- Shadowfell: Warlocks, some Rogues and Sorcerers—abyssal influence and drowned dead.
- Inner Planes: Barbarians, Druids, Sorcerers—primal ties to elements and tides.
- Outer Planes: Clerics, Paladins, Monks—divine oaths and cosmic order.
- Astral/Ethereal: Wizards, Warlocks—arcane travel, dream‑sailing, ghostly insight.


## Combat

SRD compliance
- Uses D&D 5th Edition (SRD) mechanics: d20 Attack Rolls vs. AC, Saving Throws vs. spell/save DCs, Advantage/Disadvantage, Conditions, damage types/resistance/vulnerability, critical hits, reach, and reactions (e.g., Opportunity Attack).
- Spellcasting respects SRD Casting Time, Concentration, Components (adapted for MMO UX), and the action economy (Action/Bonus Action/Reaction) mapped to real‑time pacing.

Real‑time adaptation (EverQuest‑style influence)
- Continuous time with global and per‑ability cooldowns; 6‑second “round” is a simulation target, not a literal turn system.
- Cast bars and weapon swing timers; taking damage triggers SRD Concentration checks and can interrupt long casts where appropriate.
- Threat/aggro tables and taunt mechanics guide NPC target selection; positioning and facing matter for some abilities.
- Crowd control (roots, stuns, sleeps, charms) is meaningful in open world; durations may scale by creature rank to preserve fairness.

Design notes
- Keep SRD math intact where possible; any deviations (e.g., flanking/positionals, diminishing returns on CC, movement while casting) are documented as Design Differences and tuned for MMO balance.
- Time‑to‑kill skews longer than typical theme park MMOs; resource management (health, mana, stamina) and downtime drive social play.

### Example Combat: Six‑Player Boss Fight (Aboleth)

Scenario
- Party: Fighter (tank), Cleric (healer), Wizard (control/DPS), Rogue (melee DPS), Monk (melee skirmisher), Ranger (ranged DPS/utility).
- Boss: Aboleth (Legendary aberration; AC 17, HP ~150). Uses tentacles to Grapple, Dominate Mind (2/day), Consume Memories, Legendary Resistance, Legendary Actions (Lash). While underwater, emits a Mucus Cloud that can curse nearby creatures.
- Arena: Flooded ruin with waist‑to‑chest‑deep water, broken platforms, and submerged channels. Portions of the fight happen underwater (Underwater Combat rules apply) as the aboleth dives and surfaces.

What players see (UI)
- Boss frame with AC indicator, Legendary Action pips, and Dominate Mind alert when channeling/triggering.
- Party frames: HP/mana/stamina; Concentration icon that shows DC on damage; charm warning on dominated allies.
- Player HUD: hotbar cooldowns, GCD spinner, weapon swing timer, resources, threat meter. Underwater icon shows if weapon suffers disadvantage under current rules.
- Telemetry: 5‑foot danger ring around the aboleth only when it is underwater (Mucus Cloud at end of its turn).

Pull & Phase 1 (0:00–0:45)
- Fighter opens with Taunt → closes to melee, Shove to turn the boss away; maintains threat with steady swings. Indomitable is reserved for a critical Wis/Int save.
- Cleric pre‑casts Bless (Conc) and Protection from Evil and Good on the Fighter (advantage on saves vs. aberration charm; SRD). Healing Word is kept for movement; positions on a platform.
- Wizard controls space with difficult terrain (e.g., Grease on ramps) and ranged cantrips (Fire Bolt / Ray of Frost). Watches for Dominate Mind to coordinate a response.
- Rogue opens behind the boss after the tank’s first swing to avoid ripping threat; uses Cunning Action to avoid tentacle cones and to break line if targeted.
- Monk engages flank; uses Patient Defense to ride out heavy swings; Flurry of Blows during safe windows; may attempt a stun on add spawns or to create a burst window (if feature available).
- Ranger opens with Ensnaring Strike on boss (Conc; Strength save) to create brief control windows; then sustained ranged DPS; swaps to melee in underwater phases with spear/trident to avoid disadvantage.

Boss behavior
- Multiattack: two Tentacles (15‑ft reach; on hit Grapples, escape DC ~14) plus Consume Memories against a Grappled or Charmed target (Int save for psychic damage; on reducing a Humanoid to 0 HP with this, aboleth gains memories).
- Dominate Mind (2/day): Wis save vs. DC ~16 on a visible creature within 30 ft. Dominated target acts as ally to aboleth; repeats save when it takes damage. Aboleth often targets the Cleric or Ranger.
- Legendary Actions: between turns, uses Lash (Tentacle) to maintain Grapples or threaten backline.
- Mucus Cloud (underwater only): at end of aboleth’s turn, creatures within 5 ft. make a Con save or suffer a curse (can’t regain HP unless underwater; takes periodic acid damage while dry).

Micro interactions (SRD mapped to real‑time)
- Attack rolls vs. AC; crits on 20. Advantage from restraint/positioning; disadvantage for some weapons underwater per SRD.
- Saves: Wis/Int/Con saves shown in UI; Concentration checks for Bless/Ensnaring Strike on damage (DC 10 or half damage).
- Reactions: Opportunity Attacks on movement; Shield (Wizard) and Uncanny Dodge (Rogue) as defensive reactions with short lockouts.

Phase 2 (0:45–1:45): Grapples, Charm, and Dives
- At ~70% HP, aboleth starts diving and surfacing, forcing underwater windows. Melee switch to thrusting weapons (spear/trident/shortsword) to avoid disadvantage.
- Fighter reacts to Tentacle Grapples: uses Shove/Grapple to keep the aboleth oriented; calls for focus to break allies free (escape checks) before Consume Memories.
- Dominate Mind hits the Ranger: Cleric pings the target with a low‑damage cantrip to force a new save; Wizard readies a disabling spell on the dominated ally if needed; party avoids lethal bursts.
- Cleric triage: Healing Word on the move; if Bless drops, re‑establish when safe. Can cast Protection from Evil and Good on a vulnerable ally to blunt further charm attempts.
- Monk uses mobility to tag adds, peel pressure, and interrupt a Lash window (if kit allows). Patient Defense covers dive transitions.
- Wizard prioritizes control and single‑target during spread mechanics; avoids Fireball if allies are Grappled to the boss.

Phase 3 (1:45–end): Legendary Pressure
- Bloodied, the aboleth escalates Lash usage and pairs Grapples with Consume Memories. Legendary Resistance may negate key stuns—party baits it with medium‑impact control before committing major cooldowns.
- Fighter uses Action Surge to stabilize threat after a dive; Indomitable on a failed Dominate save.
- Rogue maintains back position, times burst between Lash windows; Cunning Action to re‑acquire safe angle after knockback drifts.
- Ranger sustains single‑target; refreshes Ensnaring Strike after breaks; positions to maintain line of sight across platforms.
- Cleric commits a big heal window during predictable Grapple+Consume combos; preserves Concentration through incoming damage.

Buttons, timing, and waits (illustrative hotbars)
- Fighter: [1 Taunt (8s cd)] [2 Heavy Strike] [3 Shove] [4 Shield Block (cd)] [Q Second Wind] [E Action Surge] [R Indomitable]
- Cleric: [1 Healing Word] [2 Cure Wounds] [3 Bless (Conc)] [4 Protection from Evil and Good (Conc)] [Q Spare the Dying] [E Turn Undead]
- Wizard: [1 Fire Bolt] [2 Ray of Frost] [3 Grease (Control)] [4 Dispel/Utility] [Q Shield (Reaction)] [E Misty Step]
- Rogue: [1 Basic Strike] [2 Eviscerate] [3 Cunning Action] [Q Uncanny Dodge (Reaction)] [E Evasion]
- Monk: [1 Jab] [2 Flurry (Focus)] [3 Patient Defense (Focus)] [4 Step of the Wind (Focus)]
- Ranger: [1 Aimed Shot] [2 Multi‑Shot] [3 Ensnaring Strike (Conc)] [4 Dash/Disengage] [Q Trapper’s Kit]

Threat & aggro
- Threat accrues from damage, taunts, and healing (reduced). Hard taunt briefly snaps target (diminishing if spammed). Threat tables are visible to teach management.

Failure and recovery
- If Dominate Mind lands on the Cleric and the party fails to break it, healing collapses rapidly. Answer: controlled damage on the charmed ally to force a save; kite while stabilizing.
- If multiple players end the aboleth’s underwater turn within 5 ft., Mucus Cloud curses the frontline. Answer: fight submerged during heal windows or rotate moistening items/abilities; avoid ending turns in the 5‑ft ring when boss is underwater.

### Underwater Combat: Quick Reference

SRD rules (5.2.1)
- Movement: without a Swim Speed, each foot of swimming costs 1 extra foot of movement (effectively half Speed); creatures with a Swim Speed are unaffected.
- Melee attacks: attack rolls are at Disadvantage unless using a dagger, javelin, shortsword, spear, or trident.
- Ranged attacks: a ranged weapon attack automatically misses beyond the weapon’s normal range; at normal range the attack roll has Disadvantage unless the weapon is a crossbow, a net, or a thrown weapon (e.g., javelin, spear, trident, dart).
- Fire damage: anything underwater has Resistance to Fire damage.

UI and adaptation notes
- Loadout hinting: when underwater, the HUD highlights viable weapons (e.g., spear/trident) and flags those that incur Disadvantage.
- Targeting: tooltips indicate automatic miss beyond normal range while submerged.
- Movement: water‑resistance icon appears when the character lacks a Swim Speed; stamina drain and animation weight communicate friction.
- Visibility: underwater fog/light cones reduce detection; Perception checks and light sources use SRD “Vision and Light” baselines.

## Player vs. Player (PvP)

Open simulation and consequence‑driven conflict; no per‑player PvP toggles. If it exists, you can interact with it—players included.

Always‑interactable targets
- All entities are valid targets. Friendly fire is on; spells, buffs, and debuffs can target any creature consistent with SRD targeting.
- Concentration, saves, conditions, opportunity attacks, and damage rules apply identically in PvE and PvP.

Civilized spaces and consequences (not invulnerability)
- Towns and sanctuaries are protected by in‑world law and warding, not “PvP off” flags. Aggression is allowed but swiftly punished: guards respond, wards mark/outlaw offenders, and capture/arrest systems resolve crimes.
- Outlaw status is visible and persistent: bounties, faction hostility, confiscation on defeat, and travel restrictions create meaningful deterrents without removing agency.

Consentful conflict, diegetically
- Duels: initiate via heralds/circles/contracts that both parties accept; rules (timers, no outside aid, stakes) are enforced by the rite, not UI toggles.
- Wars: guilds/kingdoms declare war at heralds over regions/routes; after notice, members are open targets within the declared scope. Treaties and ceasefires are likewise filed in world.

Non‑lethal and escalation options
- Subdual outcomes (knockout, disarm, fine, exile) coexist with lethal combat. Victors choose to rob, ransom, arrest, or parley; repeated offenses escalate penalties.
- Civilian protection focuses on consequences (summoned guards, crowd control, temporary binding) rather than immunity bubbles; interactions still occur.

Anti‑grief tuning (within the fiction)
- Diminishing returns on control effects in PvP; respawn/jail logistics that remove repeat harassment loops near crime scenes.
- Safe travel that is explainable (convoys, escorts, warded ferries) rather than global invulnerability; risk scales with route and reputation.

Notes on SRD alignment
- The SRD already permits targeting any creature; RoA keeps this intact. MMO‑specific mitigations (guards, bounties, duel rites) are layered as world systems, not exceptions to the rules engine.

### Combat Simulator & Harness

Goals
- Run thousands of deterministic combat simulations (PvE/PvP) to validate balance, tactics, and encounter design.
- Control timestep, latency, RNG seed, and policies to compare outcomes.
- Headless by default; optional debug visualization.

Architecture (planned crates)
- sim-core: deterministic rules engine (fixed timestep, e.g., 50 ms). Holds entities, stats, cooldowns, effects, threat, concentration, and an event bus. No rendering.
- sim-data: SRD-derived data (JSON/TOML) for classes, spells, conditions, monsters. Versioned IDs and provenance.
- sim-policies: tactical policies (boss AIs, player rotas/priority lists, movement heuristics). Pluggable strategies.
- sim-harness: CLI runner for scenarios, sweeps, and metrics export (CSV/JSON).
- sim-viz (optional): minimal wgpu/winit debug renderer (orthographic), or TUI for timelines/logs.

Determinism & timestep
- Fixed-timestep loop (e.g., 20 Hz/50 ms) with discrete-event scheduling for casts, cooldowns, DoTs/HoTs.
- Seeded RNG per run and per-actor streams; all random draws (hit, crit, save) come from these streams.
- Net-latency model: per-actor input delay and server tick alignment for realistic cast/queue timing.

Scenario format
- YAML/JSON: map, actors (class/build), gear tier, boss type, initial positions, policies, win/lose conditions, and metrics to collect.
- Example: boss: aboleth, underwater: true, depth: shallow, party: [fighter_tank, cleric_heal, wizard_ctrl, rogue_dps, monk_dps, ranger_dps].

Policies (tactics)
- Priority lists and behavior trees: tank taunt→heavy→shove; cleric keep bless→heal<35%→cure windows.
- Movement heuristics: keep flank, avoid 8 m cones, break LoS when dominated.
- PvP: role kits (burst, peel, kite) and focus-fire rules.

Outputs & metrics
- Per-fight: TTK, DPS/HPS, damage taken, save rates, conc breaks, threat swings, time-in-melee, distance moved, ability usage histograms.
- Aggregates: mean/median/percentiles, win rate by policy, sensitivity to latency or gear.
- Artifacts: event logs (NDJSON), timelines, replay seeds.

Visualization (optional)
- Headless CSV/JSON by default. Debug modes: TUI (timelines, threat meter) and simple wgpu orthographic render (positions, AoEs, cast bars).
- Replays: load event log + seed to step or scrub.

CLI (proposed)
- Single run: `cargo run -p sim-harness -- --scenario scenarios/aboleth.yaml --seed 42 --tick 50ms --log results/run.ndjson`
- Monte Carlo: `... --trials 1000 --vary policy=tank_a,tank_b --out results/aboleth.csv`
- PvP skirmish: `... --mode pvp --team-a scenarios/team_a.yaml --team-b scenarios/team_b.yaml`

Next steps
- Define sim-core state and event types; draft Aboleth encounter from this GDD.
- Implement priority policy for the six-player party; add baseline boss AI.
- Add metrics collectors and CSV exporter; wire seeds and determinism tests.

## Zones & Cosmology

Pulled from SRD 5.2.1 cosmology. We keep the canonical plane names (Material, Feywild, Shadowfell, Inner Planes, Outer Planes, Astral, Ethereal) and describe how they manifest in an Atlantis‑ruins, oceanic MMO world.

### Material Plane
- Primary game world of shattered continents, sunken cities, and Atlantean ruins.
- Both surface archipelagos and deep‑sea environments are fully explorable.
- Baseline adventuring setting for survival, exploration, and faction conflict.

### Feywild
- Accessed via coral gates, shimmering lagoons, or enchanted whirlpools.
- The ocean’s dream‑reflection: brighter, lusher, overflowing with life.
- Sirens, fae‑like sea creatures, and enchanted kelp forests dominate.

### Shadowfell
- Reached through trenches, drowned crypts, or ghost‑ship crossings.
- Dark reflection of the sea—despair, death, and pressure of the depths.
- Drowned undead, abyssal predators, and shadowed Atlantean echoes.

### Inner Planes
- Plane of Water: the primal, infinite ocean.
- Plane of Earth: deep trenches, caverns, and volcanic ridges under the sea.
- Plane of Fire: hydrothermal vents and undersea volcanoes.
- Plane of Air: endless storms above the waves, winds that tear seas apart.
- Positive/Negative Energy: surging life‑currents and necrotic undertows.

### Outer Planes
- Canonical alignment‑tied planes reframed through an oceanic lens:
  - Mount Celestia: radiant reefs above the tides.
  - Nine Hells: volcanic trenches where devils are chained.
  - The Abyss: infinite whirlpools and bottomless rifts of chaos.
  - Mechanus: vast Atlantean tide‑engine regulating cosmic currents.

### Astral Plane
- A starlit sea navigable by astral ships; long‑distance and interplanar travel.
- Access via Atlantean gateways or dream‑navigation traditions.

### Ethereal Plane
- Felt as moonlit fogs, ghost‑ships, and drowned memories near the veil.
- Liminal space between Material and others; divers may slip through unintentionally.

### Biome: The Atlantis Underdark

#### Overview

- A vast labyrinth of submerged tunnels, caverns, and trench‑vaults beneath the seafloor.
- Formed when Atlantis collapsed; cracked foundations slid entire districts into the deep.
- Waterlogged galleries, toxic air pockets, and fungal glow‑forests stretch for leagues.

#### Environmental Features

- Light: perpetual darkness punctuated by bioluminescent algae and fungal blooms.
- Water & Air: zones range from fully submerged to half‑flooded; some contain poisonous gas pockets.
- Hazards:
  - Collapsing ceilings and sudden floods.
  - Thermal vents scalding with superheated water.
  - Hallucinogenic spores from drowned fungi forests.
- Travel: treacherous; expect climbing gear, light sources, breathing apparatus, or magic.

#### Inhabitants

- Native predators: blind cave eels, giant crabs, albino sharks.
- Monstrous factions:
  - Deepfolk: twisted Atlanteans adapted to eternal night.
  - Mycelid colonies: intelligent fungal networks, hostile to intruders.
  - Abyssal spawn: otherworldly creatures leaking in from Shadowfell trenches.
- Ruin survivors: isolated enclaves of surface folk or exiles hiding from coastal kings.

#### Adventuring Themes

- Exploration: mapping endless caverns; discovering sunken shrines and vaults.
- Survival horror: low visibility, ambush predators, paranoia in the dark.
- Mystery: ancient Atlantean runes that hint at the city’s fall.
- Faction conflict: competing explorers (guilds, cultists) fighting for underground dominance.

#### Traversal Rules (Simulator)

- Movement Speed: halved without light or special senses.
- Stealth: native monsters gain advantages; intruders without proper gear suffer penalties.
- Resources: track food, oxygen, and light supply more strictly than surface zones.
- Random Hazards: collapses, floods, fungal spore events; tie to seeded RNG for determinism.

#### Expansion Hooks

- Planar leaks: Shadowfell energies bleed in; some tunnels function as literal gates.
- Lost cities: entire Atlantean metropolises intact but upside‑down, entombed beneath the sea.
- Boss arcs:
  - A fungal hivemind that “remembers” Atlantis.
  - A trench leviathan coiled through caverns.
  - Cults summoning abyssal gods using ruin‑conduits.

#### SRD Notes

- Terrain type: uses generic SRD term “Underdark.”
- Setting flavor: Atlantis ruin‑spin keeps mechanics SRD‑aligned while distinct to RoA.

## Progression Matrix (Zones × Classes, Land Drama)

We keep standard D&D tiers (1–4 local heroes, 5–10 regional champions, 11–16 planar adventurers, 17–20 legendary figures) and map them to an oceanic + planar world with strong land‑based politics and a gold‑rush economy.

### Tier I: Levels 1–4 — Survivors & Local Heroes

Zones: fishing towns, coastal villages, frontier islands, shallow ruins newly revealed by tides.

- Land drama: petty kings, corrupt governors, and guilds try to monopolize ruins; mercenaries and smugglers race to sell finds.
- Class hooks:
  - Fighters/Rogues: hired blades for guilds or rebels.
  - Clerics/Paladins: protect shrines defiled by relic‑hunters.
  - Bards: spread songs of newfound wealth, warn of curses.
  - Wizards: first to study recovered Atlantean glyphs; Warlocks/Sorcerers feel planar pull.
- Quest themes: town defense from pirates, ruin‑scavenging, local court intrigue, protecting relic‑hunters from jealous nobles.

Player journey: establish survival and identity within the drowned world.

### Tier II: Levels 5–10 — Regional Champions

Zones: port cities, fractured kingdoms, deeper coastal ruins, haunted graveyards of fleets, edges of Feywild/Shadowfell.

- Land drama: rulers see ruins as opportunity and threat; gold rush erupts; dynasties begin to falter under corruption and conflict.
- Class hooks:
  - Barbarians/Rangers: scouts for factions seizing ruin sites.
  - Rogues: sabotage rival expeditions; smuggle relics to black markets.
  - Monks: guard Atlantean knowledge against misuse.
  - Warlocks: patrons demand access to deeper mysteries; Arcanes broker risky pacts.
- Quest themes: courtly intrigue, protecting relic caravans, exposing corrupt governors, mercenary wars over coastal control.

Player journey: small parties become regional power‑brokers balancing city intrigue with ruin‑delving.

### Tier III: Levels 11–16 — Planar Adventurers

Zones: capitals in civil war, island‑nations in revolt, gateways to Feywild coral courts and Shadowfell trenches; Inner Planes open.

- Land drama: truths of Atlantis leak into politics; factions ally with planes for supremacy; kings and high priests panic.
- Class hooks:
  - Fighters/Paladins: generals or rebel champions.
  - Clerics: confront faiths’ Atlantean origins.
  - Wizards/Sorcerers: translate ruin‑texts into potent planar magic.
  - Bards: sway courts with prophecy songs.
- Quest themes: kingdom‑wide wars, assassinations, uncovering Atlantean conspiracies, negotiating with planar courts for allies.

Player journey: advance from survival to mastery, acting as agents in world‑shaping conflicts across land and planes.

### Tier IV: Levels 17–20 — Legendary Figures

Zones: ruined empires, Outer Planes reefs and trenches, astral seas, widespread planar contact.

- Land drama: kingdoms collapse or transform; some rulers attempt god‑king ascension via Atlantean artifacts; mass migrations and rebellions.
- Class hooks:
  - Martials: mythic captains and warlords leading land‑sea armies.
  - Divines: heralds of new religions, reshaping faith itself.
  - Arcanes: command fleets sailing astral currents; build planar strongholds.
- Quest themes: stop/support ascendant god‑kings, avert ruin‑driven apocalypses, arbitrate between warring planes and mortal powers.

Player journey: heroes become kingmakers, god‑slayers, and founders of new civilizations.

### Summary Table (with Land Drama)

| Level Range | Zone Focus                   | Land Drama                              | Planar Touch                | Class Themes                           |
| ----------- | ---------------------------- | --------------------------------------- | --------------------------- | -------------------------------------- |
| 1–4         | Fishing towns, shallow ruins | Guilds & petty rulers fight over scraps | None                        | Survival, small‑scale intrigue         |
| 5–10        | Port cities, deeper ruins    | Gold rush, civil strife, guild wars     | Edges of Feywild/Shadowfell | Expedition leaders, regional champions |
| 11–16       | Capitals, island‑kingdoms    | Ruins destabilize dynasties             | Inner Planes open           | Courtly intrigue, planar alliances     |
| 17–20       | Ruined empires, planar gates | God‑kings rise, kingdoms collapse       | Outer Planes, Astral seas   | Legendary founders of new orders       |

Notes
- Launch: Tiers I–II (Material Plane with hints of Fey/Shadow).
- First expansions: Tier III (Elemental & deep planar content).
- Final arcs: Tier IV (Outer Planes + Astral endgame).

## Faction Framework

### 1. Coastal Monarchies (Old Rulers)

- Identity: ancient kings, queens, and noble houses of surviving coastal cities.
- Motives: maintain power; suppress ruin secrets that undermine legitimacy.
- Methods: armies, taxation, propaganda, ruthless courts.
- Player hooks: early protect villages under their banner; midgame spy/sabotage/defend dynasties in ruin wars; endgame confront or support god‑king ascension attempts.

### 2. Merchant Guild Cartels (Gold Rush Barons)

- Identity: merchant lords, treasure‑fleets, banking syndicates.
- Motives: exploit the ruin gold rush; monopolize relic trade.
- Methods: smuggling, privateer fleets, mercenary armies, bribery.
- Player hooks: early smuggle relics; midgame seize ruin sites and trade routes; endgame decide whether guilds become the new order.

### 3. Ruin Cults (Secrets of Atlantis)

- Identity: fanatical sects who see divine/apocalyptic truth in the ruins.
- Motives: awaken drowned gods, release abyssal powers, claim Atlantean heritage.
- Methods: rituals, sabotage, assassinations, court infiltration.
- Player hooks: early disrupt cult raids on shrines; midgame expose ties to nobles or planar patrons; endgame stop or serve ruin‑fueled apocalypses.

### 4. Seafarer Alliances (Free Peoples of the Waves)

- Identity: pirate confederacies, rebel sailors, independent islanders.
- Motives: freedom from kings and guilds; share ruin wealth among the waves.
- Methods: piracy, smuggling, populist uprisings, guerrilla naval warfare.
- Player hooks: early underdog skirmishes vs. navies; midgame alliances to claim islands; endgame establish freeports and rebel states.

### 5. Planar Orders (Beyond the Mortal Sea)

- Identity: religious orders, arcane cabals, and outsiders tied to Feywild, Shadowfell, and beyond.
- Motives: guide or manipulate mortals in the use of ruin magic.
- Methods: planar bargains, miracles, recruitment, sanctuaries.
- Player hooks: early mysterious emissaries; midgame open faction sponsorship; endgame planes clash over Atlantis’s legacy.

### Faction Conflict Axes

- Control of Ruins: monarchs vs. guilds vs. cults.
- Freedom vs. Authority: seafarer alliances vs. coastal monarchies.
- Planar Allegiance: planar orders recruit across factions; loyalties pull cross‑plane.
- Economics of Discovery: guild‑driven expansion destabilizes locals.

### Faction Progression by Tier

| Tier  | Faction Role |
| ----- | ------------- |
| 1–4   | Monarchs enforce local order; guilds/seafarers fight over scraps; cults appear as whispers. |
| 5–10  | Monarchs clash with guild cartels in ruin‑wars; seafarers grow bold; cults destabilize courts; planar orders emerge. |
| 11–16 | Monarchs fall or ally with planes; guilds run city‑states; cults control entire ruins; seafarers seize islands; planar orders intervene openly. |
| 17–20 | Monarchs attempt god‑king apotheosis; guilds create empires; cults unleash apocalypses; seafarers found free nations; planar orders bring Outer Plane war to the Material. |

### Gameplay Applications

- PvE: quest arcs around protecting relics, hunting cultists, aiding rebels.
- PvP: guild vs. guild or kingdom vs. alliance conflicts over ruin sites and trade routes.
- Player Agency: by Tier IV, players choose to uphold old orders, build new empires, or ally with planes.

## Technical Overview

- Engine: custom engine from scratch in Rust (no third‑party game engine).
- Rendering: built on `wgpu` for modern graphics APIs.
- Windowing/Input: `winit` for cross‑platform windows and event handling.
- Rationale: maximum control, performance, and customizability for MMO‑scale systems.

### Engine Strategy

We are building a custom Rust engine tailored for an authoritative MMO: server determinism first, a lean client focused on streaming, visibility, and custom ocean/terrain rendering. We’ll compose small crates (rendering, window/input, scene, assets, net, sim) with strict boundaries—no gameplay types in the renderer and no renderer types in gameplay.

### Rendering & Platform Stack Choice

#### What is `wgpu` (and why we want it)

`wgpu` is a safe, modern Rust graphics API that targets the next‑gen GPU backends: Vulkan, Direct3D 12, Metal, and WebGPU. Think of it as a Rust‑native “unified driver layer” that lets us write one renderer and run it on Windows, Linux, macOS, and (optionally) the web—without writing four backends.

Benefits
- Modern API set: explicit resource lifetimes, bind groups, render/compute passes—clean fit for our framegraph and GPU culling plans.
- Cross‑platform parity: we get DX12/Metal/Vulkan without bespoke codepaths (massively reduces maintenance).
- Safety + ergonomics: Rust types for GPU state reduce entire classes of lifetime/synchronization bugs common in raw Vulkan/DX12.
- Compute‑friendly: easy to add GPU jobs (skinning, culling, terrain/ocean FFT) as we scale.

Tradeoffs
- Less “bare metal” than raw Vulkan/DX12 (tiny overhead, but we’ll profile).
- Web builds (WebGPU) are optional for us; we treat them as a nicety, not a core target.

#### What is `winit` (and why we want it)

`winit` is a cross‑platform window + event library for Rust. It handles windows, input (keyboard/mouse), DPI, and integrates smoothly with `wgpu` surfaces.

Benefits
- One windowing layer for Win/macOS/Linux (and Wayland/X11 differences).
- Input that “just works”—keyboard, mouse, focus/resize—so we can write our own controller/UI without a full engine.

Tradeoffs
- It is intentionally minimal (no menus, no native widgets). That’s fine; we’re building an in‑engine HUD anyway.

### Why this stack fits an MMO client

- Performance control: We own the render graph, resource residency, and batching; nothing hides from the profiler.
- Deterministic sim isolation: Rendering never touches sim types; sim stays replayable and testable for server authority.
- Streaming‑first: Custom asset packs, chunked world streaming, GPU culling/indirect draws—no engine assumptions to fight.
- Long‑life maintainability: A small dependency surface that tracks platform APIs directly—less churn than big engines’ editor/tooling layers.
