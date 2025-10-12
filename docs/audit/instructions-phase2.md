Below is a concrete **phase‑two** playbook you can hand to a coding agent. It turns the scaffolding from phase one into a real, validated framegraph with extracted passes, draw‑list preparation, cache layers, and centralized resize/rebuild—**still behavior‑equivalent** (no intentional visual or gameplay changes).

---

## Phase‑two ground rules

* **Behavior parity:** The final frame output should match phase‑one output. If a visual diff appears, treat it as a bug unless an intentional fix is explicitly documented.
* **Tight PRs:** Keep changes small and thematic. One new capability per PR (pass extraction, draw‑builder, cache layer, etc.).
* **Type safety for invariants:** Prefer types/traits to encode rules (e.g., pass IO), backed by runtime asserts in debug.
* **Tests first:** Each capability lands with CPU‑only tests; GPU checks are optional/feature‑gated.
* **Compatibility layer:** Keep re‑exports/aliases while migrating call sites; deprecate, don’t delete, until the end of phase two.

---

## Phase‑two objectives (what “done” looks like)

* A **real framegraph** (`renderer::graph`) compiles declared pass IO to an execution plan and **validates hazards** (no write–after–write or read–write of the same virtual resource).
* **Passes extracted** from the monolithic render path: `Sky`, `Main`, `Particles`, `Post::{AO, SSGI, SSR, Bloom}`, `UI`, `Present`.
* **Draw‑list preparation** runs **before** command encoding; draws are grouped by pipeline/material to minimize state churn.
* A **BindGroup cache** (typed keys) prevents per‑frame rebuild churn.
* A **uniform/storage upload ring** replaces ad‑hoc buffer updates.
* **Resize/reconfigure** is centralized (attachments + pipeline rebuild subscription).
* UI is a clean pass that consumes prepared buffers—no render‑path conditionals.
* Per‑pass **timings & counters** exist (CPU timers; GPU timestamps when available).
* CI: new unit tests for graph invariants & draw grouping; optional headless GPU tests behind a feature.

---

## PR plan & detailed instructions

> Use these PRs in order. Each PR includes scope, steps, and acceptance criteria.

### PR 12 — Framegraph core (builder, resources, validation)

**Scope**

* Replace the “forwarder” graph with a minimal, real graph compiler and hazard checker. Keep a single “Monolith” pass that still calls old `record_frame()` so nothing changes visually yet.

**Steps**

1. In `renderer/graph.rs`, define core types:

   ```rust
   pub type Size2D = glam::UVec2;

   pub enum ImageKind { Color { format: wgpu::TextureFormat, size: Size2D, msaa: u32 },
                        Depth { format: wgpu::TextureFormat, size: Size2D, msaa: u32 } }

   pub struct Handle<T>(u32, std::marker::PhantomData<T>); // ResourceId newtype

   pub enum Access { Read, Write }
   pub struct Img; // phantom tag for images

   pub struct GraphBuilder {
       images: Vec<ImageKind>,
       passes: Vec<PassDecl>,
   }
   pub struct PassDecl {
       name: &'static str,
       reads: Vec<Handle<Img>>,
       writes: Vec<Handle<Img>>,
       exec: Box<dyn Fn(&mut ExecCtx) + Send + Sync>,
   }

   pub struct ExecCtx<'a> {
       pub gpu: &'a GpuCtx,
       pub surface: &'a SurfaceCtx,
       pub attachments: &'a mut Attachments,
       pub pipelines: &'a Pipelines,
       pub bindgroups: &'a mut BgCache,
       pub uploads: &'a mut UploadRing,
       pub encoder: &'a mut wgpu::CommandEncoder,
   }
   ```
2. Add APIs:

   ```rust
   impl GraphBuilder {
       pub fn image(&mut self, kind: ImageKind) -> Handle<Img> { /* push; return handle */ }
       pub fn pass<F>(&mut self, name: &'static str, f: F) -> &mut PassDecl
         where F: Fn(&mut ExecCtx) + Send + Sync + 'static { /* ... */ }
   }
   impl PassDecl {
       pub fn reads(&mut self, h: Handle<Img>) -> &mut Self { /* ... */ }
       pub fn writes(&mut self, h: Handle<Img>) -> &mut Self { /* ... */ }
   }
   ```
