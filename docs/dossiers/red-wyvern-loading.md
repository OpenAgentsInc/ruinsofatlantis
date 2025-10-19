# Red Wyvern Loading — Engine vs. Viewer (Current State and Issues)

This dossier is the comprehensive, traceable record for how the Red Wyvern is loaded and rendered in the engine today, how that differs from the standalone model viewer, what we changed during investigation, what is still failing, and the shortest path to make it match the viewer exactly.

It is intentionally exhaustive and includes code references, logs, hypotheses, and a clear remediation plan.

## TL;DR

- The viewer renders `assets/anims/converted/RedDragon2021.glb` skinned and correct.
- The engine now loads the same GLB (skinned), merges 42 clips, and draws via a dedicated wyvern-only skinned pipeline that mirrors the viewer’s binding order.
- We fixed multiple concrete issues (attribute order, bind-group order mismatches, static pipeline binding, weight/joint hardening, env override, removal of static-texture borrowing).
- The wyvern still appears “mangled” in-engine under some conditions. Geometry is skinned (joints/indices OK), but the final result does not match the viewer.
- The remaining root cause is most likely one of: (1) a subtle palette upload/offset misuse, (2) a matrix space mismatch (model vs. instance), or (3) a submesh/material indexing side-effect. The remediation plan below makes validation unambiguous.

## Asset Resolution and Overrides

- Preferred skinned base is force-selected via env:
  - `ROA_WYVERN_BASE` or alias `OA_WYVERN_BASE`: engine honors either.
  - Code: crates/render_wgpu/src/gfx/wyvern.rs:31, crates/render_wgpu/src/gfx/renderer/init.rs:1640
- Minimal candidate list (no deep scans) when no override:
  - `assets/anims/converted/RedDragon2021.glb`
  - `assets/models/red_wyvern/RedDragon2021.glb`
  - Code: crates/render_wgpu/src/gfx/wyvern.rs:133

Relevant log (good path):

```
wyvern] override WYVERN_BASE -> .../assets/anims/converted/RedDragon2021.glb
skinning: selected skin index 0 (67150 verts)
wyvern: skinned ok (override): ... (verts=67150, idx=353478, joints=701, anims=21)
wyvern] merged 42 animation clips
```

## CPU Load Path (Skinned)

1) GLB import and skin selection
- Shared loader picks dominant skin and gathers all skin‑bound primitives.
- Code: shared/assets/src/skinning.rs:14, 58, 382, 494

2) Vertex stream extraction
- Reads positions, normals, UVs, joints (u8/u16), weights (u8/u16/f32).
- Code: shared/assets/src/skinning.rs:312–540

3) Submeshes and baseColor textures
- Each primitive contributes a submesh with optional baseColor.
- Code: shared/assets/src/skinning.rs:463–540

4) Animation clips
- All glTF clips are loaded; later we merge additional clips from companion GLBs.
- Code: shared/assets/src/skinning.rs:540–708

5) CPU → engine vertex mapping (hardening)
- We clamp joint indices to range and renormalize weights to protect against edge cases.
- Code: crates/render_wgpu/src/gfx/wyvern.rs:62–93, 284–306

6) Materials
- New policy: we do NOT borrow textures from the static textured GLB for the skinned model. If the skinned base lacks textures, we render untextured (white) to avoid UV mismatches.
- Code: crates/render_wgpu/src/gfx/wyvern.rs:94–104, 221–226

## GPU Pipelines and Bindings

Two different pipelines are relevant:

1) Wizard/skinned (legacy shared) — group order set(0)=globals, set(1)=model, set(2)=palettes, set(3)=material.
- Shader: crates/render_wgpu/src/gfx/shader.wgsl:407–468
- Pipeline: crates/render_wgpu/src/gfx/pipeline.rs:459–520

2) Wyvern-only skinned (viewer-parity) — group order set(0)=globals, set(1)=palettes (skin), set(2)=material.
- Shader: crates/render_wgpu/src/gfx/wyvern_shader.wgsl
- Pipeline creation: crates/render_wgpu/src/gfx/pipeline.rs:555–618
- Used by draw_wyvern (see below).

Static textured fallback has its own instanced-textured pipeline and a distinct layout: set(0)=globals, set(1)=model, set(2)=palettes (unused), set(3)=material.
- Shader: crates/render_wgpu/src/gfx/shader.wgsl:237–307
- Pipeline: crates/render_wgpu/src/gfx/pipeline.rs:180–233

## Vertex Formats and Layouts

Skinned Vertex (`VertexSkinned`)
- Order and locations are now ascending and match shaders: pos@0, nrm@1, joints@8, weights@9, uv@11.
- Code: crates/render_wgpu/src/gfx/types.rs:53–86

Instance (`InstanceSkin`)
- Instance matrix mat4 @ locations 2..5; color@6; selected@7; palette_base@10.
- Code: crates/render_wgpu/src/gfx/types.rs:169–217

