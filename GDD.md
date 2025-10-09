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

### Spell Data Pipeline: JSON Authoring, Binary Runtime
### Spell Data Pipeline: JSON Authoring, Binary Runtime

Short answer: JSON isn’t the fastest, but it is usually the best authoring format to stand up a correct, SRD‑faithful, moddable spell system quickly—then we compile it to a fast binary for runtime.

Why JSON (authoring)
- Frictionless editing & review: easy to read, diff, and review in PRs; great fit for designers, engineers, and external contributors. Plays well with codegen, CI linters, and schemas.
- Strong validation: lock data down with a JSON Schema (types, ranges, enums) and CI validation; guarantees SRD mechanics are representable (save DC formulas, V/S/M components, durations, AoE shapes, Concentration, half‑on‑success, etc.).
- Tooling ecosystem: abundant tools for migration scripts, formatters, and quick transforms (e.g., jq); simple import/export to spreadsheets.
- Hot‑reload during development: fast iteration while tuning balance or fixing issues found by the combat sim.
- Clear provenance: mapping SRD 5.2.1 mechanics into data (not code) simplifies audits/attribution.

But JSON parsing isn’t the fastest… right—so we don’t ship JSON on the hot path.

Pipeline
1. Author in JSON (human‑friendly).
2. Validate in CI with JSON Schema + unit tests (mitigation order, THP rules, Concentration DC, AoE shapes).
3. Compile at build time into a compact binary pack (e.g., `*.spellpack`): stable numeric IDs, deduped strings, precomputed lookups, and pre‑flattened effect graphs.
4. Load binary at runtime (zero‑copy/`bytemuck` PODs or a tight deserializer). JSON is used only for dev/hot‑reload and tooling.

Alternatives & trade‑offs
- Rust enums/const tables only: fastest load and strong type safety, but slow iteration and noisy diffs; good for “frozen” core content later.
- RON/TOML/YAML: nicer comments than JSON, but weaker editor/CI ecosystem and typically slower/looser parsers; if comments are required, RON/YAML is acceptable for authoring, still compile to binary for runtime.
- SQLite pack: useful for patching/queries, but overkill early; a binary blob is simpler and faster to start.

Performance notes (what we’ll do)
- Precompute AoE samplers, save DC resolvers, mitigation‑order tables, condition IDs, and damage‑type masks.
- Intern strings and use numeric IDs at runtime.
- Avoid dynamic dispatch in the hot path: small enum opcodes dispatched via a jump table.
- Keep cold data compressed: ship one spellpack per version; memory‑map and build in‑memory indices on first use.

Safety & SRD fidelity
- A data‑driven spell system makes it straightforward to verify SRD mechanics (save DCs, components, Concentration, resistance/vulnerability order, THP non‑stacking, roll‑once AoE damage) and to document any MMO‑layer deviations; this is easier to audit than logic scattered across code.

Concrete recommendation
- Use JSON for authoring (or RON if comments are required).
- Add `spell_schema.json`, CI validation, and a build step that emits `spellpack.bin`.
- Support hot‑reload JSON in development, load only binary in release.
- Bake in content hashes and versioning; fail fast if client/server spellpack hashes mismatch.
 

## Environment: Sky & Weather

**Design Intent.** A physically‑plausible, configurable sky that animates day/night, drives sun light and ambient skylight, and supports per‑zone weather variation—consistent with RoA’s “in‑world, no toggles” philosophy.

**Player Experience.**

* Sun and sky progress naturally through the day; dawn/dusk tint the world.
* Overcast, haze, and fog vary by zone (swampy lowlands vs. coastal cliffs).
* Lighting changes are readable and influence visibility and mood.

**Scope (Phase 1).**

* Analytic clear‑sky model (Hosek–Wilkie) evaluated per pixel.
* Sun position from game time (day‑fraction) with optional geographic driver.
* Directional sunlight + **SH‑L2** ambient skylight for fill.
* Distance/height‑based fog. Optional simple tonemapper (Reinhard / ACES fit).
* Per‑zone weather overrides: turbidity, fog density/color, ambient tint, exposure.
* Tooling hooks in `tools/model-viewer`.

**Data & Authoring.**

