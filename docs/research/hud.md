Below is a practical “teach‑me + design spec + MVP issue” package for building a world‑class HUD on a Rust / `wgpu` stack **from scratch** while staying faithful to your new architecture (renderer = draw‑only; `client_core` owns input/UI; `server_core` is authoritative, UI is non‑authoritative).

---

## 1) Best practices in HUD design (engine‑agnostic)

**Goals**

* **Readability first**: consistent type scale, spacing, and contrast; prioritize at‑a‑glance info (health, ammo, objectives).
* **Information hierarchy**: a few always‑visible primaries, secondary info on demand, tertiary in panels.
* **Stability**: avoid layout “jumping”; reserve space for values that change.
* **Glanceability** under motion: limit high‑frequency flicker; smooth numbers with simple low‑pass filters where appropriate.
* **Latency**: HUD should render **every frame** but compute **only when dirty** (invalidation model).
* **Resolution independence**: device‑independent units + DPI scaling; guard for ultrawide and 4K.
* **Accessibility**: color‑blind safe palette; high‑contrast theme; scalable fonts; input remapping and focus nav.
* **Performance budgets** (per frame): UI CPU ≤ 1 ms, GPU ≤ 0.5 ms on mid hardware; ≤ 5 draw calls for static HUD, ≤ 20 for complex panels.

**Interaction model**

* **Direct** (hover/click, controller focus) with predictable focus & tab order.
* **Non‑blocking**: HUD must never block camera or game input unless an overlay modal is active.
* **Coexistence**: overlay HUD + in‑world widgets (diegetic) + debug/dev tools, each on its own layer.

---

## 2) What’s worth learning from Unity / Unreal / custom engines

**Unity (UGUI & UI Toolkit)**

* **Batching is king**: too many Canvases = expensive rebuilds; prefer a few large canvases with **invalidation**.
* **Anchors & scaling**: anchor to screen edges, not absolute pixels; use a **Scale With Screen Size** policy.
* **Masking/Clipping**: Rect masks are cheaper than stencil when possible; invalidate only affected subtrees.

**Unreal (Slate/UMG)**

* **Invalidation Panels / Retainer Boxes**: cache pre‑composited widgets; only repaint when dirty.
* **Draw‑list architecture**: build a lightweight **element list** per frame from a retained widget tree, then batch.
* **Input routing**: robust focus system + controller navigation; emulate this.

**Custom engines (e.g., ImGui‑style and bespoke retained UIs)**

* **Immediate vs. retained**: immediate is great for tooling; retained is better for complex, animated game HUDs.
* **Command buffers**: generate a simple, flat **UI draw command buffer** for the GPU after layout & hit‑testing.
* **Virtualization**: big tables/lists must virtualize; only layout/draw visible rows.

**Takeaways for us**

* Use a **retained tree** with **invalidation** + a **recorded command list** each frame.
* Keep **one or very few UI passes**; batch by atlas/material; scissor‑based clipping.
* Separate **layout/data** from **render**, and isolate **input routing**.

---

## 3) The “perfect” Rust/`wgpu` HUD stack (build‑our‑own spec)

### 3.1 High‑level architecture (new crates)

```
crates/
  ui_core/              # Retained tree, layout, style, input, focus, accessibility
  ui_renderer_wgpu/     # GPU-facing: atlases, pipelines, batching, clip/scissor, SDF text
  ui_input/             # Winit→UI event translation, gamepad mapping
  ui_widgets/           # Basic widgets: text, icon, image, bar, button, checkbox, list, table
  ux_hud/               # Game-specific HUD composition & bindings
```

* **Ownership**: `client_core` hosts UI state & drives updates; `render_wgpu` consumes the **UI command buffer** to draw.
* **Server** never sees or needs UI.

### 3.2 Data flow & frame phases

1. **Input**: `platform_winit` events → `ui_input` → `ui_core::Dispatcher` (focus, routing, gestures).
2. **Update**: `ui_core` runs systems (animations, timers) and applies **dirty flags**.
3. **Layout**: Only dirty subtrees recompute layout (Flex + Absolute + Grid lite).
4. **Build Command Buffer**: `ui_core` emits `UiCmd[]`: quads, rounded rects, nine‑slice, images, text runs, clip rect pushes/pops.
5. **Render**: `render_wgpu` takes `UiCmd[]`, performs **binning + batching** by material/atlas/clip, issues draws in a final UI pass.