3. Implement `Graph::compile(&GraphBuilder)`:

   * Topologically order passes by declared resource deps.
   * Validate hazards (same `Handle<Img>` cannot be both read & write in the same frame except with explicit subpass marker—**not implemented yet**, so error).
   * **Runtime asserts in debug**; in release, log and continue (to avoid panics in prod).
4. Execution:

   * For now, allocate/alias to `attachments` (reuse existing views) and run the single Monolith pass that calls `render::record_frame`.
5. Tests (CPU‑only):

   * Unit test that a contrived R/W conflict is detected.
   * Unit test that order is stable for independent passes.

**Acceptance**

* All tests pass; game renders via Monolith pass. Hazard checks active in debug.

---

### PR 13 — Extract `Present` and `Sky` passes

**Scope**

* Split out the most independent passes with minimal dependencies.

**Steps**

1. Create `renderer/passes/present.rs` and `renderer/passes/sky.rs`:

   * Each implements:

     ```rust
     pub struct PresentPass;
     impl PresentPass {
         pub fn declare(builder: &mut GraphBuilder, backbuffer: Handle<Img>) -> PassId { /* reads backbuffer; exec: submit present */ }
     }
     ```

     ```rust
     pub struct SkyPass { /* refs to pipelines/layouts as handles only */ }
     impl SkyPass {
         pub fn declare(builder: &mut GraphBuilder, color: Handle<Img>, depth: Handle<Img>) -> PassId {
             // reads depth? usually writes color/depth
         }
     }
     ```
2. In `FrameGraph::run`, build images:

   * `backbuffer` (surface view), `hdr_color`, `depth`.
3. Call `SkyPass::declare(...)` then `PresentPass::declare(...)`.
4. Move the sky portion of `record_frame` into the Sky pass **verbatim** (binding & draws). Keep Monolith for the rest.

**Acceptance**

* Visuals unchanged; `Sky` renders; `Present` presents. Hazard validation passes.

---

### PR 14 — Extract `Main` pass (terrain + instanced)

**Scope**

* Move terrain & instanced draws into `passes/main.rs`. **No draw grouping yet**, just a verbatim move.

**Steps**

1. Create `renderer/passes/main.rs`:

   * `MainPass::declare(builder, color, depth)` **writes** color & depth.
   * Carry required pipelines via `Pipelines` reference provided through `ExecCtx`.
2. Split the corresponding code from `record_frame`. Ensure all bind group/layout refs come via `pipelines/common.rs` typed wrappers.

**Acceptance**

* Scene still renders. Pass order: Sky → Main → Present.

---

### PR 15 — Extract `Particles` and `UI` passes

**Scope**

* Move particle draws and HUD draw to dedicated passes.

**Steps**

1. `passes/particles.rs`: writes color; typically reads depth (if doing soft particles).
2. `passes/ui.rs`: draws on top of `color` only; consumes prepared UI VBs/IBs from `ui` module (phase‑one split).
3. Update graph order: Sky → Main → Particles → UI → Present.

**Acceptance**

* Particles and HUD render correctly; ordering preserved by declared IO.

---

### PR 16 — Extract `Post` suite as subpasses

**Scope**

* AO, SSGI, SSR, Bloom as individual passes writing transient images; final pass writes back to `hdr_color` or a distinct `post_color` that feeds Present.

**Steps**

1. Create `passes/post/{ao,ssgi,ssr,bloom,composite}.rs`.
2. For each, declare:

   * Inputs: `hdr_color`, `depth` (and history if used later).
   * Outputs: `ao_tex`, `ssgi_tex`, `ssr_tex`, `bloom_tex` (new images via `GraphBuilder::image`); `composite` writes to `hdr_color` or a `post_color`.
3. Move pipeline/bind‑group setup from phase‑one pipelines into these passes.
4. Keep toggles in `renderer::config` and branch **at graph build time** (skip passes by not declaring them).

**Acceptance**

* Visuals unchanged with features toggled on/off.
* Graph can omit passes based on config without code churn.

---

### PR 17 — Draw‑list preparation & grouping (Main pass)

**Scope**

* Precompute draws outside command encoding; sort & group by pipeline/material to reduce state changes.

**Steps**

