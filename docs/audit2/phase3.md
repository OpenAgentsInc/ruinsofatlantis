Phase Three is where we turn the scaffolding into a fast, clean, *production-grade* renderer & loop. Below is a full, end-to-end plan you can hand to the coding agent. It’s organized as a sequence of **small, verifiable PRs** with goals, steps, tests, and “done” criteria. Default rule still applies: behavior-neutral unless a PR explicitly says otherwise.

---

# Phase Three — Productionizing the Renderer & Loop

## North-star outcomes (what’s “done”)

* **True framegraph**: real, graph-backed images/buffers with lifetime/aliasing, MSAA resolve modeled as a pass, explicit Present ownership and recovery.
* **Modern post**: history buffers + temporal reprojection available to SSR/SSGI/AA; hazards enforce read/write truth.
* **Batched Main**: DrawList 2.0 with material state buckets, optional (feature-gated) GPU/compute culling; RenderStats reflect real batch & state-change counts.
* **Performance plumbing**: BG cache across all hot spots, UploadRing 2.0 for uniforms/storage, GPU timestamps + budget overlay.
* **Hygiene**: all long files retired or reduced; legacy code deleted; pipelines/types fully split; passes never poke raw wgpu state directly (typed ExecCtx).
* **Tests**: strong CPU tests; optional device tests behind feature flag; CI stays green.

---

## Track A — Graph → Real Resources, Aliasing, MSAA

### PR 50 — Virtual images → real allocations (no behavior change)

**Goal:** make graph handles map to actual `Texture`/`TextureView`s.

* Add an **ImageArena** that, during `Graph::compile`, creates concrete textures from `ImageKind` and stores `TextureView`s keyed by `Handle<Img>`.
* Extend `ExecCtx`:

  ```rust
  pub fn view_color(&self, h: Handle<Img>) -> &wgpu::TextureView;
  pub fn view_depth(&self, h: Handle<Img>) -> &wgpu::TextureView;
  ```

  (Now they *really* return graph-owned views.)
* Passes must stop reading `renderer.attachments.*` and only use `ctx.view_*`.

**Tests (CPU):** existing hazard tests continue; add a compile-time assertion that two distinct handles produce distinct slots in the arena (logic-only).

**Done when:** all current passes use `ctx.view_*` exclusively; visuals unchanged.

---

### PR 51 — MSAA Resolve as a pass (behavior-neutral)

**Goal:** model MSAA explicitly.

* If MSAA > 1, declare `msaa_color` (MSAA) and `hdr_color` (single-sample); **Main writes `msaa_color`**, add `ResolvePass` that **reads `msaa_color` → writes `hdr_color`**.
* If MSAA == 1, Main writes `hdr_color` directly (builder branches at build time).

**Tests (CPU):** hazard: no writes to `msaa_color` after Resolve.

**Done when:** toggling sample count swaps pass layout, visuals unchanged.

---

### PR 52 — Resource aliasing (transients)

**Goal:** recycle transient textures to reduce VRAM.

* Extend compiler to detect non-overlapping lifetimes and alias color/depth temps (post sub-passes) into shared allocations.
* Add simple **peak memory** estimation in Graph compile (sum of non-aliased image footprints) and surface it in the perf overlay.

**Done when:** perf overlay shows “graph mem peak”; memory drops when post suite enabled; visuals unchanged.

---

### PR 53 — Surface recovery (complete Present recovery)

**Goal:** bullet-proof Present.

* Handle `Outdated/Lost/Timeout/OOM` robustly:

  * Lost/Outdated → reconfigure surface + clear the image arena (force rebuild next frame).
  * Timeout → skip; OOM → log and bail for the frame.
* Record “present recoveries” counter in perf overlay.

**Done when:** aggressive window resize/min/max cycles don’t crash; counter increments.

---

## Track B — Post Suite → Temporal & History

### PR 54 — History buffers (read-only hazards)

**Goal:** enable temporal reprojection.

