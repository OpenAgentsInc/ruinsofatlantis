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
@group(1) @binding(1) var samp: sampler;
@group(1) @binding(2) var depth_tex: texture_depth_2d;

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

@fragment
fn fs_present(in: VsOut) -> @location(0) vec4<f32> {
  var col = textureSample(scene_tex, samp, in.uv).rgb;
  // Fog (exponential) based on linearized depth
  let depth = textureSample(depth_tex, samp, in.uv);
  let zlin = linearize_depth(depth, globals.clip.x, globals.clip.y);
  let density = globals.fog.a;
  if (density > 0.0) {
    let f = 1.0 - exp(-density * zlin);
    col = mix(col, globals.fog.rgb, clamp(f, 0.0, 1.0));
  }
  // Tonemap in linear
  let mapped = tonemap_aces_approx(col);
  return vec4<f32>(mapped, 1.0);
}
