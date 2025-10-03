# 95D — Data Runtime: Destructible Budgets & Tuning

Labels: data, config, voxel
Depends on: Epic #95

Intent
- Move destructible tuning/budgets out of code and into `data_runtime` with CLI overrides.

Outcomes
- Server reads a destructible config at startup; CLI overrides can adjust; values logged once.

Files
- `crates/data_runtime/src/configs/destructible.rs` (new) — config struct + loader
- `crates/data_runtime/src/lib.rs` — export module
- `crates/server_core/src/destructible.rs` — wire CLI overrides to config (merge)

Config keys (initial)
- `voxel_size_m: f64`, `chunk: glam::UVec3`
- `aabb_pad_m: f64` (selector padding)
- Budgets: `max_remesh_per_tick: usize`, `collider_budget_per_tick: usize`, `max_debris: usize`, `max_carve_chunks: u32`
- Tuning: `close_surfaces: bool`, `seed: u64`

Tasks
- [ ] Define struct and loader (TOML or JSON; choose one consistent with repo).
- [ ] Map existing `server_core::destructible::config::DestructibleConfig` flags to override fields.
- [ ] Add unit tests: default load, CLI override merge.

Acceptance
- Server logs an effective destructible config on boot; values reflect CLI overrides; no hard‑coded constants remain in the carve/mesh/collider budget paths.
