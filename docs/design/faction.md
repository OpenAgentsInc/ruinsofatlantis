 Here’s what “Faction” is in our code today — no hand‑waving, just how it actually works.

  Definition

  - Server enum: A small allegiance label on every actor.
      - crates/server_core/src/actor.rs:16
          - enum Faction { Pc, Wizards, Undead, Neutral }
      - Stored on every actor as faction (not “team”).
      - crates/server_core/src/ecs/world.rs:36

  What it’s used for (server authority)

  - Hostility rules:
      - Static default: Pc↔Undead and Wizards↔Undead are hostile.
          - crates/server_core/src/ecs/world.rs:215
      - Dynamic flip: Pc↔Wizards hostility toggles true when PC damages a Wizard.
          - crates/server_core/src/combat.rs:15
          - crates/server_core/src/ecs/schedule.rs:990
  - System queries and filters (no kind‑based logic):
      - Target “wizards” by faction, not kind (e.g., movement/melee/chase picks Wizards faction members).
          - crates/server_core/src/ecs/schedule.rs:94
      - All “owner vs target” checks use a.faction and FactionState::effective_hostile(...).
          - crates/server_core/src/ecs/schedule.rs:122, 463, 523, 658, 851, 1154

  Where it’s set

  - Spawns assign factions explicitly:
      - PC → Faction::Pc, NPC wizards → Faction::Wizards, Undead/DK → Faction::Undead.
      - crates/server_core/src/lib.rs:395, 592, 580, 623

  Replication (wire)

  - v4 ActorSnapshotDelta carries faction as a compact u8, plus IDs for presentation:
      - ActorRep { faction: u8, archetype_id: u16, name_id: u16, unique: u8, … }
      - crates/net_core/src/snapshot.rs:300
      - Server maps enum → u8 in tick_snapshot_actors.
          - crates/server_core/src/lib.rs:780
      - Client stores these fields in ActorView (no gameplay on client).
          - crates/client_core/src/replication.rs:220

  What it is not

  - Not an ECS “system.”
  - Not a class or archetype. ActorKind is presentation only (models/UI); logic does not branch on it.
  - Not a string on the wire. We send a compact u8 faction and data IDs (archetype/name).

  Runtime behavior summary

  - Every actor has faction.
  - Systems decide who can target/attack whom by calling FactionState::effective_hostile(a_faction, b_faction).
  - We statically treat Undead as hostile to Pc and Wizards; a runtime toggle flips Pc↔Wizards hostility when PC damages a Wizard.
  - All places that used “Team” in the ECS path now use Faction; “Team” only remains in sim_core scenario strings (tabletop sim), not in the server ECS.

  Tests covering this

  - server_core/tests/faction_flip_on_pc_wizard_damage.rs:1 — PC damages Wizard flips hostility flag.
  - server_core/tests/faction_vs_kind_independence.rs:1 — caster selection still works after changing kind; faction drives behavior.
  - net_core/tests/actorrep_v4_roundtrip.rs:1 — v4 roundtrip ensures faction+IDs survive encode/decode.

  If you want the sim_core scenario types renamed as well (their string field is still “team”), I can do that next for consistency.
