# Ruins of Atlantis Game Design Document

Ruins of Atlantis is a fantasy MMORPG under development by Blue Rush Studios, a division of OpenAgents, Inc.

## Contents

- [Philosophy](#philosophy)
- [Game Mechanics](#game-mechanics)
- [SRD Usage and Attribution](#srd-usage-and-attribution)
- [SRD Scope & Implementation](#srd-scope--implementation)
- [Classes](#classes)
- [Races](#races)
- [Class Lore (Oceanic Context)](#class-lore-oceanic-context)
- [Combat](#combat)
- [Zones & Cosmology](#zones--cosmology)
- [Progression Matrix (Zones × Classes, Land Drama)](#progression-matrix-zones--classes-land-drama)
- [Faction Framework](#faction-framework)
- [Technical Overview](#technical-overview)

## Philosophy

RoA embraces old‑school difficulty. Travel and logistics matter, resources are scarce, choices are meaningful, and death is painful.

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

### Example Combat: Six‑Player Boss Fight

Scenario
- Party: Fighter (tank), Cleric (healer), Wizard (control/DPS), Rogue (melee DPS), Monk (melee skirmisher), Ranger (ranged DPS/utility).
- Boss: Leviathan Warden (Legendary; high AC, thunder/cold attacks, summons adds, heavy knockback). Mechanics draw on SRD (attack rolls vs. AC, saves vs. DCs, conditions, concentration) adapted to real‑time pacing.

What players see (UI)
- Boss frame with AC “skull” indicator, cast bar (e.g., Tidal Roar 2.5s), and enrage timer.
- Party frames: HP/mana/stamina; a Concentration icon for concentrating casters (glows and shows DC when damaged).
- Player HUD: hotbar with per‑ability cooldowns, global cooldown (GCD) spinner, weapon swing timer (for auto‑attacks), resource bars, threat meter.
- Ground telegraphs for large effects (cones, circles); subtle tells (animation, audio) for interruptible casts.

Pull & Phase 1 (0:00–0:45)
- Fighter opens with Taunt (MMO mechanic; sets initial threat) → closes to melee, Shove (SRD) to turn the boss away from group; maintains threat with sustained attacks. Uses Second Wind reactively.
- Cleric pre‑casts Bless (SRD; Concentration) on three allies; keeps Healing Word (Bonus Action) on standby; positions at mid‑range to avoid cleave.
- Wizard applies Web on add spawn points (SRD; Dex save to restrain; Concentration) and uses Firebolt/Ray of Frost as filler; watches boss cast bar for interrupts.
- Rogue opens from behind; time a heavy opener after Fighter’s first swing (to avoid pulling threat); uses Cunning Action (Disengage) to dodge cleaves; applies Sneak Attack when advantage/back window appears.
- Monk engages flank: uses Focus Points for Flurry of Blows to build threat‑adjacent DPS without overtaking tank; Patient Defense when boss turns; Step of the Wind to exit ground AoE.
- Ranger opens with Ensnaring Strike (SRD; on‑hit, Str save or restrained; Concentration) on the boss for brief control windows; then cycles aimed shots; swaps to adds as needed.

Boss behavior
- Basic pattern: heavy slam (bludgeoning), tail sweep (cone knockback), and Tidal Roar (2.5s cast, 30m cone, Con save half; on fail: Deafened 6s).
- On damage taken, casters make SRD Concentration checks (Con save DC 10 or half damage) to maintain Bless/Web/Ensnaring Strike.
- Every 20s: Summon Barnacle Swarm (3–4 adds) at reef vents; adds fixate on healers unless taunted/controlled.

Micro interactions (SRD mapped to real‑time)
- Attack rolls: all weapon and spell attacks roll vs. boss AC; crits on 20. Advantage granted by control (restrained), stealth windows, or specific positional rules we adopt.
- Saves: players make Dex/Con/Wis saves vs. boss DCs; UI shows save type and result; on fail, apply conditions (Prone, Deafened, Restrained) per SRD.
- Reactions: Opportunity Attacks trigger on careless movement; Shield (SRD) and Uncanny Dodge (Rogue) fire as reactions with short lockouts.

Phase 2 (0:45–1:45): Pressure & Adds
- At 70% HP, boss gains Crushing Pressure: stacking debuff on current target (increases incoming damage and Concentration DCs). Fighter rotates defensive cooldowns; may Grapple/Shove (SRD) to keep boss off squishies after knockbacks.
- Add wave spawns; Wizard’s Web restrains some (Dex save). Ranger kites loose adds through caltrops/snare shots; Rogue peels an add with high burst then Vanish (if available) or Cunning Action to drop threat.
- Cleric swaps to triage: instant Healing Word on the move; Cure Wounds (casted) between dodges. If Concentration breaks, re‑establish Bless when safe.
- Monk interrupts a late Tidal Roar with a class stun (if available) or times a Flurry window between boss casts; otherwise uses Patient Defense to help healers stabilize.

Phase 3 (1:45–end): Enrage Windows
- Boss becomes Bloodied (50%): gains Undertow Grab (grapple, contested Athletics vs. Fighter) and occasional Frightful Current (Wis save or Frightened 8s). Party uses Inspiration or class features to mitigate.
- Wizard times Fireball on clustered adds (Dex save) when tank stabilizes threat; swaps to single‑target cantrips during spread mechanics to avoid collateral.
- Ranger switches to sustained single‑target; refreshes Ensnaring Strike after breaks. Positions to avoid tail cones while maintaining line of sight.
- Rogue maintains back position; watches for swing timer gap to Backstab (if we add positionals); Cunning Action to re‑enter back arc after knockbacks.
- Fighter uses Action Surge to cover a healer save failure (spike DPS/threat, shorten phase); Indomitable on a failed critical save.
- Cleric channels a big heal window during predictable boss combos; maintains Concentration and rotates defensive buffs if domain allows (per SRD).

Buttons, timing, and waits (illustrative hotbars)
- Fighter: [1 Taunt (8s cd)] [2 Heavy Strike] [3 Shove] [4 Shield Block (cd)] [Q Second Wind] [E Action Surge] [R Indomitable]
- Cleric: [1 Healing Word] [2 Cure Wounds] [3 Bless (Conc)] [4 Shield of Faith (Conc)] [Q Spare the Dying] [E Turn Undead]
- Wizard: [1 Firebolt] [2 Ray of Frost] [3 Web (Conc)] [4 Fireball] [Q Shield (Reaction)] [E Misty Step]
- Rogue: [1 Basic Strike] [2 Eviscerate] [3 Cunning Action] [Q Uncanny Dodge (Reaction)] [E Evasion]
- Monk: [1 Jab] [2 Flurry (Focus)] [3 Patient Defense (Focus)] [4 Step of the Wind (Focus)]
- Ranger: [1 Aimed Shot] [2 Multi‑Shot] [3 Ensnaring Strike (Conc)] [4 Dash/Disengage] [Q Trapper’s Kit]

Threat & aggro
- Threat accumulates from damage, taunts, and healing (reduced). Tank has innate threat modifiers; hard taunt snaps target briefly (diminishing if abused). Threat tables are visible to the group to teach management.

Failure and recovery
- If Tidal Roar is un‑interrupted and several fail Con saves, Concentration breaks and damage spikes; group answers with stuns, kiting, and triage.
- Wipes typically occur when adds overwhelm healers or the tank loses control during knockback + grab. Recovery involves battle rez (if available), controlled kites, and re‑establishing Concentration.

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
