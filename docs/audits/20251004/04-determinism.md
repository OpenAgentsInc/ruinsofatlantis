# Determinism — 2025-10-04

Summary
- RNG: `server_core` uses `SmallRng` with explicit seeding (evidence/randomness.txt). No `thread_rng`/`random()` found in server core (evidence/randomness-sites.txt).
- Time: No `Instant::now()` found on server paths (evidence/server-blocking-globals.txt empty). Renderer uses frame time (client-only impact).
- Ordering: Authoritative destructible jobs use a deterministic queue and pop fixed budgets (crates/server_core/src/destructible.rs:208,229; crates/server_core/src/systems/destructible.rs:47).
- Maps: `ecs_core::ChunkMesh` uses `HashMap` for per-chunk meshes. Ensure iteration over chunk keys is sorted/stable when authoritative logic depends on order.

Findings
- F-DET-010: Ensure stable iteration on any `HashMap` used in server-side mutation paths (P3 Low).

Recommendations
- Keep all authoritative loops order-stable: use sorted vectors or `BTreeMap` or collect keys then sort.
- Continue to pass a single `dt` to systems and avoid wall-clock reads in hot/server paths.

