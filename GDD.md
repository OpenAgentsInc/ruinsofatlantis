# Ruins of Atlantis Game Design Document

Ruins of Atlantis is a fantasy MMORPG under development by Blue Rush Studios, a division of OpenAgents, Inc.

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

## Technical Overview

- Engine: custom engine from scratch in Rust (no third‑party game engine).
- Rendering: built on `wgpu` for modern graphics APIs.
- Windowing/Input: `winit` for cross‑platform windows and event handling.
- Rationale: maximum control, performance, and customizability for MMO‑scale systems.

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
