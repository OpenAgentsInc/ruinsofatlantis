# Risk Matrix — 2025-10-04

Legend: Severity P0–P3 × Likelihood Low/Med/High. See 99-findings-log.md for details.

- P1 × High — Renderer triggers server spawn (F-ARCH-001) — render path violating layering (crates/render_wgpu/src/gfx/npcs.rs:116).
- P1 × High — Renderer hosts gameplay/AI/input (F-ARCH-002) — logic inside `render_impl` (crates/render_wgpu/src/gfx/renderer/render.rs:12).
- P1 × Med — Test build failures (F-CI-005) — unresolved dev‑deps (evidence/warnings.txt).
- P1 × Med — Server `unwrap` in systems (F-SIM-009) — panics on hot/server paths (evidence/panics-server.txt).
- P2 × Med — Unbounded replication channel (F-NET-003) — backpressure risk (crates/net_core/src/channel.rs:13).
- P2 × Med — Hot-loop allocations/clones (F-RENDER-004) — renderer/server copies (evidence/hotloop-allocs.txt).
- P2 × Med — Missing versioning/caps in snapshot (F-NET-014) — forward/backward compat plan.
- P2 × Low — LFS patterns incomplete (F-ASSET-007) — textures/buffers not tracked by LFS.
- P2 × Low — Observability coverage gaps (F-OBS-011) — missing key counters/histograms.
- P3 × Low — Absolute paths in docs/scripts (F-DOCS-008) — replace with relative/env-based examples.
- P3 × Low — `HashMap` iteration ordering caution (F-DET-010) — ensure stable ordering in authoritative loops.
- P3 × Low — Unsafe surface lifetime transmute (F-RENDER-012) — ensure narrow scope and handle fallbacks.

