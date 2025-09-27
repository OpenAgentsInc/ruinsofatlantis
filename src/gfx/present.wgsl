// Fullscreen present: samples SceneColor and writes to swapchain.

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_present(@builtin(vertex_index) vid: u32) -> VsOut {
  var p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
  var out: VsOut;
  out.pos = vec4<f32>(p[vid], 0.0, 1.0);
  out.uv = 0.5 * (p[vid] + vec2<f32>(1.0, 1.0));
  return out;
}

@fragment
fn fs_present(in: VsOut) -> @location(0) vec4<f32> {
  // Simple pass-through; tone mapping can be added later
  let c = textureSample(scene_tex, samp, in.uv);
  return c;
}

