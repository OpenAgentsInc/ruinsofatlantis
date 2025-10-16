### Wishcrafting (Profession)

Overview
- Wishcrafting is the design, binding, simulation, and execution of high‑impact changes to the world using structured “wishes.” It bridges fantasy genie lore with a real agentic planning loop: intention → constraints → shadow‑run → adjudication → commit → audit.
- Identity: Wishcrafters craft safe, precise, rules‑lawyerable wishes. You still “cast” a wish at the end, but the gameplay is in specification and validation.
 - Name: Wishcrafting (profession); practitioners are Wishcrafters.

Design Tenets
- Promises, not spells: wishes are structured promises made enforceable by bindings and audits.
- Monkey’s Paw as gameplay: ambiguity and side‑effects are story engines you can predict and manage.
- Same scaffolding, two layers: schema, court, and ledger mirror the real agentic toolchain powering development.

Core Loop (Prompt → Plan → Safe Commit)
- Intention: author a natural‑language wish.
- Binding: add clauses (scope, invariants, budget, tools).
- Shadow‑Run: simulate consequences on a staging shard; produce Echo Report (diff of predicted changes and impacts).
- Adjudication: Wish Court scores Clarity, Safety, Impact; peers can add amicus clauses.
- Commit: cast wish; log to the Wish Ledger with rollback anchor.
- Heat & Echo: generate Paradox Heat (risk/aggro) and Reality Echoes (side‑quests/anomalies) proportional to scope and novelty.

Safety & Constraints (Elegant Nerfs)
- Paradox Heat: global meter; high heat invites Paradox Storms/Inquisitors. Decays over time; reduced by cooling rituals.
- Anchor Invariants: unbreakable rules (e.g., no deleting agency, no infinite currency). Learned as collectible Anchor Runes.
- Clarity Tax: ambiguity is interpreted adversarially by genies; clarity perks reduce drift.
- Budgeted Reach: wishes consume Chrono‑Sand (reagent), Genie Slots (concurrency), and Binding Seals (permissions).
- Reversibility Score: big wishes must include a rollback plan; low reversibility raises scrutiny and heat.

Scope Tiers & Cadence
- Micro: personal/party, local cell; frequent (daily).
- Meso: zone/sub‑region; occasional (weekly).
- Macro: region/server‑level; rare, scheduled events (monthly/seasonal).
- Each tier sets caps on budgets, court thresholds, and max Heat deltas.

Progression Tree (Tooling Roadmap)
- Clarity (prompt engineering): Concrete Speech, Guarded Negations, Schema Casting (semi‑structured Wish Schema).
- Reach (tool access): unlock Weather/Trade/Cartography/Logistics; Choral Wishes (co‑op orchestration); Realm Hooks.
- Stability (simulation/safety): faster Shadow‑Runs, Invariance Lattices (templates), Echo Dampeners.
- Jurisprudence (governance/negotiation): Wish Court Advocate, Genie Rapport, Public Mandate (petition signatures reduce heat).
- Capstones: Wishwright (multi‑stage reforms), Paradox Auditor (detect/patch ripples), Grand Compiler (turn lore into wish templates).

Multiplayer
- Petitions & Signatures: post wishes publicly; backers contribute resources/seals or cooling rituals.
- Wish Duels (PvP): time counter‑wishes; reinterpret clauses; push opponent heat beyond safe limits.
- Choral Orchestration (Raids): 10‑player synchronized wishes (schema authoring, sims, logistics, defense vs. Inquisitors).

Narrative Bedrock
- Bootstrap paradox: future Wishwrights learned from the First Crafter’s early sketches (your shipped systems). The Wish Ledger doubles as scripture; prophecy bugs mirror repo merges.

Dev Harness Parallels (Diegetic = Useful)
- Wish Schema (DSL): typed template with `objective`, `scope`, `invariants`, `budget`, `tools`, `safety_tests`, `rollback` (YAML/JSON + lints).
- Shadow‑Run Sandbox: staging sim that executes plans on snapshots; outputs Echo Reports.
- Wish Court Bot: evaluator scoring clarity/side‑effects/reversibility; suggests clauses (a CI gate).
- Genie Registry: catalog of tools/APIs with caps and costs (permissioned connectors).
- Wish Ledger: versioned, human‑readable, diffable logs; rollback anchors.
- See: docs/gdd/11-technical/overview.md for systems overview.

Event Hooks
- The Day the Sky Was Recompiled: server‑wide choral wish that re‑routes storm dragons; logistics and heat management mini‑season.
- Echo Bloom: ambiguous prosperity wish spawns Echo Doppelgangers; fix via amendment invariant.
- The Anchor Heist: steal a First Era Anchor Rune to enable ocean‑floor terraforming under strict pacifist clauses.

