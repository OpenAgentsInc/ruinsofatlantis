# Simulation Audit (`crates/sim_core`)

Context
- Deterministic sim runtime with clear modules: `rules`, `combat`, and `sim` (engine ECS & systems).
- Good separation from renderer/platform; integration tests exercise major flows.

Strengths
- Seeded RNG (`ChaCha8Rng`) and explicit tick loop; deterministic unit/integration tests.
- Rules split is clean (attack/saves/dice) and systems pipeline is easy to follow.

Pain Points
- Stringly-typed IDs and file-based fallbacks in `SimState::load_spell_spec_by_id` (`crates/sim_core/src/sim/state.rs:...`) couple engine to on-disk layout and naming.
- Systems reference implicit fields (e.g., underwater) without a typed policy model; scaling to more world policies will sprawl.
- Logging is ad-hoc strings; difficult to assert specific events beyond substring checks in tests.

Recommendations
1) Typed Handles for Content
- Introduce typed IDs for spells/classes/monsters with fixed namespaces (e.g., `SpellId(u32)`), backed by a content table built at startup from packs.
- Retain human-readable `spec_id: String` inside the spec for debugging, but internal references use typed handles.
- Remove heuristic fallbacks reading JSON by id substrings.

2) Scenario and Policy Modeling
- Extract environment/policy into a typed struct (e.g., `Environment { underwater: bool, gravity: f32, ... }`) injected into systems; simplifies tests and future features.

3) Event Log Structs
- Replace free-form strings with a typed `SimEvent { kind, actor, target, details }`; keep a compact text formatter for UI/diagnostics.
- Tests can assert on enum variants and fields rather than substrings.

4) Rules Validation
- Add property-based tests for dice and advantage mechanics; assert statistical bounds across seeds.
- Encode invariants: e.g., adding Bless never reduces hit chance.

5) Data Access
- Centralize spec access through a `SpecDb` facade in `data_runtime`; sim consumes an interface rather than direct file paths.
- For optional content, return typed errors early rather than scanning alternates.

Incremental Plan
- Phase 1: Introduce `SpecDb` and typed event log; keep string IDs for compatibility.
- Phase 2: Add typed `SpellId` indirection with a migration layer from strings; convert system storage.
- Phase 3: Remove path-based fallbacks; packs become required input in harness/tests.

