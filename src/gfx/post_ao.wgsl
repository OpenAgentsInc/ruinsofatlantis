// Fullscreen SSAO-like postprocess using only depth.
// Very lightweight: samples 8 neighbors and darkens creases.

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
  // Assuming OpenGL-style depth in [0,1]
  return (2.0 * znear) / (zfar + znear - d * (zfar - znear));
}

@fragment
fn fs_ao(in: VsOut) -> @location(0) vec4<f32> {
  let znear = globals.clip.x;
  let zfar = globals.clip.y;
  let depth = textureSample(depth_tex, samp, in.uv);
  let zlin = linearize_depth(depth, znear, zfar);
  // Sample a small cross pattern
  let px = vec2<f32>(1.0 / 1920.0, 1.0 / 1080.0); // Approx; replaced by real res in future
  var occ = 0.0;
  let taps = array<vec2<f32>, 8>(
    vec2<f32>(1.0, 0.0), vec2<f32>(-1.0, 0.0), vec2<f32>(0.0, 1.0), vec2<f32>(0.0, -1.0),
    vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0), vec2<f32>(1.0, -1.0), vec2<f32>(-1.0, -1.0)
  );
  for (var i = 0u; i < 8u; i++) {
    let uv = in.uv + taps[i] * px * 2.0;
    let dn = textureSample(depth_tex, samp, uv);
    let zn = linearize_depth(dn, znear, zfar);
    // If neighbor is closer (smaller z), it occludes this pixel
    let delta = zn - zlin;
    occ += select(0.0, 1.0, delta < -0.005);
  }
  occ = clamp(occ / 8.0, 0.0, 1.0);
  let strength = 0.4; // mild
  let ao = 1.0 - strength * occ;
  return vec4<f32>(vec3<f32>(ao), 1.0);
}