Key fix applied: attribute order was previously non‑ascending (UV before joints), which could cause cross‑backend vertex fetch corruption. Now fixed.
- Commit: 68c8dff9

## Draw Paths (Binding Orders)

Skinned wyvern
- Uses wyvern-only pipeline (viewer parity).
- Binding order per draw:
  - set(0) globals BG
  - set(1) wyvern_palettes_bg (storage buffer)
  - set(2) material BG (per-submesh or single)
- Code: crates/render_wgpu/src/gfx/draw.rs:272–310

Static wyvern fallback (unskinned)
- Uses instanced textured pipeline, binding order:
  - set(0) globals, set(1) model, set(2) palettes (placeholder), set(3) material
- Code: crates/render_wgpu/src/gfx/draw.rs:180–205

## Animation Palette Update

Wyvern palette is updated every frame based on a selected clip (Idle/Fly_Loop or longest clip fallback) and uploaded to `wyvern_palettes_buf`.
- Code: crates/render_wgpu/src/gfx/mod.rs:5648–5689

Bind-pose freeze (for troubleshooting) is available via `ROA_WYVERN_BIND=1`.
- Code: crates/render_wgpu/src/gfx/mod.rs:5648–5689 (bind-pose branch)

## Materials & Submeshes (Skinned)

- Submesh list is taken from the skinned GLB; each submesh may carry its own baseColor.
- During init, we pre-bake per-submesh material BGs for wyvern to avoid pass-time creation.
- Code: crates/render_wgpu/src/gfx/renderer/init.rs:1708–1730

If the skinned GLB has no textures, wyvern renders untextured (white). We intentionally do not “borrow” textures from the static GLB anymore.
- Code: crates/render_wgpu/src/gfx/wyvern.rs:221–226

## Differences vs. Model Viewer

Viewer (tools/model-viewer):
- Group order: set(0)=Globals, set(2)=Skin (varies by viewer impl), set(1)=Material; but the viewer wiring is self-consistent and simple.
- Computes a bind-pose palette first and uses its own shader/path.
- Vertex layout and attribute order are simple/ascending.
- Viewer treats skinned model textures separately from static assets; it does not “borrow” from unrelated GLBs.

Engine (current):
- Now has a dedicated wyvern-only pipeline to mirror viewer’s binding structure and semantics.
- Global model UBO is not used by wyvern-only pipeline; the instance matrix (mat4 at 2..5) is the model transform (viewer parity).
- Materials are pre-baked at load.

## What We Changed During Investigation

1) Ensure wyvern loads skinned even in Picker mode (override honored)
- Code: crates/render_wgpu/src/gfx/renderer/init.rs:1640

2) Add env override + verbose probing
- Code: crates/render_wgpu/src/gfx/wyvern.rs:31

3) Fix attribute layout and offsets for skinned vertices (critical)
- Code: crates/render_wgpu/src/gfx/types.rs:61

4) Harden vertex mapping (clamp joints, renormalize weights)
- Code: crates/render_wgpu/src/gfx/wyvern.rs:62–93, 284–306

5) Correct bind-group orders in both skinned and static paths
- Wyvern-only skinned: set(0)=globals, set(1)=skin, set(2)=material
  - Shader: crates/render_wgpu/src/gfx/wyvern_shader.wgsl
  - Pipeline: crates/render_wgpu/src/gfx/pipeline.rs:555–618
  - Draw: crates/render_wgpu/src/gfx/draw.rs:272–310
- Static textured: set(0)=globals, set(1)=model, set(2)=palettes(unused), set(3)=material
  - Draw: crates/render_wgpu/src/gfx/draw.rs:180–205

6) Prebake wyvern submesh materials once in init
- Code: crates/render_wgpu/src/gfx/renderer/init.rs:1708–1730

7) Remove static borrowing of baseColor for skinned wyvern
- Code: crates/render_wgpu/src/gfx/wyvern.rs:94–104, 221–226

8) Minimal candidate list (no deep scans)
- Code: crates/render_wgpu/src/gfx/wyvern.rs:133

9) Accept env alias `OA_WYVERN_BASE` and adjust gating
- Code: crates/render_wgpu/src/gfx/wyvern.rs:31, crates/render_wgpu/src/gfx/renderer/init.rs:1640

## Current Symptoms (After Fixes)

- Logs confirm skinned load (joints=701), clip merge (42), and valid index/vertex counts.
- Still observe “mangled” look in-engine (wings/head not matching viewer), even when textures are suppressed (white), implying a transform issue (not a texture issue) remains.

## What’s Likely Still Wrong

Given the fixes above, these are the top hypotheses:

1) Palette base/offset misuse
- Instance `palette_base` is set to 0 for wyvern (single instance). That is correct, but if the palette buffer upload or range math ever desynchronizes from draw, you can see severe deformation. We should assert length and ranges per frame and/or draw.

