# Testing Strategy Audit

Context
- Strong unit/integration tests for data/sim; good CPU‑only coverage for voxel proxy/mesher.
- Missing orchestration tests, network/replication tests, and ECS system tests.

Recommendations
- System tests (pure): projectile integrate, collision pairs, carve voxel counts (tolerance), chunk budgets, debris mass.
- Orchestration: server tick with fixed seed → assert entity counts/health deltas/dirty chunks.
- Replication: snapshot encode/decode round‑trips with baselines and spatial interest filters.
- Renderer CPU tests: build terrain buffers and chunk meshes → hash; verify winding/normal invariants.
- CI: run Naga WGSL validation (done), `cargo deny` advisories, perf smoke (frame build ≤ budgets).
