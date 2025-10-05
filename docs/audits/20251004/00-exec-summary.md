# Executive Summary — 2025-10-04

Scope
- Full workspace audit covering architecture, determinism, net/replication, server systems, ECS/data, renderer/GPU, assets/LFS, observability, security, build/CI, tests, and docs.

Scores (0–5)
- Architecture & Layering: 3 — Mostly clean boundaries; one renderer→server spawn violation (renderer initiating boss spawn).
- Network & Replication: 2 — Solid snapshot scaffolding; unbounded channel; missing versioning and backpressure.
- Determinism: 4 — Seeded RNG and deterministic queues; no `thread_rng` use found; stable chunk budget scheduling.
- Server Systems & Scheduler: 3 — Budgeted carve→mesh→collider orchestration present; metrics hooks limited.
- ECS & Data Model: 3 — Components well-defined; be cautious with `HashMap` iteration ordering.
- Renderer & GPU: 2 — Renderer hosts gameplay/input/AI; many `.clone()` hot-path allocations; surface-lost handling present but orchestration is monolithic.
- Asset Pipeline & LFS: 2 — LFS covers models/anims; add images/ktx2/bin patterns; remove absolute paths from docs/scripts where avoidable.
- Observability & Ops: 3 — Tracing + optional Prometheus bootstrap; add counters/histograms for budgets, net bytes, queue depths.
- Security & Anti‑Cheat: 2 — Server-authority plan emerging; unbounded channels; decode paths are largely bounded.
- Build/CI/CD: 2 — `xtask ci` exists; `cargo fmt --check` reports diffs; `cargo test --no-run` fails on unresolved dev-deps; add `cargo deny`.
- Tests & Quality: 3 — Many unit tests across crates; compilation of tests currently fails due to missing dev-deps in some crates.
- Docs & Governance: 4 — Good hygiene and playbooks; keep `src/README.md` aligned as renderer refactors land.

Top Risks
- Renderer performs gameplay and world mutation (input/controller, simple AI, uploads) inside `render_impl` instead of ECS/client/server separation (crates/render_wgpu/src/gfx/renderer/render.rs:12).
- Unbounded replication channel (`net_core::channel`) risks memory growth without backpressure (crates/net_core/src/channel.rs:13).
- Build hygiene gaps: `cargo fmt --check` shows diffs; `cargo test --all --no-run` fails with unresolved dev-deps (docs/audits/20251004/evidence/fmt-diff.txt, docs/audits/20251004/evidence/warnings.txt).

30/60/90 Plan
- Now (2 weeks)
  - Move boss spawn and any server state mutation out of renderer; expose via `server_core` APIs called from app/platform layer.
  - Fix test build failures by adding missing dev‑deps for crates using `core_*` types in test modules.
  - Add basic backpressure: bound replication channel or migrate to a limited ring buffer.
  - Enforce `cargo fmt` in CI and fix current diffs; enable `cargo deny` (optional in xtask today).
- Soon (30–45 days)
  - Extract gameplay/input/AI from renderer into `client_core` systems; keep renderer to upload/draw.
  - Add message version headers and size caps in `net_core::snapshot`.
  - Expand metrics: per‑tick times, budgets observed, bytes sent, queue depth; add dashboards.
  - Extend LFS patterns to include textures and binary buffers.
- Later (90+ days)
  - Introduce interest management and delta cadence budgeting; add prediction/reconciliation hooks.
  - Lightweight frame‑graph for renderer with explicit read/write edges; job offload for meshing/colliders.

