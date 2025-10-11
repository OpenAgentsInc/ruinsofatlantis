**Startup Latency Audit**

- Context: Campaign Builder zone (`ROA_ZONE=campaign_builder`)
- Observation: ~30–35 s from process start to first interactive frame on a warmed build.

**Timeline From Logs (wall-time deltas)**

- 0.0 s — App start; WGPU/device init (no explicit timestamp)
- +4.0 s — Renderer init starts with zone policy
  - render_wgpu: init message appears
- +4.5–6.0 s — UBC male rig and second rig load + merge 47 clips
  - roa_assets::skinning logs and “PC: merged GLTF animations …”
- +6.0 s — Trees begin (“trees: building 40 instances”)
- +8.0 s → +31.0 s — Repeated GLTF imports for Quaternius pines fail with images, then retry headless (buffers-only)
  - 13 s for first ‘pine_4’
  - 3 s for ‘pine_3’
  - 3 s for ‘pine_5’
  - 2 s for ‘pine_1’
  - 2 s for ‘pine_2’

The bulk (>60%) of the observed startup time is spent in repeated, failing `gltf::import()` calls for tree kinds, followed by a second parse path without images.

**Hotspots Identified**

- `crates/render_wgpu/src/gfx/foliage.rs` (build_trees_by_kind)
  - For each kind present in the baked snapshot (`trees.json` by_kind), we try a full `gltf::import()` which resolves external images, then build a material bind group from the baseColor. If any referenced file is missing or path resolution fails, we log a warning and retry via `gltf::Gltf::open()` (buffers-only) to at least get geometry.
  - The first import attempt is expensive; failures cost seconds per kind.
  - File refs: crates/render_wgpu/src/gfx/foliage.rs:260, crates/render_wgpu/src/gfx/foliage.rs:312

- `crates/render_wgpu/src/gfx/renderer/init.rs` (PC rig)
  - Even in Campaign Builder (HUD/casting disabled), the renderer still loads the UBC skinned mesh and merges the universal animation library. This adds ~1–2 s CPU time and file IO.
  - File refs: crates/render_wgpu/src/gfx/renderer/init.rs:964, crates/render_wgpu/src/gfx/renderer/init.rs:1068

- Shader/pipeline compilation
  - WGSL modules compile at init. In dev profile this is typically sub-second, but on some GPUs it adds a couple seconds; not the dominant factor here.

**Root Causes**

1) Full `gltf::import()` on every tree kind, with missing-image failures
   - The Quaternius GLTFs reference multiple PNGs. `gltf::import()` attempts to open each image on disk. Any mismatch in path, case, or missing file triggers a filesystem error and a retry path.
   - The retry (`Gltf::open`) avoids images but still requires re-parsing buffers, duplicating work.
   - Negative results are not cached, so we pay this cost every boot.

2) Unnecessary PC rig load in authoring zones
   - The platform side avoids preloading PC CPU assets for `campaign_builder`, but the renderer still creates the rig/material/palettes and merges animations on init.

3) Synchronous, single-threaded asset work on the critical path
   - GLTF parsing, PNG decoding (when it succeeds), buffer creation, and bind group creation all happen before the first frame.

**Quick Wins (Low Risk, High Impact)**

- Cache negative GLTF-with-images results per kind
  - Add a tiny in-memory map: kind → `ImagesPresent { path, ok }`.
  - When a kind fails once with `os error 2`, skip `gltf::import()` on subsequent boots and go directly to the buffers-only path (or a known-good fallback material).
  - Expected win: eliminate ~20+ s of repeated failures across pine variants.

- One-shot preflight for image existence
  - Before import, check the `images[]` URIs in the GLTF JSON (cheap parse via `gltf::Gltf::open`) and verify files exist next to the GLTF. If any are missing, don’t call `gltf::import()`; bind the default material and move on.
  - Expected win: same as above, but deterministic per run.

- Skip PC rig entirely when zone policy disables HUD/casting
  - Gate PC mesh/material/animation load on `zone_policy.show_player_hud || zone_policy.allow_casting`.
  - Campaign Builder and cc_demo will skip skinned-rig work completely.
  - Expected win: ~1–2 s.

- Defer tree material creation until first frame (lazy)
  - Keep geometry upload (VB/IB) in init, but delay material BG creation to first draw, per kind. Use the default material until the real texture is ready (or permanently when images are missing).
  - Expected win: a few seconds and smoother boot.

