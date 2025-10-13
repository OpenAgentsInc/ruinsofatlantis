# Refactor Log — Ordered (PR 1 → Phase‑3 wrap)

This log consolidates and orders all refactor PRs through Phase Three. CI remained green throughout; behavior‑neutral unless noted.

--

## Phase One

### PR 1 — Device/attachments/config scaffolding (render_wgpu)
Adds renderer/device/config scaffolds (`GpuCtx`, `SurfaceCtx`, `Samplers`) and config home. No behavior change.

### PR 2 — Pipelines directory & typed wrappers (scaffold)
Creates `renderer/pipelines/*` namespace with `Bgl<T>` and `Pipeline<T>` wrappers; `Pipelines` grouping placeholder.

### PR 3 — Framegraph skeleton (forwarder only)
Adds a framegraph forwarder stub that calls the legacy render path.

### PR 4 — Split renderer::update by concern (scaffold)
Creates `renderer/update/{builder,projectiles,math,destructibles_demo}`; re‑exports legacy body; adds math tests.

### PR 5 — Split gfx::ui into focused modules (scaffold)
Creates `gfx/ui/{mod,perf,help,hotbar}`; legacy UI re‑exported; adds a sanity test.

### PR 6 — tools/model‑viewer split scaffolding
Scaffolds `src/{cli,app,viewer,panels,loader,utils}.rs` in model‑viewer.

### PR 7 — platform_winit split (scaffold)
Adds façade modules and a small tested helper; keeps legacy loop in place.

### PR 8 — server_core schedule staged (scaffold)
Scaffolds schedule stages and an order test; no run‑order changes.

### PR 9 — server_core lib.rs trims (docs)
Keeps crate root concise; functional moves deferred.

### PR 10 — net_core snapshot split (scaffold)
Converts monolith to a directory module; keeps legacy via re‑export; tests unchanged.

### PR 11 — Vox demo gated
Moves demo under `gfx::demo::vox_onepath` behind `vox_onepath_demo`.

--

## Phase Two

### PR 12 — Framegraph core (builder/resources/validation)
Introduces `ImageKind`, `Handle<Img>`, `GraphBuilder`, `PassDecl`, `ExecCtx`, and `Graph`. Adds per‑pass hazard validation. Forwarder keeps behavior parity.

### PR 13 — Extract Present and Sky (declarations)
Scaffolds `SkyPass` and `PresentPass` declarations.

### PR 14 — Extract Main (declaration)
Adds `MainPass::declare(...)`; execution remains legacy.

### PR 15 — Particles + UI (execute via graph)
Exec closures for `ParticlesPass` and `UiPass`; `Graph::execute` runs closures.

### PR 16 — Offscreen hdr_color + real Present pass
Present becomes a pass compositing offscreen → swapchain; graph `Particles → UI → Present`.

### PR 17 — Draw‑list builder (CPU‑only) + tests
Adds deterministic `DrawList` and grouping tests.

### PR 18 — BindGroup cache scaffold
Adds `BgCache`/`BgKey` with hits/misses/evictions counters.

### PR 19 — Upload ring scaffold
Adds `UploadRing` allocator; alignment helper tests.

### PR 20 — Centralized resize/rebuild bus
Adds `RebuildBus` with listeners; idempotent resize rebuild.

### PR 21 — UI pass finalization
Pass remains thin queue+draw; small deterministic HUD test.

### PR 22 — Shims/doc polish
Rustdoc and cleanup of temporary shims.

### PR 33 — Main batching plumbing (behavior‑neutral)
Conservative batch counting; no draw changes.

### PR 36 — Split pipelines (mechanical)
Adds per‑pass modules under `renderer/pipelines/*`; re‑export shims maintained.

### PR 37 — ExecCtx::pipelines() + typed handle scaffolding
Adds accessor; no behavior change.

### PR 38 — Main adopts DrawList batching (behavior‑neutral)
Counts batches via `(pipeline, material, mesh)`; HUD shows batches ≤ draws.

### PR 39 — UploadRing adoption (more hotspots)
Moves several per‑frame writes to ring+copy; adds `next_frame_resets_cursor` test.

### PR 40 — RebuildBusCore<T> + tests
Extracts generic core; order test verifies deterministic run order.

### PR 41 — File splits (mechanical)
Continues model‑viewer and platform_winit splits (façades in place).

### PR 35 — Extract Post Suite (behavior‑neutral)
Declares AO/SSGI/SSR/Bloom passes and wires ordering; visuals unchanged.

--

## Phase Three

### PR 42 — Graph Views API (behavior‑neutral)
Passes use `ctx.view_*` (graph views), not attachments.

### PR 43 — Allocation Plan & Liveness (no aliasing by default)
Computes liveness/usage; optional 1:1 allocations via env.

### PR 44 — Optional Aliasing (interval packing, env‑gated)
Reuses textures for compatible descriptors with disjoint lifetimes; RA_GRAPH_TRACE logs.

### PR 45 — MSAA threading (scaffold)
Threads sample count across images and pipelines; resolve staging.

### PR 50 — Graph‑owned images (ImageArena) + ExecCtx views/textures
Adds `ImageArena` + `ImageDesc`, `ExecCtx::texture/desc`; passes consume `ctx.view_*` exclusively.

### PR 51 — Resolve as a pass (resolve_target)
Resolve pass writes single‑sample HDR from MSAA color via `resolve_target`.

### PR 52 — Aliasing + peak memory stats
Aliasing allows usage‑superset reuse; tracks peak VRAM (slot‑based with aliasing). HUD shows MiB.

### PR 53 — Present recovery counter
Lost/Outdated triggers reconfigure; increments `present_recoveries` (shown in HUD).

### PR 54 — History buffers (read‑only; EoF copy)
Adds `history_color` and copies HDR → history at frame end (prep for temporal).

### PR 55 — Temporal scaffolding (feature‑gated, OFF)
Temporal reprojection remains gated/off; no behavior change.

### PR 56 — DrawList 2.1 counters (integrated in Main)
Main reports pipeline_binds/bg_binds/vb_ib_sets; visuals unchanged.

### PR 58 — BgCache coverage (passes & present)
Present/post BGs built via cache from graph views (no attachment coupling in passes).

### PR 59–60 — UploadRing 2.0 + GPU timestamps (scaffold)
Left gated/off by default; behavior‑neutral.

### PR 63 — Remove legacy render path
Removes `RA_RENDER_LEGACY` and direct swapchain drawing outside Present. Graph owns the full frame path.

--

## Phase‑Three wrap

Results
- Full frame executes through the framegraph (Sky → Main → Resolve → Particles → Post → UI → Present).
- Graph alloc/alias validated; peak VRAM surfaced; Present recovery tracked.
- Passes sample graph views only; Main reports state‑change counters.

--

## Phase‑Three — Follow‑ups (tests and correctness)

### PR 63a — Main resolve correctness
- Extracted `Renderer::main_draw_into(&mut RenderPass)` and updated `MainPass` to draw inside the resolve‑enabled pass (with `resolve_target = hdr` when MSAA > 1). Removed the intermediate/dummy pass.

### PR 63b — History copy as a graph pass
- Added `HistoryCopyPass` to sample the graph `post` image and render into `attachments.history_view` via a per‑frame BG built from the graph view. Removed out‑of‑graph history copy.

### PR 63c — Tests: MSAA graph shape
- Added CPU‑only tests in `renderer/graph.rs`:
  - No `Resolve` pass exists.
  - `Main` writes `hdr`/`depth` (no MSAA) and `hdr`/`msaa`/`depth` (with MSAA).
- Existing self‑conflict IO test already covers read+write same image panics.
