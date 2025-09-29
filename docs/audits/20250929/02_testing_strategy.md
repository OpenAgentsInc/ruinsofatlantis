# Testing Strategy Audit

Current State
- Integration tests under `tests/` cover data loading and simulation rules thoroughly (`tests/sim_systems.rs:1`, `tests/core_data.rs:1`).
- Unit tests exist in key CPU‑only renderer modules (camera, terrain, hiz, util) and in `ux_hud` and `collision_static`.
- `xtask ci` runs fmt, clippy (deny warnings), tests, and schema checks. No WGSL validation, no dependency policy checks, and no perf smoke tests.

Gaps & Risks
- Renderer orchestration logic not covered: resize paths, BGL rebuilds, pass enable/disable toggles, and resource lifetime invariants.
- No golden tests for content packs or zone snapshots; risk of accidental drift.
- No deterministic CPU hashing of renderer‑built buffers for headless validation.
- No property tests for dice/attack/save rules; limited fuzz of parser and ID resolution.

Recommendations
- Add headless renderer CPU tests:
  - Expose CPU‑only builders to produce vertex/index/instance buffers without a GPU device and hash results for a fixed seed and camera.
  - Verify pass toggles do not affect forbidden resource usage (e.g., sampling write‑targets).
- Introduce golden tests:
  - Spell pack: deterministic build via `xtask build-spells` → compare bytes with golden.
  - Zone bake: record zone_meta + collider footprint → compare JSON and binary headers (stable IDs/hashes).
- Schema expansion and validation:
  - Author JSON Schemas for `spells/*.json` and classes; validate in `xtask schema-check`.
- Property-based tests:
  - Dice parser and roller (e.g., ranges are respected; adding flat terms behaves as expected).
  - Attack/save systems monotonicity (e.g., higher AC never increases hit chance).
- Determinism hygiene tests:
  - Sim: assert no wall‑clock reads in hot path and only seeded RNG is used.
  - Renderer CPU builders: assert stable results for fixed seeds and camera.

CI Integration (extend `xtask ci`)
- Run Naga WGSL validation on all shaders.
- Add dependency policy gates via `cargo deny` (licenses, advisories, bans).
- Add a perf smoke subset (nightly/opt‑in): measure CPU frame build time envelopes and assert budgets.

