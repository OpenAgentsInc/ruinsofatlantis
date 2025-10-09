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
- All entities are valid targets except players who are your allies via party, guild, or raid. Hostile actions (attacks, harmful spells/effects, hostile interactions) do not apply to allied members; buffs and beneficial effects still do.
- Concentration, saves, conditions, opportunity attacks, and damage rules apply identically in PvE and PvP, with the ally exception above. Area effects ignore allied members by default.

Civilized spaces and consequences (not invulnerability)
- Towns and sanctuaries are protected by in‑world law and warding, not “PvP off” flags. Aggression is allowed but swiftly punished: guards respond, wards mark/outlaw offenders, and capture/arrest systems resolve crimes.
- Outlaw status is visible and persistent: bounties, faction hostility, confiscation on defeat, and travel restrictions create meaningful deterrents without removing agency.

Consentful conflict, in‑world
- Duels: initiate via heralds/circles/contracts that both parties accept; rules (timers, no outside aid, stakes) are enforced by the rite, not UI toggles.
- Wars: guilds/kingdoms declare war at heralds over regions/routes; after notice, members are open targets within the declared scope. Treaties and ceasefires are likewise filed in world.

Non‑lethal and escalation options
- Subdual outcomes (knockout, disarm, fine, exile) coexist with lethal combat. Victors choose to rob, ransom, arrest, or parley; repeated offenses escalate penalties.
- Civilian protection focuses on consequences (summoned guards, crowd control, temporary binding) rather than immunity bubbles; interactions still occur.

Anti‑grief tuning (within the fiction)
- Diminishing returns on control effects in PvP; respawn/jail logistics that remove repeat harassment loops near crime scenes.
- Safe travel that is explainable (convoys, escorts, warded ferries) rather than global invulnerability; risk scales with route and reputation.

Notes on SRD alignment
- The SRD permits targeting any creature; RoA preserves this except for explicit ally groupings (party/guild/raid), where hostile actions are disabled by design. Duels/wars temporarily override this when consented or declared. Other MMO‑specific mitigations (guards, bounties, duel rites) are layered as world systems.

### Combat Simulator & Harness

Goals
- Run thousands of deterministic combat simulations (PvE/PvP) to validate balance, tactics, and encounter design.
- Control timestep, latency, RNG seed, and policies to compare outcomes.
- Headless by default; optional debug visualization.

Design doc: see `docs/combat_sim_ecs.md` for the ECS design, system pipeline, and SRD rules mapping used by the simulator.

Architecture (planned crates)