### 3.3 Rendering model (wgpu)

* **One UI render pass** after 3D scene, with premultiplied alpha:

  * `blend: (ONE, ONE_MINUS_SRC_ALPHA)`
  * no depth write; optional depth test off.
* **Pipelines**

  1. **UIShape**: rounded rects, lines, gradients (simple signed‑distance in fragment).
  2. **UIText**: **MSDF/SDF** glyphs in a texture atlas (MSDF preferred for sharp scaling).
  3. **UIImage**: UI textures & icons; nine‑slice support.
* **Buffers**

  * **Persistent, ring‑buffered** vertex/index buffers; dynamic `StorageBuffer` for per‑instance data.
  * Push constants (if supported) for per‑draw small params; otherwise dynamic UBO offsets.
* **Clipping**: hierarchical via **scissor** rects; optional stencil for complex shapes (MVP: scissor only).
* **Atlases**

  * **Glyph atlas** (MSDF) + **UI texture atlas** for icons/images; LRU eviction with metrics.
  * On atlas miss, enqueue upload; draw fallback (blank or placeholder) this frame; render correct next frame.

### 3.4 Text & localization

* **Shaping**: wrapper over a shaping lib (e.g., rustybuzz) behind our `ui_core` trait (so we can replace later).
* **Line breaking**: Unicode line break algo; hyphenation optional later.
* **MSDF baking**: offline at build or first use (developer mode). Store `.msdf` or distance field channel textures.
* **Fallback fonts & bidi**: minimal MVP supports Latin; design API for fallback stacks so extending is straightforward.
* **Accessibility scaling**: style tokens (see 3.6) ensure type scale changes adjust globally.

### 3.5 Layout engine (MVP)

* **Flex** (row/column, align, justify, wrap), **Absolute** (for anchors/overlays), **Stack** (z‑order).
* Units: **dp** (device‑independent pixels), `%`, `auto`. DPI scale from `winit`.
* **Dirty propagation**: style/content/geometry changes mark nodes dirty up the chain.
* Perf: O(visible_nodes) per invalidation wave.

### 3.6 Style system & theming

* **Design tokens** (struct, not CSS): `Color`, `Spacing`, `Radius`, `Shadow`, `TypeScale`.
* Theme switch (light, dark, high‑contrast) via swapping token sets.
* **No CSS**—just Rust structs; serde for hot‑reload in dev.

### 3.7 Input, focus, IME

* Central **focus manager** (tab order, gamepad nav graph); mouse capture; drag thresholds.
* IME composition events routed to focused editable widget (text fields for debug console, code panes later).
* Controller nav: D‑pad/left stick moves between focusable widgets using geometry heuristics.

### 3.8 Data binding to ECS

* **One‑way** binding from ECS queries to widget props (health, ammo, objectives).
* **No UI → server mutation** except via explicit client actions (commands) that are validated server‑side.
* **Diff‑apply**: when ECS data changes, only mark dependent widgets dirty.

### 3.9 Observability

* Metrics: `ui.draw_calls`, `ui.vertices`, `ui.tris`, `ui.cmd_count`, `ui.atlas_evictions`, `ui.glyph_misses`, `ui.ms.layout`, `ui.ms.encode`, `ui.ms.render`.
* Debug overlay widget shows these live.

### 3.10 Resilience & determinism

* UI is **client‑only**; zero effect on authoritative sim.
* **No panics** on asset/atlas misses; degrade gracefully.
* All caches bounded; LRU with metrics; never unbounded growth.

---

## 4) Example HUD composition (what we build in `ux_hud`)

* **Top bar**: health/armor, ammo, compass.
* **Objective panel**: current objective + sub‑goals; expandable.
* **Notifications toasts**: bottom‑right; queued, animated in/out.
* **Debug panel** (toggle): FPS, frame timings, net stats, budgets.
* **Full‑screen “data mode”**: table + plot (virtualized list; render only visible rows).

