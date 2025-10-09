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
