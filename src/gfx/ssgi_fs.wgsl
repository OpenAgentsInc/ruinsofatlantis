// Simple SSGI-like additive pass: samples SceneColor around the pixel
// guided by depth discontinuities. This is a placeholder for compute SSGI.

struct Globals {
  view_proj: mat4x4<f32>,
  camRightTime: vec4<f32>,
  camUpPad: vec4<f32>,
  sunDirTime: vec4<f32>,
  sh: array<vec4<f32>, 9>,
  fog: vec4<f32>,
  clip: vec4<f32>, // x=znear, y=zfar
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var depth_tex: texture_depth_2d;
@group(1) @binding(1) var samp: sampler;
@group(2) @binding(0) var scene_tex: texture_2d<f32>;
@group(2) @binding(1) var scene_samp: sampler;

struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VsOut {
  var p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
  var out: VsOut;
  out.pos = vec4<f32>(p[vid], 0.0, 1.0);
  out.uv = 0.5 * (p[vid] + vec2<f32>(1.0, 1.0));
  return out;
}

fn linearize_depth(d: f32, znear: f32, zfar: f32) -> f32 {
  return (2.0 * znear) / (zfar + znear - d * (zfar - znear));
}

@fragment
fn fs_ssgi(in: VsOut) -> @location(0) vec4<f32> {
  let znear = globals.clip.x; let zfar = globals.clip.y;
  let d0 = textureSample(depth_tex, samp, in.uv);
  if (d0 >= 1.0) { return vec4<f32>(0.0); }
  let z0 = linearize_depth(d0, znear, zfar);
  let texSize = vec2<f32>(textureDimensions(scene_tex));
  let px = 1.0 / texSize;
  // 8-tap disk
  let taps = array<vec2<f32>, 8>(
    vec2<f32>(1.0, 0.0), vec2<f32>(-1.0, 0.0), vec2<f32>(0.0, 1.0), vec2<f32>(0.0, -1.0),
    vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0), vec2<f32>(1.0, -1.0), vec2<f32>(-1.0, -1.0)
  );
  var acc = vec3<f32>(0.0);
  var wsum = 0.0;
  for (var i = 0u; i < 8u; i++) {
    let uv = in.uv + taps[i] * px * 2.0;
    let di = textureSample(depth_tex, samp, uv);
    if (di >= 1.0) { continue; }
    let zi = linearize_depth(di, znear, zfar);
    // Prefer samples at similar or slightly farther depth (avoid foreground bleeding)
    let dz = zi - z0;
    let w = clamp(1.0 - abs(dz) * 20.0, 0.0, 1.0);
    let ci = textureSample(scene_tex, scene_samp, uv).rgb;
    acc += ci * w;
    wsum += w;
  }
  if (wsum > 0.0) { acc /= wsum; }
  // Small gain to avoid over-brightening; acts like subtle bounce
  let gain = 0.08;
  return vec4<f32>(acc * gain, 1.0);
}

