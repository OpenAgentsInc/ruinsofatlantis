# Data Pipeline Audit (`crates/data_runtime`, `/data`, `/packs`)

Context
- Schemas present; SRD attribution; packs flow via xtask. Destructible tagging currently implied by ruins.

Recommendations
- Destructible tagging schema: per‑mesh flag + per‑instance override.
- IDs: typed handles for spells/classes/monsters; avoid free strings.
- Golden tests: pack content hash; scenario determinism; voxel proxy cache format versioning.
- Migration tools in xtask for schema evolution.
