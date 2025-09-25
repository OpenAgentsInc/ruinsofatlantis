struct Globals { mvp: mat4x4<f32> };
@group(0) @binding(0) var<uniform> globals: Globals;

@group(1) @binding(0) var base_tex: texture_2d<f32>;
@group(1) @binding(1) var base_sam: sampler;

struct VSIn {
  @location(0) pos: vec3<f32>,
  @location(1) uv: vec2<f32>,
};

struct VSOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(input: VSIn) -> VSOut {
  var out: VSOut;
  out.pos = globals.mvp * vec4<f32>(input.pos, 1.0);
  out.uv = input.uv;
  return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
  // glTF uses top-left origin; WebGPU/WGSL sampling also uses top-left.
  let color = textureSample(base_tex, base_sam, in.uv);
  return vec4<f32>(color.rgb, 1.0);
}