Good vs. Monkey’s Paw Examples
- Bad: “Make my guild rich.” → market floods, inflation, heat spike, Inquisitors.
- Good (schema’d): objective + regional scope + invariants (no price impact >1%, no currency duplication, NPC welfare), budget, concrete plan, safety tests, rollback.

Wish Schema (Lightweight)
```
wish:
  title: "Stabilize the Western Sea Lanes"
  objective: "Reduce pirate attacks by 40% over 14 days without increasing naval or civilian casualties."
  scope:
    region: "Western Sea"
    duration_days: 14
  invariants:
    - "No increase in civilian deaths"
    - "No city-state sovereignty changes"
    - "Trade price index delta <= 1%"
  budget:
    chrono_sand: 3
    genie_slots: 4
    gold_cap: 10000
  tools:
    - "Cartography.Gen-Pathfinder"
    - "Logistics.ConvoyPlanner"
    - "Weather.StormOracle"
    - "Diplomacy.Broker"
  plan:
    - "Map low-risk corridors using last 90d incident data"
    - "Coordinate 6 convoy windows; hire airborne scouts"
    - "Broker ceasefire with Corsair factions via bonded oaths"
  safety_tests:
    - "Sim pirate displacement effects"
    - "Stress test convoy windows under storm variance"
  rollback:
    - "Dissolve oaths; revert patrol routes; publish notice"
```

Economy & Social Systems
- Wish Markets: commission contracts priced by scope, risk, heat.
- Insurance: Heat Insurance funds cooling rituals on overheats.
- Open Templates: high‑quality schemas become Wishcards; owning one reduces commit cost.

UX Pillars
- Ambiguity Meter: live feedback on risky phrasing; shows which clauses reduce drift.
- Before/After Diff Cards: predicted vs. actual world diffs post‑commit.
- Persona‑Aware Genie Picker: match genie temperament to wish type; expose risk tradeoffs.

Telemetry & KPIs
- Time‑to‑First‑Impact (new Wishcrafter → first successful micro wish).
- Average Clarity Score and Ambiguity Meter dwell time.
- Heat generated per commit by tier; Overheat Rate.
- Rollback Frequency and Amendment Acceptance Rate.
- Template Reuse Rate (Wishcard adoption).
- Choral Participation (unique participants; retention across events).

Live‑Ops & Tuning Hooks
- Anchor Rune Rotation to adjust invariants seasonally.
- Heat Decay Curves per global/region to control cadence.
- Court Strictness thresholds per tier to manage throughput.
- Echo Tables to vary “fun failure” families over time.

Open Questions (Design Meditations)
- Anchor Invariants: what must never be wishable?
- Fun Failure: what’s the enjoyable recovery loop after a twisted wish?
- Progression Pacing: time to first meaningful impact; cadence of micro‑ vs. macro‑wishes.
- Governance: NPC vs. player Wish Court; appeal windows; amicus clauses.
- Awareness & Transparency: how public are Ledger entries?
- Real APIs First: which connectors are safe to bind initially?
- Community Rewards: royalties/fame/heat discounts for great templates; non‑Wishcrafter roles (sign, scout, simulate).
- Risk & Ethics: clear permission scopes; guardrails vs. lawful‑but‑awful griefing.

Scope (V1 Slice)
- Petition Board + Schema Linting, limited to micro‑scope wishes in one region.
- Shadow‑Run Diff Viewer (map diff + NPC routine deltas).
- Heat Meter + 2 Cooling Rituals.
- One Choral Wish Event to stabilize a harbor via logistics, weather, and diplomacy.

Optional Tuning Knobs (Ready for Balancing)
```
Heat = Base * ScopeFactor * NoveltyFactor * SpeedFactor * ReversibilityPenalty * ClarityPenalty * ChainLengthFactor - Mitigations

ScopeFactor: Micro 0.5, Meso 1.0, Macro 2.0
NoveltyFactor: 0.8…1.5 (vetted templates lower it)
SpeedFactor: 1.0 (normal) → 1.4 (instant)
ReversibilityPenalty: 0.8 (strong rollback) → 1.3 (weak)
ClarityPenalty: 0.8 (high clarity) → 1.4 (ambiguous)
Mitigations: cooling rituals, public mandate, genie match bonus
```

Court Thresholds (Example)
- Micro: Clarity ≥ 70, Safety ≥ 60, Reversibility ≥ 60
- Meso: Clarity ≥ 75, Safety ≥ 70, Reversibility ≥ 70
- Macro: Clarity ≥ 85, Safety ≥ 85, Reversibility ≥ 80 + petition quorum
