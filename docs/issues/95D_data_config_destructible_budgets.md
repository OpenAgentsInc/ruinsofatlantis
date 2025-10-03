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
 - [ ] Provide a sample file `data/config/destructible.toml` and document path:
   ```toml
   voxel_size_m = 0.10
   chunk = [32, 32, 32]
   aabb_pad_m = 0.25
   max_remesh_per_tick = 4
   collider_budget_per_tick = 2
   max_debris = 1500
   max_carve_chunks = 64
   close_surfaces = false
   seed = 12648430
   ```
 - [ ] Precedence & validation:
   - Defaults in code < file values < CLI flags (highest). Test precedence.
   - Clamp insane values (e.g., `voxel_size_m >= 0.02`, `max_remesh_per_tick <= 256`), log clamped result once.
 - [ ] Wire into `server_core` (replace hard‑coded uses) in:
   - `server_core/src/destructible.rs` (budgets, debris caps), and future systems once added.

Acceptance
- Server logs an effective destructible config on boot; values reflect CLI overrides; no hard‑coded constants remain in the carve/mesh/collider budget paths.
 - Add a single boot line, e.g.:
   ```
   [destructible] cfg: voxel=0.10m chunk=32^3 pad=0.25m budgets(remesh=4,colliders=2) debris=1500 seed=0x00C0FFEE
   ```
