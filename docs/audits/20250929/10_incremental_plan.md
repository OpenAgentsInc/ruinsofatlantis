# 4‑Week Incremental Plan

Week 1 — Renderer Attachments & CI Boost
- Factor `Attachments` (color/depth/scene_read) and centralize resize logic; add unit tests for idempotent rebuilds.
- Add WGSL validation to `xtask ci`.
- Add `cargo deny` checks with baseline config.

Week 2 — Update Decoupling & Golden Packs
- Introduce a narrow `SceneInputs` trait or struct consumed by renderer; move controller/collision updates out of renderer.
- Add golden test for `spellpack.v1.bin` and `zone-bake` `zone_meta.json`.

Week 3 — SpecDb & Event Log
- Add `SpecDb` facade in `data_runtime`; refactor sim to use it for spec lookups.
- Replace free-form log strings with typed `SimEvent` and update tests accordingly.

Week 4 — Frame Graph & Docs
- Add a minimal static frame-graph to encode pass dependencies and resource barriers; simplify `render()` control flow.
- Document WGSL layouts and pass I/O in module headers; update `docs/systems` and `src/README.md`.

Outcomes
- Reduced renderer coupling and clearer resource lifetimes.
- Stable packs with schema coverage and golden tests.
- Sim becomes more testable with typed logs and clean data access.

