# Wizard Viewer (Standalone)

This is a minimal, isolated viewer for `assets/models/wizard.gltf`. It bypasses the game renderer to validate UVs, textures, and sRGB handling with a tight WGPU loop.

## Why a Separate Viewer
- Eliminate unrelated state (skinning, ECS, instancing) while debugging the wizard’s base‑color mapping.
- Provide a single file you can tweak quickly (shader/loader/camera) without touching the main app.

## Files
- `src/bin/wizard_viewer.rs` — winit + wgpu app that loads and draws the first glTF mesh/primitive.
- `src/bin/wizard_viewer.wgsl` — textured shader (pos/uv → sample baseColor).

## WGPU Setup
- Creates a surface and clamps the swapchain size to the device’s `max_texture_dimension_2d` to avoid validation errors on high‑dpi displays.
- Config uses an sRGB format (Metal-friendly) and a `Depth32Float` depth buffer.

## glTF Loading
- Uses `gltf::import` to resolve buffers and embedded images.
- Loads the first mesh primitive:
  - Positions (`POSITION`)
  - UVs: chooses the `texCoord` referenced by `pbrMetallicRoughness.baseColorTexture` (fallback: simple planar UVs)
  - Indices (u8/u16/u32 → u32)
- Base‑color image: converts to RGBA8 if needed; uploads as `Rgba8UnormSrgb`.

## GPU & Shader Details
- Vertex buffer layout: `pos: Float32x3`, `uv: Float32x2`.
- Bind groups:
  - `@group(0)`: uniform `Globals { mvp }`
  - `@group(1)`: texture + sampler (filterable float)
- WGSL samples `textureSample(base_tex, base_sam, in.uv)` without V‑flip (glTF and WebGPU both use top‑left origin).

## Camera & Render Loop
- Simple orbiting camera around the origin; projects with `perspective_rh_gl`.
- Draws one indexed primitive.

## How to Run
- `cargo run --bin wizard_viewer`
- If your display is very large (hi‑DPI), the app clamps to the device limit and logs the chosen size.

## Limitations & Next Steps
- No skinning: the wizard renders in bind pose (sufficient for UV/texture validation).
- No KHR extensions: add `KHR_texture_transform` easily by multiplying UVs with an affine transform in WGSL.
- Single primitive/material: extend to loop all primitives and draw each with its material.

## Troubleshooting
- If output looks flat/gray: verify the UV gradient by replacing the fragment with `vec4(in.uv, 0, 1)`.
- If colors look washed: confirm `Rgba8UnormSrgb` for the texture and an sRGB swapchain format.
- If you hit a surface validation error on startup: your window is too large; the clamping code reduces it and reconfigures the surface.