* `data/environment/defaults.json` (global), `data/environment/zones.json` (overrides).
* Runtime controls: pause/scrub time, rate scale.
* Debug: show azimuth/elevation; sliders for turbidity/fog/exposure.

**Runtime Behavior.**

* **Renderer order:** sky → shadows → opaque → transparent/FX → UI.
* **Lighting:** `sun_dir_ws`, `sun_illuminance`, `sh9_ambient` in `Globals` UBO.
* **Zones:** entering a WeatherZone blends to its profile over 0.5–2.0s.

**Integration Points.**

* Terrain/biomes shading uses directional + SH ambient.
* Minimap shows weather glyph; HUD clock displays zone time.
* Sim/Events may trigger storms later (Phase 2).

**Performance Targets.**

* Sky pass ≤0.2 ms; SH projection ≤0.1 ms/frame amortized; single shadow map in Phase 1.

**Future Work.**

* Volumetric clouds and aerial perspective; precipitation; moon/stars; cascaded shadows.

---

## World: Terrain & Biomes

**Design Intent.** Fast, attractive terrain that varies by biome and is **procedurally generated once, then baked** into persistent zone snapshots. Phase 1 focuses on a Woodland baseline (rolling hills, dense grass, scattered trees).

**Player Experience.**

* Natural rolling hills; grass thick near the player; trees spaced believably.
* Layout is stable across sessions/players (persistent zone), not re‑rolled.

**Scope (Phase 1).**

* Heightfield generation: **OpenSimplex2 fBm + domain warping**.
* Chunked mesh (e.g., 64×64 verts) with simple distance LOD and skirts.
* **Triplanar** material with slope/height blending (grass/dirt/rock).
* Vegetation:

  * **Trees** from GLB prototypes placed via **Poisson‑disk** (baked, instanced).
  * **Grass** as GPU‑instanced cards with density masks per chunk (baked).
* Bake tool writes `snapshot.terrain.bin`, `snapshot.instances.bin`, masks, meta.

**Data & Authoring.**

* `data/zones/<zone>/config.json`: size, seeds, noise params, densities.
* `data/zones/<zone>/prototypes.json`: tree GLBs, radii, LOD hints.
* Bake outputs under `data/zones/<zone>/snapshot.*`.

**Runtime Behavior.**

* Client streams visible terrain chunks; builds VB/IB once per chunk; instanced draws for vegetation.
* Height sampling helper `terrain::sample_height(xz)` for gameplay placement.

**Integration Points.**

* Uses Sky & Weather lighting uniforms for consistent shading.
* Zones System consumes baked assets; AOI decides which chunks to stream.
* Sim uses deterministic seeds for spawn masks if needed.

**Performance Targets.**

* Terrain + vegetation ≤5 ms on mid‑range GPU at default draw distance.
* Zero per‑frame allocations in hot path; instance upload ring buffers.

**Future Work.**

* CDLOD/quadtree with geomorphing; occlusion/indirect draws; roads/decals; wind animation; navmesh bake.

---

## World: Zones (Persistence & Streaming)

**Design Intent.** A **Zone** is the atomic world unit: named, persistent, streamable, and authoritative on the server. Content is generated/authored once, **baked to a snapshot**, then served with runtime **delta logs** for persistent changes (destroyed doors, captured flags, placed campfires).

**Player Experience.**

* Zones feel alive and consistent: changes persist across sessions.
* Travel uses in‑world **connectors** (gates, docks, caves); brief hand‑off between zones.

**Core Concepts.**

* **Manifest** (authoring input): IDs, plane, size, seeds, environment defaults, spawn tables, connectors.
* **Snapshot** (cooked, immutable): terrain chunks, instances, masks, meta (content hash).
* **Delta Log** (append‑only): runtime changes applied over the snapshot; compacted into checkpoints.
* **Zone Graph**: directed connectors between zones with requirements/costs.
* **AOI Grid**: interest management for streaming chunks/entities to clients.

**Server Responsibilities.**

* Load snapshot + checkpoint + trailing deltas → **ZoneRuntime**.
* Manage AOI subscriptions, NPC spawners, timers; persist deltas and periodic checkpoints.
* Validate travel requests; hand off players to target zones/connectors.

**Client Responsibilities.**

* Subscribe to AOI cells around the camera; request and cache chunk blobs.
* Build/destroy GPU buffers as chunks enter/exit AOI; render terrain/instances.
* Trigger travel when entering connector volumes; show minimal loading.