1. In `renderer/drawlists.rs`, introduce:

   ```rust
   pub struct DrawCallKey { pipeline: PipelineKind, material_id: u64, mesh_id: u64 }
   pub struct DrawItem { key: DrawCallKey, range: std::ops::Range<u32>, instances: u32, first_instance: u32 }
   pub struct DrawBatch { key: DrawCallKey, items: SmallVec<[DrawItem; 8]> }
   pub struct DrawList { batches: Vec<DrawBatch> }
   pub fn build_draw_list(scene: &SceneBuffers /* or your scene input */) -> DrawList { /* pure, deterministic */ }
   ```
2. Sorting/grouping algorithm:

   * Sort by `(pipeline, material, mesh)`.
   * Merge consecutive items with identical key by extending instance runs where legal.
3. Update `MainPass` to take a `&DrawList` and emit draws by batch.
4. Unit tests (CPU‑only):

   * Deterministic sort with a seeded input.
   * Batch merge invariants (never merge across different keys; vertex ranges preserved).

**Acceptance**

* No visual diff; fewer pipeline/material changes (observe in logs/counters).

---

### PR 18 — Bind‑group cache (typed keys)

**Scope**

* Avoid per‑frame `wgpu::BindGroup` churn; introduce an LRU keyed by typed descriptors.

**Steps**

1. `renderer/bindgroups.rs`:

   ```rust
   #[derive(Hash, Eq, PartialEq)]
   pub struct BgKey<T> { pub layout: Bgl<T>, pub ids: SmallVec<[u64; 4]> } // ids for textures/samplers/buffers
   pub struct BgCache { map: lru::LruCache<u64, wgpu::BindGroup>, hits: u64, misses: u64 }
   impl BgCache { pub fn get_or_create<T>(&mut self, key: BgKey<T>, make: impl FnOnce() -> wgpu::BindGroup) -> &wgpu::BindGroup { /* ... */ } }
   ```
2. Replace ad‑hoc bind group creation sites in passes with `BgCache::get_or_create`.
3. Expose counters via a debug UI panel (optional).

**Acceptance**

* Perf counters show cache hits; correctness unchanged.

---

### PR 19 — Upload ring for uniforms/storage buffers

**Scope**

* Centralize small buffer updates with a frame‑lagged ring to reduce map/write overhead.

**Steps**

1. `renderer/upload.rs`:

   ```rust
   pub struct UploadRing {
       frames: [wgpu::Buffer; N], // e.g., 3
       frame_ix: usize,
       cursor: u64,
       size: u64,
   }
   pub struct UploadSlice { pub buffer: wgpu::Buffer, pub offset: u64, pub size: u64 }
   impl UploadRing {
       pub fn next_frame(&mut self) { /* advance frame_ix, reset cursor */ }
       pub fn allocate(&mut self, size: u64, align: u64, queue: &wgpu::Queue, data: &[u8]) -> UploadSlice { /* ... */ }
   }
   ```
2. Change uniform/storage updates in passes to allocate slices from the ring and bind via dynamic offsets or separate BG entries as appropriate.
3. Add a unit test: allocation alignment and wraparound behavior (CPU‑only).

**Acceptance**

* No validation errors; frame pacing stable.

---

### PR 20 — Centralized resize + pipeline rebuild subscriptions

**Scope**

* Stop sprinkling size checks in passes; do it once at the start of a frame.

**Steps**

1. Extend `Attachments::recreate_for_size` to **emit an event** with the new formats/sizes.
2. Add a `PipelineRebuilder` registry:

   ```rust
   pub trait RebuildOnResize { fn rebuild(&mut self, gpu: &GpuCtx, att: &Attachments, cfg: &Config); }
   pub struct PipelineBus { subs: Vec<Box<dyn RebuildOnResize>> }
   ```
3. Each pass registers its builder or pipelines with the bus; on resize, iterate subscribers.
4. Remove inline rebuild code from passes.

**Acceptance**

* Resizing windows rebuilds once, not per pass; no flicker.

---

### PR 21 — UI pass finalization & toggles out of draw path

**Scope**

* Ensure **all** UI branching lives in `ui` module; `passes/ui` just draws prepared buffers.

**Steps**

1. Move any remaining toggles/strings/formatting out of the pass and into `ui::{perf,help,hotbar}` creation.
2. Add a CPU‑only test that building HUD with specific flags yields deterministic vertex counts.

**Acceptance**

* UI unchanged; pass is branch‑free.

---

### PR 22 — Remove deprecated shims, update imports, docs

**Scope**

* Clean out re‑exports introduced in phase one that are now unused. Keep only compatibility aliases used by external crates (if any).

**Steps**

