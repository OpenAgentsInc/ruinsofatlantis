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
- [Technical Overview](#technical-overview)
- [Technical Overview (Expanded)](#technical-overview-expanded)

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

## Class Lore (Oceanic Context)

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

## Technical Overview

- Engine: custom engine from scratch in Rust (no third‑party game engine).
- Rendering: built on `wgpu` for modern graphics APIs.
- Windowing/Input: `winit` for cross‑platform windows and event handling.
- Rationale: maximum control, performance, and customizability for MMO‑scale systems.

## Technical Overview (Expanded)

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
