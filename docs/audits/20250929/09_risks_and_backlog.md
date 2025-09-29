# Risks and Prioritized Backlog

Top Risks
1) Renderer orchestration complexity and resource lifetime ambiguity → regressions during feature growth.
2) Stringly IDs with fallback heuristics in sim → hidden coupling to file layout; brittle content changes.
3) Missing schemas/goldens for spells/classes/packs → silent content drift.
4) Lack of shader validation and dependency policy enforcement → runtime/device breakages and security advisories.
5) Divergent asset path resolution and Draco policies across crates.

Prioritized Backlog (Impact × Effort)
P0 – High Impact, Low/Med Effort
- Introduce WGSL validation in `xtask ci`.
- Add `cargo deny` checks.
- Add golden tests for `spellpack.v1.bin` (size, header, hash).
- Consolidate asset path policy in `shared/assets` + tests; deprecate local helpers.

P1 – High Impact, Med Effort
- Renderer: Extract `Attachments` and `Pipelines` structs; centralize resize & BGL management; module docs.
- Sim: Typed event log; `SpecDb` facade; remove ad‑hoc path probing from sim.
- Author schemas for spells/classes/monsters; enforce in `xtask schema-check`.

P2 – Medium Impact, Med Effort
- Headless renderer CPU hashing tests for terrain/instances/palettes.
- Scenario/env policy injection; remove direct field reads for global flags.
- Toolchain pin and CI cache.

P3 – Medium Impact, Higher Effort
- Minimal frame-graph for render passes + resource lifetimes.
- Typed IDs (`SpellId`, `ClassId`, `MonsterId`) and pack-loaded handle tables; migration shim.

Definition of Done (examples)
- Renderer resize idempotent test: reconfigure N times, attachments identical; no leaks.
- Golden spells pack stable across runs; failure shows diff and source of nondeterminism.
- `cargo deny` passes with documented exceptions.

