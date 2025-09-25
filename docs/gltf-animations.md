# glTF Skinning & Animations — What We Fixed

This notes how we took the wizard from a static T‑pose to playing glTF animations correctly, and what to watch for going forward.

## Symptoms
- Model rendered but stayed in bind pose (T‑pose).
- Logs showed JOINTS_0 = [0,0,0,0] and WEIGHTS_0 = [1,0,0,0].
- Clips were parsed, palettes were built, but the GPU had no skinning data to apply.

## Root Causes
- The mesh’s skinned vertex attributes were Draco‑compressed (KHR_draco_mesh_compression). If you don’t decode JOINTS_0/WEIGHTS_0, the skin stays in bind pose.
- Loader could pick a non‑skinned primitive if it didn’t explicitly prefer nodes that have a skin.

## Changes Made
- Prefer skinned nodes: we now iterate document nodes and pick a mesh attached to a node with `node.skin().is_some()`. This guarantees we render the skinned primitive.
- Robust asset prepare: `prepare_gltf_path()` tries the original `.gltf` first; if import fails, it uses a pre‑decompressed copy (`.decompressed.gltf`) or attempts an automatic CLI decompression.
- Native Draco path for skinned data: `decode_draco_skinned_primitive()` reads the Draco extension JSON, builds a decode config from the primitive’s attribute semantics, and unpacks:
  - POSITION (f32x3), NORMAL (f32x3), TEXCOORD set used by baseColor,
  - JOINTS_0 (u8/u16 → stored as u16x4), WEIGHTS_0 (u8/u16/f32 → f32x4; renormalized).
  The resulting vertices are interleaved into our `VertexSkinCPU`.
- Shader/player compatibility:
  - Vertex layout stores `joints: vec4<u16>` and `weights: vec4<f32>`; WGSL casts joints to `u32` for palette indexing.
  - Palette size matches `skin.joints().len()`; vertex shader multiplies `global * (skinned) * inverseBind` in the correct joint order.
- Sanity during development: we temporarily logged ranges for JOINTS/WEIGHTS and validated that at least one animated joint changed over time. These logs are now removed to keep the app quiet.

## What To Expect Now
- With a valid (decompressed or runtime‑decoded) skinned primitive, the loader returns non‑zero JOINTS/WEIGHTS and animations play.
- If a file still requires decompression and no CLI is available, the loader errors clearly; place a `*.decompressed.gltf` alongside the source to bypass Draco at runtime.

## Practical Notes
- Decompression tool (one‑time, optional):
  - `npx -y @gltf-transform/cli draco assets/models/wizard.gltf assets/models/wizard.decompressed.gltf --decode`
  - Keep the decompressed copy under `assets/models/` — the loader prefers it automatically.
- Run locally:
  - `cargo run` (the window opens; the wizard plays an idle/Waiting‑style clip).
- Asset hygiene:
  - When adding new skinned models, ensure they include JOINTS_0/WEIGHTS_0 (or provide a decompressed copy). Draco is fine as long as you decode those attributes.

## Future Hardening (optional)
- Gate slow path decoding behind a feature flag and prefer pre‑baked asset packs for distribution.
- Add a tiny unit test that loads a known skinned asset and asserts non‑zero JOINTS/WEIGHTS and matching palette size.

