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

This section has moved to `docs/gdd/01-philosophy.md` to improve maintainability and review flow.

Read: docs/gdd/01-philosophy.md

## Game Mechanics

This section is split for maintainability. Start with the overview, then see focused topics:

- docs/gdd/02-mechanics/overview.md
- docs/gdd/02-mechanics/in-world-repair.md
- docs/gdd/02-mechanics/dynamic-events-bosses.md
- docs/gdd/02-mechanics/player-housing-settlements.md
- docs/gdd/02-mechanics/crafting-professions-economy.md
- docs/gdd/02-mechanics/seafaring-naval.md
- docs/gdd/02-mechanics/lessons-from-classic-mmos.md
- docs/gdd/02-mechanics/combat-casting-principles.md
- docs/gdd/02-mechanics/spells-focus-resources.md
- docs/gdd/02-mechanics/damage-progression-xp.md
- docs/gdd/02-mechanics/input-ui.md
- docs/gdd/02-mechanics/navigation-wayfinding.md
- docs/gdd/02-mechanics/mounts-social-etiquette.md
- docs/gdd/02-mechanics/loot-economy.md
- docs/gdd/02-mechanics/boss-encounter-design.md
- docs/gdd/02-mechanics/visual-direction.md
- docs/gdd/02-mechanics/onboarding-micro-sandbox.md
- docs/gdd/02-mechanics/stats-clarity.md
- docs/gdd/02-mechanics/monetization.md
- docs/gdd/02-mechanics/player-experience-targets.md
  - Repairs are environment events the sim can schedule (restore object integrity, clear hazards), not combat stats. Policies can choose to prioritize repairs during lulls or after encounters.

### Dynamic World Events & World Bosses

- Emergent events
  - AI‑driven raids, sea monster attacks, faction ambushes, and environmental crises trigger from world state (over‑harvest, faction imbalance, weather), not fixed timers. Events can escalate when ignored.
- World bosses
  - Roaming or slumbering titans (krakens, leviathans, elemental colossi) appear dynamically and scale participation. Failure can strengthen them; success unlocks temporary boons, questlines, or caches.
- Impactful outcomes
  - Towns and outposts can be besieged, occupied, or liberated; NPCs may flee or return; markets fluctuate briefly. Player action visibly changes local state until recovery.
- Community play
  - Nearby players receive lightweight rally prompts; ad‑hoc cooperation is encouraged without hard scheduling. Progress is shared by contribution.

### Player Housing & Settlements

- Private homes
  - Claim plots in housing districts (coast, islands, underwater domes). Customize with furniture, trophies, and style kits. Benefits are convenience‑only (rested XP, storage, crafting benches, small gardens) to preserve non‑P2W.
- Open‑world placement
  - Homes exist in the world (in designated districts/frontiers) to form neighborhoods. Visitation permitted with privacy controls. Frontier capacity expands with population.
- Guild halls & settlements
  - Guilds can establish upgradable halls or freeports with defenses, vendors, and member buffs. These can attract dynamic events (e.g., raids) and tie into faction politics.
- Safeguards
  - Placement rules avoid griefing and maintain sightlines/performance; decay timers and upkeep are time‑based (not cash‑based).

### Crafting, Professions, and Economy

- Gathering professions
  - Mining, Herbalism, Fishing, Salvaging, Foraging—oceanic resources (pearls, corals, vent metals, abyssal fungi) with regional scarcity encourage trade and exploration.
- Crafting lines
  - Smithing/Armoring, Carpentry/Shipwright, Alchemy/Cooking, Enchanting/Inscription. Recipes discovered via exploration, faction favor, or experimentation.
- Player‑driven markets
  - Posted contracts and local auctioneers in hubs; no global instant delivery. Caravans and shipping create economic gameplay; piracy risk informs routes.
- Repair & durability
  - Items wear under stress; repairable via professions and SRD tools. Catastrophic failure is rare; mastery reduces costs.
- Economy guardrails
  - Bind‑on‑equip/pickup used judiciously. No monetized multipliers. Transparent drop tables and sinks (repairs, housing upkeep) stabilize currency.

### Seafaring & Naval Play

- Navigation & exploration
  - Chart courses across currents and storms; respect travel as gameplay. Reaching new islands requires seamanship and preparation.
- Naval combat & piracy
  - Ship‑to‑ship and ship‑to‑monster encounters; PvP piracy with in‑world consequences (bounties, faction loss). Counterplay via escorts and bounty hunting.
