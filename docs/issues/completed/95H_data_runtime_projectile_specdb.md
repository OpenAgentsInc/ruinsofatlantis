# 95H — Data Runtime: Projectile SpecDb

Status: COMPLETE

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
 - Reference existing SpecDb patterns:
   - `crates/data_runtime/src/specdb.rs` currently indexes spells/classes/monsters with id/last-segment/name-key variants; mirror the same conveniences for projectiles.

Tasks
- [x] Define schema and loader under `crates/data_runtime/src/specs/projectiles.rs`.
- [x] Provide `ProjectileSpecDb` mapping action names (AtWillLMB/RMB, EncounterQ/E/R) to params; default fallback when file absent.
- [x] Unit tests: defaults present; server systems can spawn using SpecDb.
- [x] Document sample config under `data/config/projectiles.toml`.

Acceptance
- Server projectile systems can query SpecDb for projectile params by id; no hard-coded constants remain.
 - Lookup works for canonical id (`wiz.fire_bolt.srd521`-style), last segment, and name_key forms.

---

## Addendum — Implementation Summary

- SpecDb: `crates/data_runtime/src/specs/projectiles.rs` with `ProjectileSpecDb` loads `data/config/projectiles.toml` or falls back to defaults; unit test validates presence of defaults.
- Server wiring: `server_core::systems::projectiles::spawn_from_command()` maps `InputCommand` to SpecDb action and spawns projectiles accordingly; tests validate direction and params.