* Introduce a **history registry** in the graph: `history.hdr_color_prev`, `history.depth_prev`.
* At frame end (after Present), swap current → history.
* Mark passes (SSR/SSGI/AA) with `.reads(history.*)` where applicable.

**Done when:** hazards honor history reads; visuals unchanged (no temporal filter yet).

---

### PR 55 — Feature-gated TAA/Temporal SSR (optional, behavior change but gated OFF by default)

**Goal:** wire temporal reprojection behind `feature="temporal"`.

* Add `TaaPass` that **reads(current hdr_color, velocity, history)** and **writes hdr_color**; do the simplest box or clamped blend.
* For SSR/SSGI, allow mixing history term if feature is on.
* Off by default; add perf counters for “temporal reads”.

**Done when:** feature off = identical output; on = stable, no flicker regressions in simple camera pans.

---

## Track C — Main Batching, State & Culling

### PR 56 — DrawList 2.1: material buckets + state counters

**Goal:** minimize state churn, observe it.

* Extend `DrawKey` → `(pipeline_id, material_id, mesh_id, flags)`; group by (pipeline, material) first.
* Track per-frame counters in `RenderStats`: `pipeline_binds`, `bg_binds`, `vb_ib_sets`.
* Build buckets from the existing scene, emit per-batch binds + draws (no visual change).

**Tests (CPU):** given a sequence with repeated (pipeline,material), verify grouping reduces binds.

**Done when:** perf HUD shows reduced state changes with identical visuals.

---

### PR 57 — Optional compute frustum culling (feature `gpu_cull`, OFF by default)

**Goal:** demonstrate a clean plug-in for culling.

* Add a small compute pass that reads instance AABBs + view-proj and writes a visible bitset.
* In Main, filter DrawList with bitset (or use indirect draws if you already support).
* Off by default; perf HUD shows “culled N/M”.

**Done when:** enabled mode drops draw calls for off-screen meshes; default remains identical.

---

## Track D — Performance Plumbing

### PR 58 — BgCache everywhere + eviction telemetry

**Goal:** stop churn across remaining hot spots.

* Audit all per-frame `create_bind_group` calls; route through `BgCache`.
* Add per-pass deltas (hits/misses/evictions) to Stats; add a global cache size line.

**Tests (CPU):** generic LruMap tests already exist; add a sanity test that repeated `get_or_create` increments hits.

**Done when:** perf HUD shows low misses after warmup, stable cache size.

---

### PR 59 — UploadRing 2.0 (map-once buffers)

**Goal:** reduce queue traffic where we can.

* Add a map-once, persistently-mapped staging buffer variant for platforms that allow it (feature gated).
* Fallback to your existing Queue-write + copy path.
* Add ring metrics: bytes staged per frame, allocations count.

**Tests:** extend alignment + next_frame tests; device test behind `wgpu-tests` optional.

**Done when:** counters reflect staging use; visuals unchanged.

---

### PR 60 — GPU timestamps (feature `gpu_timestamps`, OFF by default)

**Goal:** measure on-GPU pass time.

* Add timestamp queries and write per-pass GPU ms into Stats (when feature on and backend supports).
* Fallback to CPU timing only when off.

**Done when:** perf HUD shows GPU ms for passes when enabled; CI remains green (feature off).

---

## Track E — Pipelines & Shaders Hygiene

### PR 61 — Full pipelines adoption in passes

**Goal:** passes no longer touch raw `wgpu::RenderPipeline`s or layouts.

* Finish `renderer/pipelines/*` migration and ensure access through `ExecCtx::pipelines()` only.
* Add rustdoc to each builder about formats/sample counts and required inputs.

**Done when:** `grep -R RenderPipeline` inside passes only finds wrapper types.

---

### PR 62 — WGSL module structure + hot reload (dev-only feature)

**Goal:** faster shader iteration.

* Introduce a simple WGSL module system (shared includes) and dev-only hot reload that re-creates pipelines & BGs via RebuildBus.
* OFF by default; enabled only in dev runs.

**Done when:** `touch shader` triggers rebuild with no restart; CI unaffected.

---

## Track F — Cleanup & Legacy Removal

