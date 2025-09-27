struct Globals { view_proj: mat4x4<f32>, time_pad: vec4<f32>, clip: vec4<f32> };
@group(0) @binding(0) var<uniform> globals: Globals;

@group(1) @binding(0) var base_tex: texture_2d<f32>;
@group(1) @binding(1) var base_sam: sampler;

struct VSIn {
  @location(0) pos: vec3<f32>,
  @location(1) uv: vec2<f32>,
  @location(2) i0: vec4<f32>,
  @location(3) i1: vec4<f32>,
  @location(4) i2: vec4<f32>,
  @location(5) i3: vec4<f32>,
};

struct VSOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(input: VSIn) -> VSOut {
  var out: VSOut;
  let inst = mat4x4<f32>(input.i0, input.i1, input.i2, input.i3);
  out.pos = globals.view_proj * (inst * vec4<f32>(input.pos, 1.0));
  out.uv = input.uv;
  return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
  let c = textureSample(base_tex, base_sam, in.uv);
  return vec4<f32>(c.rgb, 1.0);
}