- Underwater content
  - Diving kits, spells, or crafted submersibles enable trench ruins and deep Atlantis expeditions. Extends our underwater combat rules with traversal tools and pressure hazards.

### Lessons From Classic MMOs (Design Stance)

- Community & challenge
  - Meaningful penalties, tough dungeons, and teamwork are virtues—difficulty comes from the world, not UI friction. We provide information and readable mechanics.
- Class identity matters
  - Preserve distinct fantasies and class‑specific quests. Avoid homogenizing roles; balance by encounter and utility, not identical kits.
- Balanced QoL
  - Offer modern conveniences only when they don’t erode immersion (e.g., in‑world LFG boards/tavern posts; no teleport‑to‑dungeon menu). Travel and discovery remain core.
- Continuous improvement
  - In‑client surveys, PTR for major drops, and telemetry‑informed tuning while staying true to tenets. Candid patch notes explain “what” and “why.”

### Combat & Casting Principles

- Line‑of‑Sight casting
  - Targeted spells succeed if the raycast has LoS to the aim point or target. “Out of range” does not invalidate a valid LoS shot; range instead affects travel time and optional falloff.
- Cast while moving
  - You can move during casts. Sprinting applies small tradeoffs (e.g., +10–20% cast time, minor cone spread). Channels break under heavy disruption.
- Fewer, heavier spells
  - Four core combat slots plus one ultimate. Longer cooldowns (8–25s core; 60–120s ultimate) with high impact (stagger, elemental status, destructible interaction). Cantrips bias toward mobility/utility over DPS spam.
- Friendly fire and physical lanes
  - Projectiles collide with allies/enemies. AI avoids firing through allies at short range; players can bait enemy fire.
- Default tunings (initial targets)
  - Projectile falloff: −20% damage per 30 m beyond effective range; no hard stop.
  - Moving‑cast penalty: +15% cast time; +10% cooldown; +10% spread for cone/bolt spells.

### Spells, Focus, and Resource Feel

- Focus resource
  - A light resource that drains while sprint‑casting and refills quickly when steady. Movement and damage taken reduce Focus; landing precise hits restores a small amount.
- Cooldowns as primary gate
  - Core cooldowns 10–18 s; ultimate 60–120 s. Big casts have big consequences and readable wind‑ups.
- Voxel interaction
  - Spell ranks and Focus spent scale destructible effects (e.g., carve radius, ignite/soak, brittle/freeze) where appropriate.

### Damage, Progression, and XP

- Grounded numbers
  - Early hits land at 10–60 damage, not thousands. Prefer tiers (Common/Elite/Boss) and resists over inflated health pools.
- XP from combat and adventures
  - Every enemy awards XP scaled by difficulty and streak/milestone bonuses. Quests/dungeons act as multipliers, not sole sources.
- School proficiency
  - Using a school (Fire, Frost, Force, etc.) grants small utility bonuses (e.g., −5% cooldown, +1 chain target, +0.2 s slow). Caps per region avoid grind; story beats grant bumps.

### Input & UI

- Hybrid cursor mode
  - Hold a modifier (e.g., Alt/RB) to unlock the cursor for UI; otherwise remain in mouselook.
- Immersion Mode
  - Optional preset hides waypoints, quest arrows, damage numbers, and any store/live‑ops panels. Off by default; toggle in HUD settings.
- Readable feedback
  - Small hit markers (off by default), concise status icons (Burning/Frigid/Stagger), and an on‑demand combat log. Plain‑language descriptions throughout.

### Navigation & Wayfinding

- No glowy waypoints
  - Use in‑world guidance: smoke plumes, flocking birds, NPC scouts, footprints, rune whispers, and strong landmarks. Offer a map as an inspectable “paper” item rather than a permanent overlay.
- Guide NPCs
  - Players can ask a Guide to escort them to a site; the Guide physically leads, choosing interesting, safe(ish) routes and pausing at hazards.

### Mounts & Social Etiquette

- Believable mounts
  - Whistle summons from stables/nearby; mount runs to you; mount/dismount animations; no pop‑in beneath feet.
- Settlement manners
  - Auto‑dismount in town volumes and near authorities; some dialogues refuse while mounted; guards will bark if you ride indoors.

### Loot & Economy

- Auto‑loot magnet
  - Auto‑pickup within ~2.5–4 m with rarity filters; expand radius while a key is held. No click‑spam.