### PR 63 — Remove legacy render paths

**Goal:** delete old code once graph owns everything.

* Confirm `RA_RENDER_LEGACY=1` is no longer necessary; delete code paths and flag.
* Remove any direct swapchain drawing paths outside Present.

**Done when:** only graph drives frame; Present owns acquire/present; CI & visuals good.

---

### PR 64 — Long-file purge final

**Goal:** finish the large-file plan.

* Ensure `gfx/mod.rs`, `render.rs`, `init.rs`, `ui/legacy.rs` all reduced:

  * `gfx/mod.rs` < ~1200 LOC (module plumbing only)
  * `render.rs` ~≤800 LOC (frame prep + graph build/exec only)
  * `init.rs` contains init only; per-pipeline builders moved
  * `ui/*` split with legacy left only as a thin draw facade

**Done when:** `large-files-audit-v2.md` shows no renderer files ≥1000 LOC unless inherently cohesive.

---

## Tests & CI matrix (Phase Three)

* **CPU tests** (always on):

  * Graph hazards (write after read) + order preserving.
  * DrawList grouping determinism.
  * LruMap hits/misses/evictions + recency refresh.
  * UploadRing: align, next_frame reset; (optional) expose a `cfg(test)` getter for cursor.
  * RebuildBusCore order.
* **Optional device tests** (`--features wgpu-tests`, not on CI):

  * Timestamp support probe (skip if not supported).
  * BindGroup creation smoke test with distinct labels (assert we get the right cached one by key).
  * UploadRing staging copy to a MAP_READ buffer.

**CI guardrails**

* Keep `clippy -D warnings`.
* WGSL validation must pass (no shader regressions).
* Demo features remain **off** by default.

---

## Perf HUD (what to surface by the end of Phase Three)

* **Per pass:** CPU ms, GPU ms (if enabled), draws, batches, pipeline binds, BG binds, VB/IB sets, BG cache hits/misses/evicts.
* **Global:** graph peak memory, present recoveries, bytes staged this frame (UploadRing), cache size.
* Toggle via perf panel; hidden cost is near-zero when off.

---

## Documentation & logs (what to update per PR)

* `docs/audit/log.md`: add “Changes/Validation” for each PR (50–64).
* `docs/audit/large-files-audit-v2.md`: update file sizes and mark “Split/Extracted.”
* `docs/audit/refactor-progress.md`: move items from “Gaps” → “Done.”
* `docs/audit/next-steps.md`: keep the remaining cards in order; always show *one step ahead* (next PR).
* `docs/audit/risk-and-test-plan.md`: add new tests/risks as they emerge.

---

## Quick start for the agent (first four PRs to ship now)

1. **PR 50**: implement ImageArena, switch passes to `ctx.view_*`.
2. **PR 51**: add Resolve pass for MSAA; branch when samples > 1.
3. **PR 35** (in parallel scope-wise): extract Post passes; wire order; record stats.
4. **PR 36**: split pipelines; add `Pipelines` grouping + re-export shim.

Land each with `xtask ci` green and a short log entry. Then proceed down Tracks C–F.

---

If you want, I can draft the initial file skeletons for **ImageArena** and **ResolvePass** (PR 50–51) plus the per-pass stubs for **Post Ao/SSGI/SSR/Bloom** so the agent can paste and fill them in with zero guesswork.


Great—here’s **Phase Three: Full Instructions** as an engineering playbook you (or the automation agent) can execute PR‑by‑PR. It assumes your Phase‑Two state (graph passes for Main/Particles/UI/Present, hazard checks, BgCache/UploadRing/RebuildBus, perf stats) is already on `main`.

---

## 🎯 Phase Three — Objectives & Exit Criteria

**Primary objectives**

1. **True graph‑backed allocations + aliasing.** The framegraph *owns* image creation; passes obtain real `TextureView`s from handles (no direct `attachments.*` reach‑through).
2. **MSAA end‑to‑end.** Sample count is threaded through images and pipelines; Main resolves correctly to HDR color for post.
3. **Post suite fully extracted.** SSR/SSGI/AO/Bloom run as passes; legacy path deleted.
4. **Typed pipelines in passes.** Pass code stops touching raw `wgpu::*` pipelines directly.
5. **Main batched via DrawList.** Batches show up in stats and reduce state churn.
6. **Hardening & tests.** RebuildBus has unit tests; ring/cache tested; surface error handling robust.

