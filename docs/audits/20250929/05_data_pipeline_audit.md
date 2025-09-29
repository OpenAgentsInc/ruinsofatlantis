# Data Pipeline Audit (`crates/data_runtime`, `/data`, `/packs`)

Context
- `data_runtime` provides serde models and loaders; zone manifests have a JSON Schema.
- Packs are built via `xtask build-packs` (spellpack + zone snapshot helper) but do not yet have golden tests or versioned schemas for all content types.

Strengths
- Clear separation of runtime models and filesystem; schemas directory exists and is used for zones.
- Tools (`zone-bake`) produce deterministically ordered outputs and include fingerprints.

Pain Points
- Spells/classes/monsters rely on serde validation only; no schema or semantic checks.
- `SimState` performs path heuristics when resolving IDs (engine leakage into data concerns).
- No golden tests for pack bytes; accidental pack drift can go unnoticed.

Recommendations
1) Schema Coverage and Semantics
- Author JSON Schemas for `spells/*.json`, `classes/*.json`, and `monsters/*.json` mirroring serde models; validate via `xtask schema-check`.
- Add semantic passes (e.g., class spell lists reference existing spell IDs; level bands monotonicity).

2) Pack Versioning and Golden Tests
- Introduce explicit pack versions and content-hash headers (already started for spells); document under `/docs/systems`.
- Add golden tests that compare packed bytes and header fields; store small goldens in repo.

3) ID Discipline
- Establish canonical ID format (e.g., `wiz.fire_bolt.srd521`) and move aliasing/compat rules into a single normalization step in `data_runtime`.
- Provide a reverse map and a typed `SpecDb` to avoid ad-hoc file path probing.

4) Asset Policy Consolidation
- Centralize asset path discovery in `shared/assets` with a single workspace-first fallback; deprecate local `asset_path` helpers in other crates.
- Add tests for path preference (workspace assets override crate-local).

5) Draco & GLTF Policy
- Document that runtime prefers pre-decompressed `.decompressed.gltf`; enforce via loader warnings if Draco is detected at runtime.
- Provide a one-shot migrator in `xtask` for batch decompress and manifest rewrite when needed.

