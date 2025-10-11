# GDD Split — Structure and Contribution Guide

This folder hosts the detailed design and systems sections split out from the root `GDD.md` for maintainability. The root GDD remains the canonical index and high‑level narrative; deep dives and system docs live here.

Priority reads
- architecture: see `docs/architecture/ECS_ARCHITECTURE_GUIDE.md`
- zone system: see `docs/gdd/08-zones-cosmology/zones-system.md`

Structure
- 01‑philosophy.md — Philosophy and Design Pillars
- 02‑mechanics/*.md — Major gameplay mechanics (housing, crafting, naval, combat principles, UI, etc.)
- 03‑srd/*.md — SRD usage/attribution and scope/implementation notes
- 04‑classes/*.md — Classes, races, class lore
- 05‑combat/*.md — Combat overview + examples (aboleth), underwater quick ref
- 06‑pvp.md — Player vs. Player
- 07‑combat‑simulator.md — Simulator & harness
- 08‑zones‑cosmology/*.md — Cosmology, planes, biomes
- 09‑progression/*.md — Progression tiers and summary table
- 10‑factions/*.md — Faction framework and applications
- 11‑technical/overview.md — High‑level engine strategy; system links under `docs/gdd/11-technical/**` (graphics/UI/telemetry)
- 12‑environment/*.md — Sky/weather, terrain/biomes, zones streaming (design‑side)
- 13‑rules/spell‑ability‑system.md — Spell/ability system overview (design‑side)

Contribution checklist
- If you change gameplay/rules that affect SRD:
  - Update `03-srd/scope-implementation.md` and the root `NOTICE` as needed.
- If you change a system (design or implementation):
  - Update the corresponding `docs/gdd/11-technical/**` (authoritative systems docs) and cross‑link from `11-technical/overview.md` as needed.
- Keep files short and focused; favor new focused files over very long ones.
- Update `GDD.md` links when adding or renaming files here.