**Exit criteria**

* No pass code touches `attachments.scene_view/depth_view` directly—only **graph handles**.
* All post‑FX (AO/SSGI/SSR/Bloom) execute via passes; Present last.
* MSAA sample count consistent across textures & pipelines; resolve path is explicit.
* `render.rs` contains only frame prep + small graph build/execute; legacy draw blocks removed.
* New and existing unit tests pass; `xtask ci` remains green.

---

## 📦 PR Plan (small, verifiable steps)

> Keep diffs behavior‑neutral unless a PR explicitly says it changes behavior. After each PR: run `xtask ci` and update the docs (see **Docs updates** per PR).

### PR 42 — Graph views API + handle plumbing (behavior‑neutral)

**Goal:** Passes use *graph views*, not raw attachments.

**Changes**

* In `renderer/graph.rs`:

  * During `Graph::compile`, compute a **stable index** for every `Handle<Img>` and build a `Vec<ImagePlan>` for the frame (format/size/msaa/usage).
  * Add storage on the compiled graph for **runtime views**: `views: Vec<wgpu::TextureView>`.
  * Add `ExecCtx` methods:

    ```rust
    impl<'a> ExecCtx<'a> {
        pub fn view_color(&self, h: Handle<Img>) -> &wgpu::TextureView { /* map handle → views[idx] */ }
        pub fn view_depth(&self, h: Handle<Img>) -> &wgpu::TextureView { /* same */ }
    }
    ```
  * For now, **bind each handle to the real offscreen attachments** you already have:

    * If `ImageKind::Color`: `views[idx] = attachments.scene_view.clone()`.
    * If `ImageKind::Depth`: `views[idx] = attachments.depth_view.clone()`.

* Replace in all pass exec closures:

  * `&ctx.renderer.attachments.scene_view` → `ctx.view_color(color)`
  * `&ctx.renderer.attachments.depth_view` → `ctx.view_depth(depth)`

**Tests**

* CPU‑only compile tests (ensure pass code compiles with new `view_*` APIs).
* Keep hazard tests as is.

**Validation**

* Visual parity, `xtask ci` green.

**Commit**

```
renderer(PR42): add ExecCtx::view_color/view_depth; map graph handles to real views; refactor passes to use views (behavior-neutral)
```

**Docs**

* `docs/audit/log.md`: add PR 42 entry.
* `docs/audit/next-steps.md`: mark “graph views” done.

---

### PR 43 — Allocation plan & lifetime analysis (behavior‑neutral)

**Goal:** Graph computes **live ranges** per image; prepares an **allocation plan** (no aliasing used yet).

**Changes**

* In `Graph::compile`:

  * Compute for each `Handle<Img>` the first/last pass index where it’s **read or written**.
  * Build an `AllocationPlan { desc: ImageDesc, live: RangeInclusive<usize> }` for each image.
  * On `Graph::execute`, create **real textures** per plan (no aliasing yet); create the views into `self.views`.

* `ImageDesc` must include: `format`, `size(u32x2)`, `msaa`, and **usage** flags derived from IO:

  * Color write → `RENDER_ATTACHMENT`
  * Any read in later pass → add `TEXTURE_BINDING`
  * Depth write → `RENDER_ATTACHMENT`
  * Depth read → add `TEXTURE_BINDING`
  * (Keep `COPY_SRC/COPY_DST` as needed.)

**Tests**

* Unit test for **liveness**: two images with non‑overlapping live ranges; plan records correct intervals.

**Validation**

* Visual parity, `xtask ci` green.

**Commit**

```
renderer(PR43): compute image live ranges and per-frame allocation plan; instantiate textures per plan (aliasing deferred)
```

**Docs**

* Log PR 43 with liveness details.