---

## 5) GitHub issue — **HUD v0 (MVP) stack on Rust/wgpu**

**Title:** HUD v0 — Build minimal retained UI stack + wgpu renderer and wire into new architecture
**Labels:** `area:render` `area:arch` `area:ecs` `area:observability` `effort:M` `milestone:Now`
**Summary:**
Implement a minimal but scalable HUD stack: retained UI tree, invalidation‑based layout, command buffer emission, and a single `wgpu` UI pass. Provide core widgets and compose a baseline game HUD in `ux_hud`. Keep UI client‑only and deterministic‑safe (non‑authoritative).

### Deliverables

1. **New crates**

   * `ui_core/`

     * `Node`, `WidgetId`, `Widget` trait (`measure()`, `layout()`, `build_cmds()`).
     * `Layout` (Flex/Absolute/Stack), `Style` tokens, `Theme`.
     * `Dispatcher` for events, focus, IME hooks.
     * `Command` enum for draw: `Quad`, `RoundedRect`, `NineSlice`, `Image`, `TextRun`, `ClipPush/Pop`.
     * `GlyphCache` trait (backed by `ui_renderer_wgpu` uploads).
   * `ui_renderer_wgpu/`

     * UI pipelines: Shape/Text/Image; premultiplied alpha; one render pass.
     * Atlases: glyph (MSDF) + UI textures; LRU; metrics.
     * Encoder: take `Command[]` → batched draws (instanced quads; scissor per clip).
   * `ui_input/`

     * `winit` translation to `UiEvent` (mouse, keyboard, controller stubs, IME).
   * `ui_widgets/`

     * `Text`, `Icon`, `Image`, `Button`, `Bar`, `StackPanel`, `FlexRow`, `FlexCol`, `ScrollView` (axis‑locked).
   * `ux_hud/`

     * Compose `TopBar`, `ObjectivePanel`, `Toasts`, `DebugPanel`.

2. **Integration points**

   * `client_core`

     * Owns `UiWorld` (tree + state).
     * System order: input → update → layout → build commands → submit to renderer.
   * `render_wgpu`

     * New `render_ui(&UiCmdBuffer)` pass after 3D scene.
     * Double‑buffered command buffers to avoid lifetime hazards.

3. **Config**

   * DP scaling factor from `winit` DPI; round to 0.25 steps.
   * Theme files under `assets/ui/themes/*.ron` (serde); hot‑reload in dev builds.

4. **Observability**

   * Add metrics listed in §3.9 and a tiny on‑screen stats widget.

### Non‑goals (v0)

* Complex text shaping/bidi (Latin only for MVP).
* Stencil clipping / vector paths beyond rect/rounded‑rect.
* Complex tables (just a virtualized list).

### Acceptance Criteria

* **Performance**: Static HUD scene ≤ 0.5 ms GPU, ≤ 1 ms CPU on a mid‑tier dev box; ≤ 10 draw calls.
* **Scaling**: UI scales correctly from 720p → 4K with DP scaling; anchors maintain layout.
* **Focus & input**: Mouse hover/click and basic keyboard focus traversal works; IME stub compiled.
* **Stability**: No panics on missing glyphs/textures; graceful fallback and metric increments.
* **Architecture compliance**: No `server_core` deps; UI updates in `client_core`; `render_wgpu` is draw‑only.

### Tasks

* [ ] **Scaffold crates** (`ui_core`, `ui_renderer_wgpu`, `ui_input`, `ui_widgets`, wire `ux_hud`).
* [ ] **Command enum & builder** in `ui_core`.
* [ ] **Layout (Flex/Absolute/Stack)** with invalidation + DP units.
* [ ] **Theme & tokens** (light/dark/high‑contrast stubs).
* [ ] **Glyph atlas** (MSDF preferred; fallback bitmap), basic Latin text run.
* [ ] **Pipelines & batching** in `ui_renderer_wgpu` (Shape/Text/Image).
* [ ] **Clip via scissor**; nested clip push/pop respected.
* [ ] **UI pass** integration in `render_wgpu`.
* [ ] **Input routing** from `winit` → `ui_input` → `ui_core::Dispatcher`.
* [ ] **Widgets**: Text, Button, Bar, Flex containers, ScrollView.
* [ ] **HUD composition**: Top bar, objective panel, toasts, debug overlay.
* [ ] **Metrics** & on‑screen stats widget.
* [ ] **Docs**: `docs/ui/README.md` with architecture diagram & usage sample.
* [ ] **CI**: compile checks; forbid `unwrap/expect` in UI crates (non‑test) and enforce `clippy`.

