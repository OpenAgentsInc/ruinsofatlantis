# Next Steps — Small, Verifiable PRs

Principles
- Keep diffs behavior-neutral unless flagged.
- Prefer extraction + re-export shims first; remove shims only after adoption.
- Land unit tests alongside any new logic paths (CPU-only where possible).

PR 35 — Post suite extraction (behavior‑neutral)
- Create passes: `SsrPass`, `SsgiPass`, `PostAoPass`, `BloomPass` under `renderer/passes.rs`.
- IO: Read `hdr_color` (+ depth for SSR/SSGI), write `hdr_color`.
- Wire in graph order before Present; delete monolith code after parity check.
- Validation: hazard tests compile; visuals match legacy.

PR 36 — Pipelines split (mechanical)
- Move builders from `gfx/pipeline.rs` into `renderer/pipelines/{sky,terrain,instanced,post_ao,ssgi,ssr,bloom,present}.rs`.
- Keep `gfx/pipeline.rs` as a thin re-export during migration.
- Introduce typed wrappers in `renderer/pipelines/common.rs` and provide a `Pipelines` grouping.

PR 37 — ExecCtx.pipelines() + typed handles
- Thread `&Pipelines` through ExecCtx; passes use typed handles (no raw wgpu pipeline in pass code).
- Behavior-neutral: keep underlying storage unchanged; adaptors map old fields to typed view.

PR 38 — Main adopts DrawList
- Build DrawList before recording; group by (pipeline, material, mesh).
- Issue per-batch binds and draw calls; update RenderStats.batches to actual batch count.
- CPU-only tests: verify grouping determinism on a synthetic scene.

PR 39 — UploadRing adoption (2–3 hotspots)
- Replace small uniform/storage writes with ring slices + copy commands.
- Ensure single write path per buffer per frame; add counters to RenderStats.

PR 40 — RebuildBus test & coverage
- Add a generic core or integration test verifying registration order and single-fire semantics.
- Ensure all sized bindgroups rebuild via the bus.

PR 41 — Model viewer & platform splits
- Move `tools/model-viewer/src/*` into file modules; keep main.rs minimal.
- Split `platform_winit/src/lib.rs` into submodules (app/input/picker/
  builder_overlay/replication/telemetry); lib.rs becomes a facade.

Exit criteria
- No ≥1000‑line files under renderer/platform/tools; long files reduced below ~800–1200 LOC with cohesive content only.
- Graph drives all passes; legacy draw code deleted.
- CI remains green (fmt, clippy -D warnings, WGSL validation, tests, schemas).