**Medium Wins**

- Convert tree assets to GLB with embedded textures
  - Replace `assets/trees/quaternius/glTF/*.gltf` + external PNGs with GLBs embedding textures. `gltf::import()` becomes a single file open; no directory walking.
  - Tooling: `gltf-transform copy in.gltf out.glb` (or Blender batch export).
  - Expected win: remove the failure class entirely, cut import time per kind to sub-second.

- Pre-bake a compact “foliage material”
  - For trees, a single baseColor texture (plus alpha mask) often suffices. Convert PNGs → KTX2 GPU-compressed textures and load directly (no CPU decode). Bind a shared sampler/material UBO.
  - Expected win: smaller IO, faster upload, less VRAM bandwidth.

- Asset cache for meshes/materials
  - Maintain a small cache keyed by absolute path so repeated kinds across zones reuse VB/IB/material BGs.
  - Expected win: avoids duplicate imports in multi-zone sessions.

**Bigger Changes (Architectural)**

- Background asset loader with placeholders
  - Boot with placeholders (default material + cube/low-LOD), kick off async loads per kind, and swap when ready. First frame in <3 s is achievable.
  - Requires thread-safe handoff of new buffers/BGs and a safe draw path while swapping.

- Zone snapshot augmentation
  - Persist a per-kind “asset key” and material hints in the snapshot (`trees.json`) so the client doesn’t need to rediscover baseColor textures every boot.
  - Pair with a manifest-level catalog mapping keys → GLB + KTX2.

**Measurements to Add (Visibility)**

- Instrumentation using `log::info!("took {ms} …")` around:
  - WGPU device/surface init
  - UBC rig load + animation merge
  - Trees by_kind import per kind (both with and without images)
  - Per-pipeline creation times
  - File refs: crates/render_wgpu/src/gfx/renderer/init.rs:388 (material BG), 964 (wizard material), 168–220 (policy/zone), crates/render_wgpu/src/gfx/foliage.rs:200–360

**Why the Pine Failures Are Expensive**

- `gltf::import()` opens the GLTF, all buffers, and every `images[]` URI. On any missing image, it returns an `io::Error`. We then:
  1) Log a warning
  2) Parse again via `Gltf::open()` (buffers only)
  3) Rebuild vertices/indices manually
  4) Bind the default material (white)

This pattern repeats for each pine variant: pine_1..5. On SSDs, each failure costs 2–13 s depending on disk pressure and PNG size, summing to ~20–25 s.

**Action Plan (Concrete)**

1) Guard PC rig load in renderer init
   - Skip all UBC/animation/material work when `!zone_policy.show_player_hud && !zone_policy.allow_casting`.
   - Files: crates/render_wgpu/src/gfx/renderer/init.rs:950–980

2) Preflight GLTF images, then choose import path
   - Parse `gltf::Gltf::open(path)` once, iterate `images()` and check each `source().source()` for a file in the same directory. If any missing → skip image import and bind default material.
   - Cache result in a `HashMap<PathBuf, bool>` (session static).

3) Optional env toggles for dev
   - `RA_TREES_NO_IMAGES=1`: force buffers-only path for tree kinds.
   - `RA_SKIP_PC=1`: skip skinned PC rig entirely.

4) Medium-term: convert trees to GLB + KTX2
   - Script to convert Quaternius GLTF folder → GLB with embedded textures; add to repo under `assets/trees/quaternius/GLB/`.
   - Update `foliage::path_for_kind` to prefer GLB.

**Estimated Impact**

- Quick wins alone: reduce boot from ~30–35 s to ~6–8 s on dev builds for Campaign Builder.
- With GLB conversion or async loader: ~2–4 s to first frame is realistic (device + pipelines + terrain + overlays).

**Appendix: File References (start lines)**

- foliage import + texture bind group
  - crates/render_wgpu/src/gfx/foliage.rs:200
  - crates/render_wgpu/src/gfx/foliage.rs:312

- renderer init: wizard/PC material and animations
  - crates/render_wgpu/src/gfx/renderer/init.rs:964
  - crates/render_wgpu/src/gfx/renderer/init.rs:1068

- material bind group layout
  - crates/render_wgpu/src/gfx/pipeline.rs:96

- zone policy application on set
  - crates/render_wgpu/src/gfx/mod.rs:672

