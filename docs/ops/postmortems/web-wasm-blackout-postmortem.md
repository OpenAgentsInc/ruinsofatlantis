Title: Web/wasm “black screen” postmortem (present path)

Context
- Symptom: Desktop build rendered fine, but Web/wasm showed an all‑black canvas while logs printed “pass sky ok / draw terrain indices=…”.
- Swapchain on Web reported `fmt=Bgra8Unorm srgb=false` (UNORM, not sRGB).
- Renderer had multiple recent changes around offscreen HDR, present, and resize.

Root causes (stacked)
- Gamma mismatch on Web: We were directly writing linear colors to a non‑sRGB swapchain (BGRA8Unorm). Without an explicit linear→sRGB encode, the image appears far darker (often “black at night”).
- WebGPU sampling rules: Rgba16Float is non‑filterable. Any code path that binds a Filtering sampler or uses `textureSample` (filtering) will validate but return zeros on some stacks. Non‑filtering samplers + `textureSampleLevel(..., 0.0)` are required.
- Depth sampling overload: WebGPU requires an integer LOD for `texture_depth_2d` (e.g., `textureSampleLevel(depth_tex, samp_depth, uv, 0u)`). Using a float LOD is invalid and can yield nonsense.
- Resize invalidation: Rebuilding attachments without also rebuilding the bind group that references the resized views (SceneColor/Depth) can leave present sampling a stale view. On web, surface clamps are frequent (max 2048), so this happens a lot.

Debugging path (what we did in order)
1) Proved swapchain writes with a one‑line shader change in `present.wgsl` returning solid magenta. Canvas turned magenta → the present pipeline writes to the swapchain.
2) Forced a bright clear color in the sky pass while drawing to the offscreen target to prove offscreen writes are visible when sampled. They were.
3) Simplified present: sample SceneColor with a NonFiltering sampler via `textureSampleLevel(scene_tex, samp_color, uv, 0.0)`, clamp, and linear→sRGB encode. No fog/tonemap/grade. This restored color on Web immediately.
4) Re‑enabled offscreen→present on Web (not direct‑present) whenever the swapchain is not sRGB, so we can apply sRGB encode in the present shader reliably. Kept direct‑present only for sRGB swapchains.
5) Hardened BGL + resize: present BGL declares non‑filterable sample type; bind group uses point (NonFiltering) sampler for color and depth; on resize we rebuild present_bg against the new views.

Key changes (code)
- Init path: Web uses offscreen→present when `config.format.is_srgb() == false`.
  - File: `crates/render_wgpu/src/gfx/renderer/init.rs`
  - Log line: “swapchain … is not sRGB; using offscreen+present for gamma‑correct output”.
- Present WGSL: sample SceneColor with NonFiltering, then sRGB encode.
  - File: `crates/render_wgpu/src/gfx/present.wgsl`
  - Current safe baseline keeps only sample + optional night attenuation; fog/tonemap/grade are parked to avoid regressions until the final tuning pass lands.
- Present BGL: non‑filterable sample type matches the HDR offscreen format (Rgba16Float).
  - File: `crates/render_wgpu/src/gfx/pipeline.rs` (create_present_bgl)
  - Bind groups: point (NonFiltering) sampler for color and depth, rebuilt on resize.
- Attachments: offscreen textures created with `RENDER_ATTACHMENT | TEXTURE_BINDING` (and `COPY_SRC` for debug).
  - File: `crates/render_wgpu/src/gfx/renderer/attachments.rs`

Validation cues
- Logs on Web should show:
  - “swapchain … srgb=false …”
  - “swapchain … is not sRGB; using offscreen+present for gamma‑correct output”
  - “render path: direct_present=false draw_fmt=Rgba16Float”
- If output goes black again:
  - Temporarily return magenta in `present.wgsl` to prove the present target pipeline is good.
  - Set a bright clear in the sky pass to prove offscreen writes, then step forward to sampling again.

Follow‑ups and incremental reactivation plan
- Nights: Our safe baseline restored color but initially looked too bright; we added a simple night attenuation factor in present (based on sun elevation) while we finish re‑enabling fog/tonemap/grade.
- Fog + tonemap + grade: these are parked behind a staged re‑enable to avoid reintroducing a black frame. Next steps:
  1) Re‑enable fog only (depth sample with NonFiltering, integer LOD). Verify on Web.
  2) Re‑enable tonemap (linear ACES approx) only. Verify on Web.
  3) Re‑enable the gentle color grade.
- Post‑effects (Web): keep bloom/SSR/SSGI off by default for stability. Add feature flags to enable them once the present path is fully solid.
- Resize: we already rebuild present_bg on resize. If we add more offscreen resources, include their bind groups in the resize path.

How to bisect regressions quickly
1) Magenta present: `present.wgsl` → `return vec4(1,0,1,1)`.
2) Bright sky clear to the offscreen color; if visible, sampling is the only remaining link.
3) Sample SceneColor with NonFiltering; if visible, re‑enable depth/fog and then tonemap last.

User‑visible impact
- Web now renders sky/terrain/units (parity with desktop color intent) instead of an all‑black canvas.
- Nights are currently attenuated with a simple multiplier (temporary) until the full tonemap/fog path is re‑tuned on Web.

Action items (owner → status)
- Re‑enable fog (depth sample with NonFiltering + integer LOD) → pending (stage 1)
- Re‑enable tonemap (linear ACES approx) → pending (stage 2)
- Re‑enable gentle grade → pending (stage 3)
- Consider behind‑the‑scenes feature flag to flip between “simple present” and “full present” without source edits → pending
- Add a short “present path” page under `docs/systems/` → pending