---

### PR 44 — Safe aliasing (opt-in feature, off by default)

**Goal:** Reuse memory for images whose live ranges **do not overlap**, reducing peak VRAM.

**Changes**

* Add `feature = "graph_aliasing"` (default **off**).
* If enabled, in `Graph::compile`:

  * For each `ImageDesc`, attempt to assign a previously created `Texture` in a pool keyed by descriptor **compatible** fields (same size, format, msaa, usage superset).
  * Ensure **no alias** if `live` ranges overlap.
* Keep a debug log `RA_GRAPH_TRACE=1` that prints assignments:

  ```
  [graph] Img#3 -> Tex#A (alias of Img#1)
  ```

**Tests**

* CPU‑only: test interval packing logic (no wgpu).
* (Optional) device test behind `--features wgpu-tests`: allocate >1 handles with exclusive live ranges and assert pool length reduced.

**Validation**

* Feature off by default → visual parity.
* With feature on: smoke run locally, verify no hazards trip.

**Commit**

```
renderer(PR44): optional aliasing in framegraph allocation plan behind feature flag; interval packing + pool reuse
```

**Docs**

* Document aliasing feature/flag and guardrails in `risk-and-test-plan.md`.

---

### PR 45 — MSAA end‑to‑end (behavior‑neutral toggle)

**Goal:** Thread real sample count; resolve MSAA **Main** to single‑sample HDR color for post.

**Changes**

* Store `attachments.msaa_samples` (already exists or add).

* In `render.rs` (graph path):

  * Use:

    ```rust
    let samples = r.attachments.msaa_samples;
    let hdr_format = wgpu::TextureFormat::Rgba16Float; // if that’s your HDR
    let depth_fmt = wgpu::TextureFormat::Depth32Float;
    let hdr = gb.image(ImageKind::Color { format: hdr_format, size, msaa: 1 });
    let depth = gb.image(ImageKind::Depth { format: depth_fmt, size, msaa: samples });
    let msaa = if samples > 1 { Some(gb.image(ImageKind::Color { format: hdr_format, size, msaa: samples })) } else { None };
    ```

* **MainPass::declare**:

  * If `msaa.is_some()`: begin pass with `view = ctx.view_color(msaa.unwrap())` and set **resolve target** to `ctx.view_color(hdr)`.
  * Else: render directly to `hdr`.

* Post passes **read** and **write** the **single‑sample** `hdr`.

* Ensure all Main pipelines are created with the same `sample_count` (`samples`).

* Add `RA_MSAA` env to override samples (1 or 4) at init (optional).

**Tests**

* Build tests; optional device smoke behind `--features wgpu-tests` with samples=1 & 4.

**Validation**

* Visual parity (samples=1 default).
* With samples=4 locally: no validation errors; Present unchanged.

**Commit**

```
renderer(PR45): thread MSAA sample count; Main resolves to single-sample HDR; post reads/writes HDR; pipelines sample_count updated
```

**Docs**

* `refactor-progress.md`: mark MSAA “wired end‑to‑end”.

---

### PR 46 — Extract post suite (behavior‑neutral)

**Goal:** Move AO/SSGI/SSR/Bloom logic into `pass_*` helpers and exec via graph; delete legacy blocks.

**Changes**

* `passes.rs`: implement `pass_post_ao/ssgi/ssr/bloom` using existing code targeting **hdr** and **depth** views.
* `passes_graph.rs`: add corresponding `declare` methods and stats block.
* `render.rs`: build graph with order:

  ```
  Sky(optional) → Main → Particles → UI → PostAo → Ssgi → Ssr → Bloom → Present
  ```
* Delete legacy post code in monolith after parity check.

**Tests**

* Hazard order test (write after read must fail).
* Visual smoke: parity.

**Validation**

* `xtask ci` green.

**Commit**

```
renderer(PR46): extract post suite into passes; wire graph order; remove legacy post code; hazard-validating IO
```

**Docs**

* Update large-files audit (sizes down), and log PR 46.

---

### PR 47 — Pipelines split + typed access in passes (mechanical)

