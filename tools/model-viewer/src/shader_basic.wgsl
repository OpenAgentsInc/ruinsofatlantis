struct Globals { view_proj: mat4x4<f32> };
@group(0) @binding(0) var<uniform> globals: Globals;

struct VSIn {
  @location(0) pos: vec3<f32>,
  @location(1) nrm: vec3<f32>,
};

struct VSOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) nrm: vec3<f32>,
};

@vertex
fn vs_main(input: VSIn) -> VSOut {
  var out: VSOut;
  out.pos = globals.view_proj * vec4<f32>(input.pos, 1.0);
  out.nrm = input.nrm;
  return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
  // Simple constant albedo for unskinned models (ruins is solid grey)
  return vec4<f32>(0.72, 0.72, 0.72, 1.0);
}
