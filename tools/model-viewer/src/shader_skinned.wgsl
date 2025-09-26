struct Globals { view_proj: mat4x4<f32> };
@group(0) @binding(0) var<uniform> globals: Globals;

@group(1) @binding(0) var base_sampler: sampler;
@group(1) @binding(1) var base_tex: texture_2d<f32>;

struct Skin { joints: array<mat4x4<f32>> };
@group(2) @binding(0) var<storage, read> skin: Skin;

struct VSIn {
  @location(0) pos: vec3<f32>,
  @location(1) nrm: vec3<f32>,
  @location(2) uv0: vec2<f32>,
  @location(3) joints0: vec4<u32>,
  @location(4) weights0: vec4<f32>,
};

struct VSOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv0: vec2<f32>,
};

@vertex
fn vs_main_skinned(in: VSIn) -> VSOut {
  var o: VSOut;
  let j = in.joints0;
  let w = in.weights0;
  let m = skin.joints[j.x] * w.x + skin.joints[j.y] * w.y + skin.joints[j.z] * w.z + skin.joints[j.w] * w.w;
  let wp = m * vec4<f32>(in.pos, 1.0);
  o.pos = globals.view_proj * wp;
  o.uv0 = in.uv0;
  return o;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
  let c = textureSample(base_tex, base_sampler, in.uv0);
  return c;
}
