# Refactor Progress — What We Shipped vs. Plan

Accomplished (Phase‑Two highlights)
- Framegraph
  - Pass declarations for Sky/Main/Particles/UI/Present; execution for Main/Particles/UI/Present.
  - Hazard validation: intra‑pass RW ban and cross‑pass monotonic (no write after any read).
  - ExecCtx accessor surface (device/queue/color/depth views).
- Present lifecycle
  - Present pass owns acquire/present; robust error handling for Lost/Outdated/Timeout/OOM.
- Offscreen flow
  - Unified offscreen hdr_color → Present composite; pass IO aligns with actual usage.
- Infra components
  - BgCache (LRU-ish) with correctness fix and unit tests.
  - UploadRing (per‑frame bump allocator) with alignment test; initial adoption in hot path.
  - RebuildBus centralizes resize‑dependent rebuilds for sized bind groups.
- Observability
  - RenderStats per pass; HUD perf overlay shows batches/draws/cpu_ms; stats cleared every frame.
- Hygiene
  - Gated demos moved out of core; public re-exports preserved during transition.

Not yet complete (notable gaps)
- Long files still in place (see large-files-audit-v2.md); several modules exceed 2k LOC.
- Post suite still lives in monolith (`gfx/mod.rs`): SSR/SSGI/Bloom/AO not yet expressed as graph passes.
- Pipelines remain concentrated in `gfx/pipeline.rs`; typed wrappers namespace created but builders not migrated.
- DrawList not yet driving Main (stats conservatively set: batches = draws); grouping exists and is unit-tested.
- MSAA is threaded through declarations (sample_count = 1 today). Full MSAA adoption requires attachment/pipeline audit.

Impact (what changed vs. V1 audit)
- We now have an enforceable pass order and resource access model (graph + hazards) that prevents a class of frame hazards.
- Present’s ownership and error handling remove a category of acquire/present regressions.
- Cache/ring scaffolds enable reducing per-frame churn without touching draw code internals.

Key remaining wins
- Move the post suite to passes (behavior-neutral), unlocking hazard validation for post.
- Split pipelines into per-pass files; then thread typed wrappers through ExecCtx.
- Adopt DrawList in Main to reduce bind/draw state churn (batch grouping).

