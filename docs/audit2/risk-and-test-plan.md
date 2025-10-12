# Risk & Test Plan (Phase‑Two Follow‑Up)

Risk controls
- Keep graph hazards strict until aliasing is introduced: intra‑pass RW ban and cross‑pass monotonic rule.
- Preserve public re-exports when moving modules to avoid API regressions; remove only after adoption.
- Never run interactive apps in automation; use `xtask ci` locally.
- Feature‑gate demos; assert default features exclude demo paths.

Unit tests to maintain/extend
- Framegraph
  - Should panic when write occurs after a read of the same image.
  - Preserves pass declaration order when no dependencies.
- DrawList
  - Merges contiguous items with identical keys; does not merge across different keys.
  - Deterministic grouping on synthetic inputs; add empty‑list test.
- BgCache
  - Counts: hits/misses/evictions; recency refresh.
  - Device‑backed tests gated or skipped if adapter unavailable.
- UploadRing
  - Alignment helper correctness (done); add cursor reset on `next_frame()` via test‑only getter if exposed.
- RebuildBus
  - Convert to a tiny generic core or integration test for order. Avoid unsafe in unit tests; prefer a minimal struct.

Integration checks
- Present
  - Handle `SurfaceError::{Outdated, Lost, Timeout, OutOfMemory}` with resize/reconfigure as appropriate.
- Resize
  - Centralized `resize_impl` updates attachments and fires rebuild bus; sized bind groups are recreated.

CI matrix
- Default: `xtask ci` (fmt, clippy -D warnings, WGSL validation, tests, schema checks).
- Optional local: `--features wgpu-tests` for heavier device‑backed tests (not enabled in CI by default).

Telemetry
- Keep RenderStats stable; clear per frame; avoid expensive per‑pass allocations.
- As passes adopt DrawList/BgCache/UploadRing, report batch counts and cache hit/miss deltas.