**Goal:** Moves builders out of `gfx/pipeline.rs`, adds `Pipelines` on `Renderer`, and use `ctx.pipelines()` in passes.

**Changes**

* Create `renderer/pipelines/*` modules and a `Pipelines` struct (as planned).
* `Renderer` gains `pipelines: Pipelines`.
* `ExecCtx::pipelines()` returns `&Pipelines`.
* Replace direct pipeline field reads in pass code with `ctx.pipelines().main/ssr/...`.

**Tests**

* Build only.

**Validation**

* `xtask ci` green.

**Commit**

```
renderer(PR47): split pipeline builders and refactor passes to typed Pipelines via ExecCtx (no behavior change)
```

**Docs**

* Log PR 47; update large-files audit.

---

### PR 48 — Main DrawList batching (behavior‑neutral)

**Goal:** Use `DrawList` inside `pass_main` to batch contiguous identical keys; update `RenderStats.batches`.

**Changes**

* In `pass_main`, build `DrawList` with items in **legacy order**.
* Call `to_batches()`; for each batch:

  * Bind pipeline/material once; issue draw(s) as today (you can keep per‑mesh draws if you can’t sum safely—still reduces binds).
* `RenderStats.batches = batch_count`.

**Tests**

* New unit tests in `draw_list.rs`:

  * `empty_list_has_no_batches`
  * `interleaved_keys_produce_multiple_batches`
* CPU‑only test to ensure deterministic grouping given a fixed input.

**Validation**

* Visual parity.
* Perf HUD: batches ≤ draws.

**Commit**

```
renderer(PR48): adopt DrawList batching in Main; preserve order; update RenderStats.batches; tests for grouping
```

**Docs**

* Log PR 48; note any batch reductions observed.

---

### PR 49 — UploadRing adoption (expand safely)

**Goal:** Convert 2–3 more frequent small writes to ring‑backed copies; ensure one write path per buffer per frame.

**Changes**

* Identify hot buffers written via `Queue::write_buffer` outside passes (e.g., globals, small uniforms).
* Convert to:

  ```rust
  let slice = uploads.allocate(&queue, data, 256);
  encoder.copy_buffer_to_buffer(&slice.buffer, slice.offset, &buf, off, slice.size);
  ```
* Add `RenderStats` bytes staged (delta counter) if helpful.

**Tests**

* `upload.rs`: add `next_frame_resets_cursor` (expose a test‑only getter).

**Validation**

* `xtask ci` green; grep per buffer ensures no mixed path in same frame.

**Commit**

```
renderer(PR49): extend UploadRing coverage; single write path per buffer per frame; ring test for cursor reset
```

**Docs**

* Log PR 49 with the concrete buffers migrated.

---

### PR 50 — RebuildBusCore<T> + unit tests

**Goal:** Test listener order without constructing `Renderer`.

**Changes**

* Extract:

  ```rust
  pub struct RebuildBusCore<T> { listeners: Vec<Box<dyn Fn(&mut T)+Send+Sync>>; }
  impl<T> RebuildBusCore<T> { /* new, register, run_all */ }
  pub type RebuildBus = RebuildBusCore<Renderer>;
  ```
* Tests with `RebuildBusCore<u32>` to verify **order** and **single dispatch** semantics.

**Validation**

* `xtask ci` green.

**Commit**

```
renderer(PR50): extract RebuildBusCore<T>; add tests for listener ordering and single-dispatch semantics
```

**Docs**

* Log PR 50; mark risk/test plan item covered.

---

### PR 51 — Present error handling polish (complete)

**Goal:** Explicit handling of `SurfaceError` variants, plus reconfigure path.

**Changes**

* In `PresentPass` exec closure:

  ```rust
  use wgpu::SurfaceError::*;
  let frame = match ctx.renderer.surface.get_current_texture() {
      Ok(f) => f,
      Err(Outdated | Lost) => { ctx.renderer.reconfigure_surface(); return; }
      Err(Timeout) => { log::warn!("present: timeout"); return; }
      Err(OutOfMemory) => { log::error!("present: OOM"); ctx.renderer.disable_rendering(); return; }
  };
  ```
