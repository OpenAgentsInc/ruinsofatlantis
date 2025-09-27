// Frame overlay: small animated/debug pattern to verify per-frame updates.

struct FrameDebug {
  frame_ix: u32,
};

@group(0) @binding(0) var<uniform> dbg: FrameDebug;

struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VsOut {
  var p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
  var out: VsOut;
  out.pos = vec4<f32>(p[vid], 0.0, 1.0);
  out.uv = 0.5 * (p[vid] + vec2<f32>(1.0, 1.0));
  return out;
}

@fragment
fn fs_overlay(in: VsOut) -> @location(0) vec4<f32> {
  // Draw in top-left corner: 20% width, 6% height
  if (in.uv.x > 0.2 || in.uv.y > 0.06) {
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
  }
  let f = f32(dbg.frame_ix);
  // Moving bar across width
  let t = fract(f * 0.01);
  let bar_x = t * 0.2; // 0..0.2 in uv
  let bar = step(abs(in.uv.x - bar_x), 0.002);
  // Color stripes from lower bits
  let r = f32((dbg.frame_ix >> 0u) & 1u);
  let g = f32((dbg.frame_ix >> 1u) & 1u);
  let b = f32((dbg.frame_ix >> 2u) & 1u);
  let base = vec3<f32>(r, g, b) * 0.8 + vec3<f32>(0.2, 0.2, 0.2);
  let col = mix(base, vec3<f32>(1.0, 1.0, 0.0), bar);
  return vec4<f32>(col, 1.0);
}

