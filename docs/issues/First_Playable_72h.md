# First Playable (72h) — Repo-Accurate Plan

North Star (target demo)
- Third-person camera over procedural terrain with Hosek–Wilkie sky, fog, and basic tonemap.
- Press 1 to cast Fire Bolt with a visible trail and impact particles (no balance yet).
- 20+ wizards (skinned + instanced) idling/casting; simple NPC enemies to shoot.
- Minimal HUD; add a small perf overlay (frametime, draw calls).

Notes on current repo status (as of this issue)
- Render loop/input: present under `src/platform_winit.rs` and `src/gfx/` with WASD, RMB orbit, scroll zoom, Space pause, [`[`|`]`] time scrub, `-`/`=` time scale. Entry: `src/main.rs`.
- Sky/TOD: `src/gfx/sky.rs` (CPU HW sky + SH ambient) and `src/gfx/sky.wgsl` (fullscreen sky). Present pass `src/gfx/present.wgsl` is pass-through (no tonemap yet).
- Terrain: deterministic CPU heightmap + normals, VB/IB upload, tree instancing in `src/gfx/terrain.rs`. Zone manifest in `data/zones/wizard_woods/manifest.json`. Optional baking tool `src/bin/zone_bake.rs`.
- Characters: wizard (GPU skinning) + zombies (GPU skinning) + static ruins. Scene assembly in `src/gfx/scene.rs`. Animation sampling in `src/gfx/anim.rs`.
- VFX: simple projectile + particle billboards wired in `src/gfx/fx` paths inside `src/gfx/mod.rs` and shaded in `src/gfx/shader.wgsl`.
- UI/HUD: nameplates, health bars, and a minimal HUD in `src/gfx/ui.rs`. No perf overlay yet.
- Lighting M1 scaffolding exists (G-Buffer, Hi‑Z, SSR/SSGI placeholders), see `src/gfx/{gbuffer.rs,hiz.rs}` and WGSL under `src/gfx/`.

Day 1 — Verify loop, add fog/tonemap polish
1) Render loop + input check
  - Files: `src/main.rs`, `src/platform_winit.rs`, `src/gfx/mod.rs`
  - Do: No code needed; confirm the loop renders the sky/terrain/scene and input maps are documented in `src/README.md`.
  - Accept: Window opens, sky + terrain + wizards visible; RMB orbit + scroll zoom; WASD moves PC; Space toggles sky pause; `[`/`]` scrubs time.
  - Commands: `cargo build`, `cargo test`, `cargo clippy -- -D warnings`
  - Fallback: If a machine is headless/CI, app exits early per `RA_HEADLESS`/CI logic.

2) Fog + tonemap in present pass
  - Files: `src/gfx/present.wgsl`, `src/gfx/types.rs` (uses `Globals.fog_params`), `src/gfx/camera_sys.rs` (sets fog color/density), `src/gfx/mod.rs` (writes globals)
  - Do: Implement a simple exponential fog and ACES-approx or Reinhard tonemap in `fs_present`. Use `Globals.fog_params.rgb = fog color`, `.a = density`.
  - Accept: Distant terrain washes into fog; night scenes remain in gamut; daytime remains punchy.
  - Fallback: Keep Reinhard if ACES-approx takes too long; expose fog density as a small constant.

3) Terrain QA (no LOD yet)
  - Files: `src/gfx/terrain.rs`, `data/zones/wizard_woods/manifest.json`
  - Do: Validate sizes from manifest (129, extent 150) render without holes; ensure trees load or gracefully fall back to cubes if OBJ isn’t present.
  - Accept: Camera can travel across terrain; no index overflows; frame stable on mid GPUs.
  - Fallback: Reduce terrain dimension in manifest if perf spikes on low-tier laptops.

Day 2 — Character polish and scalable crowd
4) Wizard GPU skinning + idle/cast wiring (existing)
  - Files: `src/gfx/{anim.rs,material.rs,scene.rs,draw.rs,shader.wgsl}`
  - Do: Confirm GPU skinning path renders with correct palette indexing; PC casts on 1 with a short delay. Ensure `hand_right_node` hook spawns bolts from the right hand when available.
  - Accept: Center wizard idles; pressing 1 triggers PortalOpen and then a Fire Bolt. No obvious skinning artifacts.
  - Fallback: CPU-skin one frame per wizard if palette upload becomes a bottleneck (acceptable for first playable).