* Implement `reconfigure_surface()` using current `config` + `resize_impl` (fires bus).
* Implement `disable_rendering()` to set a flag checked at frame start (skip render until manual reset).

**Tests**

* CPU‑only stub tests for the branch; device tests optional behind `wgpu-tests`.

**Validation**

* Manual: resize/minimize/maximize; verify no panics; recover.

**Commit**

```
renderer(PR51): robust Present error handling; surface reconfigure path; optional disable_rendering hook
```

**Docs**

* Log PR 51; update risk plan with recovery procedure.

---

### PR 52 — Remove legacy paths & long‑file cleanup

**Goal:** Delete the legacy rendering blocks and reduce long files further.

**Changes**

* Remove `RA_RENDER_LEGACY` gate and the legacy Main/Present code paths (graph is the default and only path).
* Ensure `gfx/mod.rs` and `render.rs` size drops—keep only frame prep + graph build/exec.
* Trim `renderer/update/legacy.rs` to thin re‑exports only.

**Validation**

* `xtask ci` green; manual runtime smoke.

**Commit**

```
renderer(PR52): remove legacy render paths; shrink long files; keep re-export shims only where still referenced
```

**Docs**

* Update `large-files-audit-v2.md` with new sizes and mark goals met.

---

## 🧪 Tests & Guardrails (Phase Three additions)

* **Graph**

  * Liveness: first/last use per image; non‑overlap assertions.
  * Aliasing: interval packing unit tests; feature‑gated device smoke.
* **MSAA**

  * Compile paths for 1 and 4 samples; optional device smoke test.
* **DrawList**

  * Empty/interleaved/contiguous tests (already have some).
* **UploadRing**

  * `align_up` (done).
  * `next_frame_resets_cursor`.
* **RebuildBus**

  * Core generic tests for order & single‑dispatch.
* **Present**

  * Surface error branch test stubs.

---

## ⚙️ Feature Flags & Toggles

* `graph_aliasing` (default off): enables texture aliasing.
* `RA_MSAA` (env): `1` or `4` samples.
* `RA_GRAPH_TRACE=1`: logs allocation/aliasing decisions.
* `RA_GPU_TIMESTAMPS=1` (optional future): enable GPU timing queries per pass.

---

## 📝 Docs to update every PR

* `docs/audit/log.md`: add **Changes** + **Validation** per PR.
* `docs/audit/large-files-audit-v2.md`: update file sizes & status.
* `docs/audit/refactor-progress.md`: move items from “Not yet complete” → “Done”.
* `docs/audit/next-steps.md`: mark the PR done, promote the next one.
* `docs/audit/risk-and-test-plan.md`: reflect new tests/flags.

---

## 📌 Notes & pitfalls to watch

* **Usage flags:** when an image is written as a color attachment and later sampled, include both `RENDER_ATTACHMENT | TEXTURE_BINDING`.
* **MSAA resolve:** post passes must read **single‑sample** HDR; keep resolve explicit in Main. Avoid sampling MSAA textures.
* **Buffers write policy:** ensure a buffer is updated via **only one method** per frame (ring+copy vs queue write) to avoid hazards.
* **Present BG view:** if your present shader expects the **readable** scene view, ensure the BG binds `scene_read_view` (SRV) rather than `scene_view`, unless they are the same and SRV‑capable.

---

## 📦 Optional Phase‑Three+ enhancements (defer unless you want them now)

* **GPU timestamps** per pass (query sets + resolve buffer + HUD).
* **Pipeline cache** keyed by shader hash, defines, layouts, sample count; eviction policy + tests.
* **Sampler cache** keyed by descriptor; avoid duplicates.
* **VRAM stats in HUD** (sum allocation plan bytes; approximate formats × msaa × size).

---

If you follow this sequence (PR 42 → 52), you’ll exit Phase Three with a fully graph‑owned renderer (allocations, MSAA, post suite, typed pipelines), lean top‑level files, robust tests, and clean parity.
