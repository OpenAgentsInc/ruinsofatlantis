# Large Files Audit — V2 (Post Phase‑Two)

Scope and method
- Re-ran a workspace scan for files ≥1000 LOC (source only; vendor/content excluded where sensible).
- Cross-referenced previous recommendations in `docs/audit/large-files-audit.md` to mark progress and gaps.

Current ≥1000‑line sources (top offenders)
- crates/render_wgpu/src/gfx/mod.rs (~4918)
- crates/render_wgpu/src/gfx/renderer/update/legacy.rs (~3394)
- crates/render_wgpu/src/gfx/renderer/init.rs (~2391)
- crates/render_wgpu/src/gfx/renderer/render.rs (~2280)
- crates/render_wgpu/src/gfx/ui/legacy.rs (~2148)
- tools/model-viewer/src/main.rs (~2096)
- crates/platform_winit/src/lib.rs (~1785)
- crates/server_core/src/ecs/schedule.rs (~1555)
- crates/server_core/src/lib.rs (~1389)
- crates/render_wgpu/src/gfx/pipeline.rs (~1302)

What improved vs V1
- Passes & graph
  - Present now owns acquire/present as a pass; offscreen → swapchain composition unified.
  - Particles + UI execute via pass closures; Pass IO is modeled and validated (intra‑pass RW + cross‑pass monotonic rule).
  - Main executes via a pass helper; stats collected; DrawList scaffolding in place.
- Infra scaffolds
  - BgCache, UploadRing, RebuildBus landed with initial adoption areas and tests.
  - ExecCtx accessor pattern introduced (device/queue/views), enabling pass extraction with fewer borrows.
  - Vox demo gated and moved under a demo path.
- Docs/tests
  - Hazard tests added; BgCache correctness tests; perf overlay wired to stats.

What still needs breaking up

1) gfx/mod.rs (~4918)
- Role: renderer god‑module; still hosts device/surface, attachments, draw flows, overlays, and legacy paths.
- Gaps
  - Mixed responsibilities (present/blit, bloom, SSR/SSGI, overlays, debug flows) make it hard to reason about ordering or rebuild/resize logic.
- Actions
  - Finish moves into `renderer/passes.rs` (already houses main/post helpers) and `renderer/passes_graph.rs` for declarations.
  - Migrate post overlays (SSR/SSGI/Bloom/AO) into pass helpers (read hdr_color, write hdr_color; Present reads only).
  - Delete any now‑redundant draw code after pass adoption to prevent double work.

2) renderer/update/legacy.rs (~3394)
- Role: large update glue (projectiles, zone/session ghosts, math helpers, destructibles demo).
- Actions
  - Split by concern under `renderer/update/` (already partially done): `projectiles.rs`, `builder.rs`, `math.rs`, `destructibles_demo.rs` (gated).
  - Keep only a thin `legacy.rs` re‑export during transition; shrink steadily as call sites move.

3) renderer/init.rs (~2391)
- Role: device/surface/config/pipelines attachment creation.
- Actions
  - Preserve as initialization home, but move per‑pipeline builders out to `renderer/pipelines/*` and hold typed wrappers.
  - Centralize sampler creation (already present as a type; ensure all consumers import it from a single place).

4) renderer/render.rs (~2280)
- Role: high‑level render loop with legacy toggles.
- Actions
  - Continue shrinking by delegating to passes/graph. Keep only: frame state prep (globals upload), graph build, encoder lifecycle, stats clear.

5) ui/legacy.rs (~2148)
- Role: monolithic HUD/UI draw.
- Actions
  - Keep GPU path here, but move HUD logic/state to `ux_hud` and keep legacy as a thin draw over a flattened HUD model.
  - Split perf/help/hotbar overlays into files in `gfx/ui/` (already scaffolded) and call from the legacy facade.

6) tools/model-viewer/main.rs (~2096)
- Role: standalone viewer with CLI, loader, render, UI in one file.
- Actions
  - Move to `src/{cli,app,viewer,panels,loader,utils}.rs`; keep `main.rs` minimal. This is already scaffolded—finish the move.

7) platform_winit/lib.rs (~1785)
- Role: event loop, input mapping, zone picker, overlays.
- Actions
  - Move code into `app.rs`, `input.rs`, `picker.rs`, `builder_overlay.rs`, `replication.rs`, `telemetry.rs`; lib.rs re-exports modules and keeps the `run()` entry.

8) server_core/ecs/schedule.rs & server_core/lib.rs (~1555 / ~1389)
- Role: schedule builder and crate root.
- Actions
  - Split schedule across stages (`stage_input`, `stage_ai`, `stage_move`, `stage_combat`, `stage_cleanup`), and assemble in `build_schedule()`; keep `lib.rs` mostly re-exports + docs.

9) gfx/pipeline.rs (~1302)
- Role: pipelines for many passes.
- Actions
  - Mechanical split to `renderer/pipelines/{sky,terrain,instanced,post_ao,ssgi,ssr,bloom,present}.rs` with common wrappers.
  - Back public API with typed wrappers to decouple module users from raw wgpu types.

Milestones & exit criteria
- All ≥1000‑line files either split or on a plan where their remaining size is due to tightly‑cohesive content (e.g., a cluster of small related builders).
- `render.rs` under ~800 LOC, containing only frame prep + graph build/execute.
- `mod.rs` under ~1200 LOC (or less) with no per‑pass draw code left.