1. `rg` for deprecation warnings; remove internal uses; keep external ones.
2. Update inline rustdoc in `renderer::graph` and `passes/*` with brief IO docs.

**Acceptance**

* Clippy clean; docs build; no downstream breakage.

---

## Example code skeletons (copy/adapt)

**Pass declaration pattern**

```rust
pub trait Pass {
    fn declare(&self, gb: &mut GraphBuilder, io: &mut Io) -> PassId;
    fn record(&self, ctx: &mut ExecCtx, io: &Io);
}
pub struct Io {
    pub color: Handle<Img>,
    pub depth: Handle<Img>,
    // attach more for post passes as needed
}
```

**Sky pass sketch**

```rust
pub struct SkyPass;
impl SkyPass {
    pub fn declare(gb: &mut GraphBuilder, color: Handle<Img>, depth: Handle<Img>) -> PassId {
        gb.pass("Sky", move |ctx| {
            // begin render pass on color/depth, bind sky pipeline, issue draws
        })
        .writes(color)
        .writes(depth);
        // return some id if needed
        PassId::Sky
    }
}
```

**Draw‑list grouping rule (documented & tested)**

* Sort key = `(PipelineKind as u8, material_id, mesh_id)`.
* Never merge across index/vertex range boundaries.
* Instance runs merged if contiguous and `first_instance + instances` aligns.

---

## Tests to add in phase two (CPU‑only unless noted)

1. **Graph hazards:** Declaring a pass that writes `hdr_color` followed by a pass that also writes `hdr_color` without an intermediate read should fail in debug.
2. **Graph ordering:** Independent passes preserve insertion order (stable topo sort).
3. **Draw grouping:** Given a mixed sequence, the grouped `DrawList` has fewer batches and retains per‑item ranges.
4. **Upload ring:** Alignment and wraparound are correct; offsets are multiples of `wgpu::Limits::min_uniform_buffer_offset_alignment`.
5. **BG cache:** Same key returns same BG; different sampler or texture ID yields cache miss.
6. **UI:** Vertex count golden numbers for a small HUD config.
7. **(Optional, feature `gpu_tests`)**: Headless wgpu renders a tiny frame (no surface) and verifies a trivial clear color or a checksum of a compute buffer.

---

## Observability

* Add `RenderStats` struct per pass:

  ```rust
  pub struct RenderStats { pub batches: u32, pub draws: u32, pub bg_hits: u32, pub bg_misses: u32, pub cpu_ms: f32, pub gpu_ms: Option<f32> }
  ```
* Record CPU time with `Instant`; GPU timestamps behind a `#[cfg(feature="gpu_timestamps")]` guard.
* Surface an overlay in `ui::perf` showing per‑pass timings and cache hit rate.

---

## Risk controls & rollback

* Keep an environment toggle `RENDERER_USE_FRAMEGRAPH=false` during early PRs (graph builds both ways but only executes the chosen path). Flip default to `true` at PR 22.
* If any pass extraction introduces a regression, temporarily route that pass back through Monolith while you bisect.
* Ensure every resize‑sensitive pass is subscribed to the `PipelineBus` before removing legacy rebuild code.

---

## Deliverables checklist (phase two)

* [ ] `GraphBuilder` + compile/validate + execution with virtual images.
* [ ] Passes extracted: Sky, Main, Particles, Post (AO, SSGI, SSR, Bloom), UI, Present.
* [ ] `DrawList` builder integrated with `Main` pass; tests for grouping.
* [ ] `BgCache` integrated; counters exposed.
* [ ] `UploadRing` integrated; alignment test.
* [ ] Centralized resize/rebuild via `PipelineBus`.
* [ ] UI pass branch‑free; UI logic stays in `ui::*`.
* [ ] Optional GPU timestamps; perf overlay wired.
* [ ] Deprecated shims removed; docs updated; clippy/fmt clean.

---

### Notes for the agent

* When migrating code into passes, **copy first**, then replace the call site. Avoid interleaving refactors with logic cleanups.
* Keep **public fields private** behind sub‑structs wherever you touch code; expose read‑only accessors as needed.
* If a pass needs temporary scratch images, **declare them as graph resources**; never smuggle `wgpu::Texture` through side channels.

This phase leaves you with a validated, modular renderer that’s easy to extend in phase three (e.g., history buffers, async uploads, or true transient resource aliasing) without destabilizing the game loop.