5) Instance 20–50 wizards (scalable)
  - Files: `src/gfx/scene.rs`, `src/gfx/mod.rs` (instance buffers and per-instance time offsets)
  - Do: Start at ~20 wizards (current: 1 + 19 ring). If perf allows, add a second ring or wider spacing parameterized by zone manifest or a local constant.
  - Accept: 20–50 wizards render smoothly; animations out-of-phase; no Z-fighting.
  - Fallback: Keep 20; swap distant wizards for colored capsules if GLTF load perf is borderline on low-end.

6) Simple reflection/shine hint (skip true IBL for now)
  - Files: `src/gfx/shader.wgsl`
  - Do: Add a subtle Fresnel-ish rim term on wizards to hint gloss under sun lighting. Avoid material system expansion for this milestone.
  - Accept: Wizard cloth/metal read with a mild spec highlight when facing away from sun.
  - Fallback: Keep current lambert + SH ambient; revisit after Lighting M1 completes.

Day 3 — VFX, HUD, and capture polish
7) Fire Bolt VFX (existing)
  - Files: In-place within `src/gfx/mod.rs` (projectiles/particles) and `src/gfx/shader.wgsl`
  - Do: Verify trail and impact sprites render and cull; particles fade; budget ≤1ms on mid GPU.
  - Accept: Press 1 → fiery streak with visible impact puff; no frame hitch.
  - Fallback: Reduce particle count/lifetime.

8) Minimal perf overlay (new)
  - Files: `src/gfx/ui.rs` (re-use text atlas), `src/gfx/mod.rs` (wire toggle and counters)
  - Do: Display frametime ms (EMA), FPS, and approximate draw call count. Toggle with F1. Draw in the HUD pass (screen-space).
  - Accept: F1 shows/hides overlay; numbers update each frame; negligible cost.
  - Fallback: Frametime/FPS only; add draw call count later.

9) Screenshot mode + HUD toggle
  - Files: `src/gfx/mod.rs`, `src/gfx/camera_sys.rs`, `src/gfx/ui.rs`
  - Do: Bind F5 to a 5s smooth orbit around the PC; bind H to hide/show HUD elements.
  - Accept: Orbit yields a stable capture path; HUD hides cleanly.
  - Fallback: Simple fixed-rate yaw spin for 5s if path smoothing takes time.

10) Stability & QA pass
  - Checklist: resize, alt-tab, minimized restore, frame pacing stability; clamp dt spikes; keep per-frame heap allocs out of hot path.
  - Commands: `cargo build`, `cargo test`, `cargo clippy -- -D warnings`
  - Optional: Add `--no-vsync` CLI flag in `src/main.rs` and feed to surface config (nice-to-have).

Strict kill-/swap-switches
- If tonemap tuning drags: keep Reinhard for present; adjust fog color/density to maintain readability.
- If crowd perf dips: cap wizards at 20 and keep all polish; prioritize perf overlay and screenshot mode.
- If text atlas causes issues: display perf text via a fixed-width debug font subset only.

Visual priorities (if time remains)
1) Bloom (post) — cheap wow: `src/gfx/pipeline.rs` + a small WGSL `post_bloom.wgsl` sampling `SceneColor`.
2) Color grading LUT — add a 3D LUT sample (Atlantis teal) in present.
3) Tree wind — low-amplitude vertex sway near camera in `src/gfx/shader.wgsl` based on world pos.

Acceptance demo script
1) Golden hour: F5 for a smooth orbit, H to hide HUD.
2) Pan to the wizard ring; walk between them (WASD); show soft sky/ambient.
3) Face NPCs and cast Fire Bolt 2–3×; watch trails and impacts.
4) Toggle perf overlay (F1) briefly; scrub time with `[`/`]`; end with a night shot into fog.

Deliverables
- Build: `cargo build` (devs run `cargo run` locally)
- Config: `data/zones/wizard_woods/manifest.json` (sky/terrain params)
- Assets: `assets/models/wizard.gltf` (+ pre-decompressed `wizard.decompressed.gltf` already present), `assets/models/ruins.gltf`, zombie GLBs.
- Code: changes isolated to files listed above under each step; keep `src/README.md` updated when adding/touching modules.
- Tests: keep CPU-only tests deterministic (terrain/sky already have coverage); add small unit tests if new math/helpers are introduced.

Out-of-scope for this milestone (tracked separately)
- Shadows (CSM) and IBL pipelines: see `docs/issues/Lighting_M1.md`..`Lighting_M4.md` and `docs/lighting.md`. Defer to lighting milestones.
- Full PBR and material system expansion; stick to current simple shading + small polish.

