# 95H — Data Runtime: Projectile SpecDb

Labels: data, combat
Depends on: Epic #95

Intent
- Move projectile parameters (speed, radius, damage, lifetime) out of code and into `data_runtime`, exposed via SpecDb.

Outcomes
- Server projectile systems use SpecDb instead of hard-coded values. Client can read a read-only view for prediction.

Files
- `crates/data_runtime/src/specs/projectiles.rs` (new) — schema + loader + validator
- `crates/data_runtime/src/specdb.rs` — wire projectiles into SpecDb indices/getters
- Data files: `/data/projectiles/*.json|toml` (choose format consistent with repo)

Tasks
- [ ] Define schema: `{ id: string, speed_mps: f32, radius_m: f32, damage: i32, life_s: f32 }` and loader.
- [ ] Extend `SpecDb` with `projectiles: HashMap<String, ProjectileSpec>` and getters.
- [ ] Add unit tests: load one file, index by id with variations (canonical, last segment, name-key) similar to spells.
- [ ] Document defaults and validation (e.g., clamp absurd values).

Acceptance
- Server projectile systems can query SpecDb for projectile params by id; no hard-coded constants remain.