**Data & Authoring.**

* `data/zones/<zone>/manifest.json`, `graph.json`.
* Bake tool `zone_bake` emits `snapshot.v1/*` (terrain/instances/masks/meta).
* `zone_check` validates IDs/graph, budgets, and content hashes.

**Integration Points.**

* Terrain & Biomes: snapshot payloads; height sampling.
* Sky & Weather: environment defaults merge with zone weather.
* Sim/Rules: deltas carry structure/state changes; events inform HUD toasts.
* PvP/Law (later): ward volumes + policies enforce consequences (no invuln toggles).

**Determinism & Security.**

* Per‑zone seeded RNG streams; content‑addressed snapshots; client only consumes server‑approved blobs.

**Performance Targets.**

* AOI churn ≤2 ms/tick server‑side at target concurrency; steady‑state snapshot streaming ≤1 MB/s per client at default radius.

**Future Work.**

* Seamless cross‑zone streaming; city law/ward logic; instanced expeditions/raids; sharding.

---

## Rules: Spell & Ability System

**Design Intent.** SRD 5.2.1–faithful spellcasting and abilities with a thin, deterministic real‑time MMO layer (cast bars, GCD, cooldowns) that preserves SRD math while fitting RoA’s pacing and telemetry needs.

**Player Experience.**

* Classic feel: cast bars, channels, interrupts; Concentration with visible feedback.
* Tooltips show save DC, components (V/S/M), duration, damage types, and tags.
* Auras/buffs/debuffs behave predictably; combat log is clear.

**Rules Fidelity (high level).**

* **Formulas:** Spell Save DC and Spell Attack bonus; Advantage/Disadvantage and conditions per SRD.
* **Components:** Verbal/Somatic/Material with focus/pouch substitution where allowed.
* **Casting Time:** Action/Bonus/Reaction, rituals, long casts; “one slot per turn.”
* **Durations & Concentration:** new concentration breaks old; damage triggers Con save DC 10 or half damage (floor), cap 30.
* **Targeting & Areas:** cone/cube/cylinder/line/sphere; clear path required.
* **Damage Resolution:** roll once for AoE saves; apply half on success where specified.
* **Mitigation Order:** adjusters → resistance → vulnerability; immunity nullifies; **Temporary HP** doesn’t stack and is consumed first.
* SRD attribution and licensing live in the GDD’s existing SRD sections.

**MMO Layer.**

* **Global Cooldown (GCD)** + per‑ability cooldowns.
* Cast/channel states; movement/interrupt policies tuned for RoA but never altering SRD math.
* Threat hooks (damage, healing, taunt) integrate with sim aggro.

**Data & Authoring.**

* **Authoring format:** JSON (or RON) for readability and CI validation; compiled at build time to a compact **binary spellpack** for runtime.
* Stable IDs; content hashes; server/client spellpack hash check at login.
* Authoring schema covers: school, level, lists, components, targeting, effects (op‑codes: MakeSpellAttack, PromptSave, DealDamage, GrantTempHP, ApplyCondition, SpawnArea, ModifyRoll), scaling by slot.

**Runtime & Events.**

* **Pipeline:** intent → validation (resources, components, LoS, range) → cast/channel → resolve effect graph → apply outcomes → emit events.
* **Events:** `CastStarted/Finished/Interrupted`, `SaveRolled`, `DamageDealt/Healed`, `AuraApplied/Removed`, `ConcentrationStarted/Broken`.
* HUD consumes events for cast bars, GCD, auras, combat log.

**Representative Coverage (MVP).**

* Direct attack (Fire Bolt), AoE save/half (Cone of Cold/Fireball pattern), buff with THP + Concentration (Heroism), curse rider (Hex), hybrid attack+AoE (Ice Knife), non‑damage zone with repeated saves (Zone of Truth).

**Performance & Determinism.**

* No dynamic dispatch in hot path; small enum jump table for effect ops.
* Precompute SH/mitigation tables, area samplers, string interning → numeric IDs only at runtime.
* Fixed timestep sim; seeded RNG streams; golden tests per spell.

**Future Work.**

* Class features/metamagic, item‑granted spells, aura stacking policies, network protocol finalization.

---