- Fair, transparent rewards
  - Published loot tables; pity timers for extreme rares; bind‑on‑equip where it supports a healthy player economy. No monetized multipliers in core loops.

### Boss & Encounter Design

- Positioning and lanes
  - Enemies can hit each other; destructible cover and friendly fire create tactical play. Boss immunity windows are short and clearly telegraphed.
- Break bars & phases
  - Encounters use stability/break mechanics and phase transitions instead of sponge HP. Ultimates interact strongly with break bars (e.g., deplete ~35–45% when well‑timed) rather than deleting health.

### Visual Direction

- Readability first
  - Fewer details, clearer shapes, strong silhouettes. Effects communicate gameplay (color = element; shape = area; duration = linger). Triplanar texture for voxels and stylized materials for clarity.

### Onboarding & Opening (Micro‑Sandbox)

- First 10–15 minutes
  - A compact space with two spells, one movement tool, a Guide NPC, and a destructible combat set piece. Teaches LoS casting, moving casts, friendly‑fire baiting, cover destruction, mount call, and Guide request. Rewards a third spell and first proficiency point upon completion.

### Stats & Clarity

- Plain‑language stats
  - Stat cards explain exactly what each stat does and show before/after deltas. Keep core stats ≤ 6. Hover for effects (e.g., “+1 Control: +4% slow duration, +3% stun resist”).

### Monetization Principles

- Cosmetics and QoL only
  - No loot boxes, no pay‑to‑win, no power gating. Cosmetics (dyes, pets, emotes), account services, and non‑combat QoL only. Paid expansions may add zones/dungeons/campaigns; base arcs remain accessible.

### Player Experience Acceptance (Initial Targets)

- Casting with clear LoS succeeds regardless of distance; travel time and falloff apply.
- Casting while moving feels responsive; sprinting changes cast feel but not availability.
- Auto‑loot by proximity works; minimal clicking.
- Enemies can strike each other; players can bait friendly fire.
- Early‑game numbers are grounded and legible; damage spam is optional or off.
- Mobs grant meaningful XP; school proficiency advances utility.
- No glowy waypoints; a Guide NPC can physically lead you.
- Mount arrives believably; towns enforce etiquette.
- Boss ultimates drive phase changes rather than single‑press kills.
- HUD “Immersion Mode” hides out‑of‑world UI clutter (waypoints, sales).

## SRD Usage and Attribution

Read: docs/gdd/03-srd/usage-attribution.md (NOTICE at repo root).

## SRD Scope & Implementation

Read: docs/gdd/03-srd/scope-implementation.md

## Classes

Read: docs/gdd/04-classes/overview.md

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

Read: docs/gdd/04-classes/lore.md

## Combat

Read:
- docs/gdd/05-combat/overview.md
- docs/gdd/05-combat/example-aboleth-raid.md
- docs/gdd/05-combat/underwater-quickref.md

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

Read: docs/gdd/06-pvp.md

### Combat Simulator & Harness

Read: docs/gdd/07-combat-simulator.md

## Zones & Cosmology

Read:
- docs/gdd/08-zones-cosmology/overview.md
- docs/gdd/08-zones-cosmology/material-plane.md
- docs/gdd/08-zones-cosmology/feywild.md
- docs/gdd/08-zones-cosmology/shadowfell.md
- docs/gdd/08-zones-cosmology/inner-planes.md
- docs/gdd/08-zones-cosmology/outer-planes.md
- docs/gdd/08-zones-cosmology/astral-plane.md
- docs/gdd/08-zones-cosmology/ethereal-plane.md
- docs/gdd/08-zones-cosmology/biome-atlantis-underdark.md

## Progression Matrix (Zones × Classes, Land Drama)

Read: docs/gdd/09-progression/tiers.md

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

Read: docs/gdd/10-factions/framework.md

## Technical Overview

Read: docs/gdd/11-technical/overview.md
## Environment: Sky & Weather

Read: docs/gdd/12-environment/sky-weather.md

## World: Terrain & Biomes

Read: docs/gdd/12-environment/terrain-biomes.md

* CDLOD/quadtree with geomorphing; occlusion/indirect draws; roads/decals; wind animation; navmesh bake.

---

## World: Zones (Persistence & Streaming)

Read: docs/gdd/12-environment/zones-persistence-streaming.md

## Rules: Spell & Ability System

Read: docs/gdd/13-rules/spell-ability-system.md