### Code Sketches (orientation)

**UI command buffer (core)**

```rust
pub enum UiCmd {
    ClipPush(RectPx),
    ClipPop,
    Quad { rect: RectPx, color: Color },
    RoundedRect { rect: RectPx, radius: f32, color: Color },
    NineSlice { rect: RectPx, atlas_id: AtlasId, uv: UvRect },
    Image { rect: RectPx, atlas_id: AtlasId, uv: UvRect, tint: Color },
    TextRun { baseline: PointPx, run: TextRunId, color: Color },
}
```

**Widget trait (core)**

```rust
pub trait Widget {
    fn update(&mut self, cx: &mut Cx);          // animations, timers
    fn measure(&mut self, cx: &mut Cx, bc: BoxConstraints) -> SizePx;
    fn layout(&mut self, cx: &mut Cx, origin: PointPx) -> RectPx;
    fn build_cmds(&self, out: &mut UiCmdBuffer);
    fn hit_test(&self, p: PointPx) -> bool { /* default rect hit */ }
}
```

**Renderer entry (wgpu)**

```rust
pub fn render_ui(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &wgpu::TextureView,
    frame_size_px: [u32;2],
    ui_cmds: &UiCmdBuffer,
    caches: &mut UiCaches,
    enc: &mut wgpu::CommandEncoder)
{
    // 1) bin & batch by material + scissor
    // 2) upload changed glyphs/icons to atlases
    // 3) record single render pass with instanced draws
}
```

**Client update order**

```rust
fn client_tick_ui() {
    ui_input::pump_winit_events(..., &mut ui_world.dispatcher);
    ui_core::update(&mut ui_world);
    ui_core::layout(&mut ui_world);
    ui_core::build_cmds(&ui_world, &mut ui_cmd_buffer);
    render_submit_ui(ui_cmd_buffer.swap_take());
}
```

### Risk & Mitigation

* **Text complexity**: start Latin, design for shaping fallback later.
* **Atlas thrash**: cap glyphs per frame; LRU eviction metrics; pin common glyph pages.
* **Over‑abstraction**: keep APIs small, evolve with real HUD use.

---

## 6) How to approach “full‑screen HUDs with lots of data and code”

* **Virtualized lists/tables**: only layout/draw visible rows; estimate row height; refine lazily.
* **Incremental updates**: diff binding changes; partial invalidation.
* **Command buffer size caps**: per‑frame upper bound; if exceeded, degrade (split across frames) with a visible “busy” indicator for debug panels.
* **Text panes/code views**: monospaced font atlas with per‑line chunking; partial redraw on edits; IME & selection model designed early.
* **GPU plots**: simple vertex buffers for line/area plots; decimate data on CPU to reduce vertices; reuse buffers.

---

## 7) Roadmap after MVP

* **Shaping/bidi/localization** (rustybuzz wrapper behind trait).
* **Stencil clipping & vector paths** (rounded rect with variable radii, capsules, arcs).
* **Animation curves & timeline** (spring/bezier).
* **World‑space widgets** (billboard UI, depth‑sorted pass).
* **Retained caching (retainer panels)** for complex static subtrees.
* **Theming editor** & live reload panel.

---

This plan gives you a **lean, owned UI stack** tuned for Rust/`wgpu`, following the strongest patterns from Unity/Unreal without their bloat. It keeps your sim deterministic and server‑clean, supports small HUDs and **full‑screen data‑heavy overlays**, and grows naturally toward advanced text/layout and world‑space UI. The **HUD v0 issue** above is ready to paste into GitHub and run with.
