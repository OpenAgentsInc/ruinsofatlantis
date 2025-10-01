// Fullscreen present: sample SceneColor, apply fog and tonemap, and write to swapchain.

// Globals layout mirrors src/gfx/types.rs (we only use fog and clip here).
struct Globals {
  view_proj: mat4x4<f32>,
  camRightTime: vec4<f32>,
  camUpPad: vec4<f32>,
  sunDirTime: vec4<f32>,
  sh: array<vec4<f32>, 9>,
  fog: vec4<f32>,    // rgb=color, a=density
  clip: vec4<f32>,   // x=znear, y=zfar
};
@group(0) @binding(0) var<uniform> globals: Globals;

@group(1) @binding(0) var scene_tex: texture_2d<f32>;
@group(1) @binding(1) var samp_color: sampler;
@group(1) @binding(2) var depth_tex: texture_depth_2d;
@group(1) @binding(3) var samp_depth: sampler;

struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_present(@builtin(vertex_index) vid: u32) -> VsOut {
  var p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
  var out: VsOut;
  out.pos = vec4<f32>(p[vid], 0.0, 1.0);
  // Flip Y so offscreen texture (origin top-left) appears upright on swapchain
  out.uv = vec2<f32>(0.5 * (p[vid].x + 1.0), 0.5 * (1.0 - p[vid].y));
  return out;
}

fn linearize_depth(d: f32, znear: f32, zfar: f32) -> f32 {
  // Assuming standard [0,1] depth
  return (2.0 * znear) / (zfar + znear - d * (zfar - znear));
}

fn tonemap_aces_approx(x: vec3<f32>) -> vec3<f32> {
  // Narkowicz 2015, ACES approximation
  let a = 2.51;
  let b = 0.03;
  let c = 2.43;
  let d = 0.59;
  let e = 0.14;
  return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// Convert linear RGB to sRGB for presentation to a non‑sRGB swapchain.
// Web swapchains commonly expose UNORM (non‑sRGB) formats; without this
// encode, the scene appears much darker (nearly black at night).
fn linear_to_srgb(x: vec3<f32>) -> vec3<f32> {
  let lo = 12.92 * x;
  let hi = 1.055 * pow(x, vec3<f32>(1.0/2.4)) - 0.055;
  let cutoff = vec3<f32>(0.0031308);
  return select(hi, lo, x <= cutoff);
}

@fragment
fn fs_present(in: VsOut) -> @location(0) vec4<f32> {
  // Clamp UV to avoid sampling exactly at 0/1 edges (prevents mirrored/clamped artifacts)
  let sz = vec2<f32>(textureDimensions(scene_tex));
  let eps = vec2<f32>(0.5) / sz;
  let uv = clamp(in.uv, eps, vec2<f32>(1.0) - eps);
  // DEBUG SIMPLIFICATION: sample the scene color directly and sRGB-encode.
  // Fog/tonemap/grade disabled to isolate sampling path.
  var col = textureSampleLevel(scene_tex, samp_color, uv, 0.0).rgb;
  let out_rgb = linear_to_srgb(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)));
  return vec4<f32>(out_rgb, 1.0);
}
