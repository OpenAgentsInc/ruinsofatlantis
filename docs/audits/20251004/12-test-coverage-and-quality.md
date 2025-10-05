# Tests — Coverage & Quality — 2025-10-04

Inventory
- Broad unit test presence across server_core, render_wgpu (CPU-only), voxel_mesh/proxy, data_runtime, net_core, client_core (evidence/tests-grep.txt).

Status
- `cargo test --all --no-run` fails due to unresolved imports in test modules for `collision_static` and `voxel_mesh` (evidence/warnings.txt).

Gaps
- Orchestration/system tests for server tick budgets across carve→mesh→colliders beyond unit checks.
- Replication round-trips exist; add size caps and malformed input tests.
- CPU renderer tests present; extend with hash tests for terrain/mesh invariants.

Findings
- F-CI-005: Fix test build failures; ensure CI runs tests green (P1 Med).

Recommendations
- Add missing dev-deps for tests; consolidate common test helpers.
- Add deterministic system/orchestration tests with fixed seeds; keep execution under fast thresholds for pre-push hooks.

