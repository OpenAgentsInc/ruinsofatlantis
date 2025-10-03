# 95P — Tests & CI Expansion

Labels: ci, testing
Depends on: Epic #95, early phases complete

Intent
- Expand unit/integration tests and CI checks to enforce determinism and budgets.

Tasks
- [ ] Unit tests: projectile integration determinism; collision candidate counts; carve voxel counts (tolerance); mesh quad counts after carve.
- [ ] Integration tests: server tick over N frames; assert entity counts/health deltas/dirty chunk sizes.
- [ ] Replication tests: encode/decode snapshots; interest filters.
- [ ] CI: ensure Naga WGSL validation (already in xtask), add `cargo deny` advisories, and perf smoke for budgets per tick.

Acceptance
- CI green with new suites; failures surface deterministic deltas.