2) Matrix space mismatch in wyvern-only pipeline
- Wizard pipeline multiplies `(model_u.model * inst * skinned_pos)`; wyvern-only pipeline uses `(inst * skinned_pos)` to mirror the viewer. If any downstream pass or global expects the model UBO to be active (e.g., for lights in world space), normals/world may be off. We should spot-check world-space construction against expectations.

3) Submesh/material indexing side-effects
- Although materials no longer affect transforms, incorrect per-submesh index ranges could masquerade as geometry corruption. We should validate each submesh’s draw range against CPU ranges and ensure no overflow/overrun.

4) Skinning matrices content
- If the joint palette contains wrong matrices (e.g., wrong node global or bad inverse bind ordering), the whole rig deforms. The viewer computes a bind-pose palette; we do too (for bind-pose test) and animate similarly, but we should dump/select a known clip and capture several joint matrices at draw to compare against the viewer’s output.

## Validation Plan (Concrete)

1) Add a wyvern debug mode `ROA_WYVERN_DEBUG=1` that:
- Captures and logs: first 8 palette matrices (flattened 4x4) each frame for two frames.
- Checks palette buffer length equals `wyvern_count * wyvern_joints` and asserts no overflow.
- Dumps first submesh draw range (start,count) and validates they’re within index buffer length (u16).

2) Force a known clip
- Add `ROA_WYVERN_CLIP=Idle` (or name substring) and log the selected clip, duration, and time.
- Ensures deterministic pose for comparison.

3) Head-pitch correction parity (viewer feature)
- Port the viewer’s head pitch correction and allow `ROA_WYVERN_HEAD_PITCH_DEG` to confirm posture alignment quickly.

4) Optional: single-submesh draw
- Temporarily draw only the first submesh with a solid color to verify index ranges and vertex fetch.

## Proposed Remediation (Step-by-step)

Short path to parity:

1) Implement `ROA_WYVERN_CLIP` and log pick + duration.
2) Add `ROA_WYVERN_DEBUG=1` and dump: joint count, palette length, first submesh range, first 2–4 joint matrices, and first 8 vertex joints/weights from VB to correlate CPU/GPU.
3) If matrices mismatch expected values from viewer, compare inverse bind order and joint-node mapping (names) across both loaders.
4) If matrices are correct but deform still happens, inspect the instance matrix path and remove any extra transforms from the pass (ensure world-space constructed consistently across pipelines).
5) Keep static fallback intact; do not reuse static albedo for skinned.

## Code References (Key Sites)

- Override + candidates: crates/render_wgpu/src/gfx/wyvern.rs:31, 133
- CPU→GPU vertex mapping + hardening: crates/render_wgpu/src/gfx/wyvern.rs:62–93, 284–306
- Remove borrowing of textures for skinned: crates/render_wgpu/src/gfx/wyvern.rs:94–104, 221–226
- Per-submesh material pre-bake: crates/render_wgpu/src/gfx/renderer/init.rs:1708–1730
- Wyvern-only shader: crates/render_wgpu/src/gfx/wyvern_shader.wgsl
- Wyvern-only pipeline creation: crates/render_wgpu/src/gfx/pipeline.rs:555–618
- Wyvern draw (bind order and per-submesh draw): crates/render_wgpu/src/gfx/draw.rs:272–310
- Static textured pipeline draw (corrected binds): crates/render_wgpu/src/gfx/draw.rs:180–205
- Wizard shared shader (for comparison): crates/render_wgpu/src/gfx/shader.wgsl:407–468
- Attribute layouts: crates/render_wgpu/src/gfx/types.rs:53, 169
- Animation sampling (palette build): crates/render_wgpu/src/gfx/anim.rs:8–68

## Known Good vs Known Bad (Ground Truth)

- Viewer loads `assets/anims/converted/RedDragon2021.glb` and shows correct geometry with 21 clips available (loggable there). Bind-pose and Idle clip are stable.
- Engine now loads the same GLB and reports indices/verts/joints consistent with the viewer. Despite this, deformation persists in-engine.
- This strongly suggests a remaining transform chain mismatch (palette or instance/world), not texture or candidate selection.

## Next Work Items (Owner: Graphics)

1) Add `ROA_WYVERN_DEBUG` to dump palette and VB slices at draw (one frame).
2) Add `ROA_WYVERN_CLIP` and a hard-coded list of clip aliases (e.g., Idle, Fly_Loop).
3) Implement viewer head-pitch correction parity and compare posture.
4) If mismatch persists, temporarily switch wyvern-only pipeline to multiply a unit model UBO (add small model_bgl) and compare vs. instance-only transform to isolate a space mismatch.
5) If still failing, instrument shared/assets palette generation and verify inverse bind order vs. glTF skin joints.

---

This dossier will be kept current as we iterate. Once the debug fences above are in place, we can capture a single frame’s matrices and resolve the remaining mismatch quickly.

