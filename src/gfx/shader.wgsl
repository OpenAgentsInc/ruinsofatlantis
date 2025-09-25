// Basic WGSL used for both non-instanced and instanced draws.

struct Globals { view_proj: mat4x4<f32>, time_pad: vec4<f32> };
@group(0) @binding(0) var<uniform> globals: Globals;

struct Model { model: mat4x4<f32>, color: vec3<f32>, emissive: f32, _pad: vec2<f32> };
@group(1) @binding(0) var<uniform> model_u: Model;

struct VSIn {
  @location(0) pos: vec3<f32>,
  @location(1) nrm: vec3<f32>,
};

struct VSOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) nrm: vec3<f32>,
  @location(1) world: vec3<f32>,
};

@vertex
fn vs_main(input: VSIn) -> VSOut {
  var p = input.pos;
  // Cheap ripple for y==0 plane tiles
  if (abs(input.nrm.y) > 0.9 && abs(p.y) < 0.0001) {
    let amp = 0.05;
    let freq = 0.5;
    let t = globals.time_pad.x;
    p.y = amp * sin(p.x * freq + t * 1.5) + amp * cos(p.z * freq + t);
  }
  let world_pos = (model_u.model * vec4<f32>(p, 1.0)).xyz;
  var out: VSOut;
  out.world = world_pos;
  out.nrm = normalize((model_u.model * vec4<f32>(input.nrm, 0.0)).xyz);
  out.pos = globals.view_proj * vec4<f32>(world_pos, 1.0);
  return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
  let light_dir = normalize(vec3<f32>(0.3, 1.0, 0.4));
  let ndl = max(dot(in.nrm, light_dir), 0.0);
  let base = model_u.color * (0.2 + 0.8 * ndl) + model_u.emissive;
  return vec4<f32>(base, 1.0);
}

// Instanced pipeline
struct InstIn {
  @location(0) pos: vec3<f32>,
  @location(1) nrm: vec3<f32>,
  @location(2) i0: vec4<f32>,
  @location(3) i1: vec4<f32>,
  @location(4) i2: vec4<f32>,
  @location(5) i3: vec4<f32>,
  @location(6) icolor: vec3<f32>,
  @location(7) iselected: f32,
};

struct InstOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) nrm: vec3<f32>,
  @location(1) world: vec3<f32>,
  @location(2) sel: f32,
  @location(3) icolor: vec3<f32>,
};

@vertex
fn vs_inst(input: InstIn) -> InstOut {
  let inst = mat4x4<f32>(input.i0, input.i1, input.i2, input.i3);
  let world_pos = (model_u.model * inst * vec4<f32>(input.pos, 1.0)).xyz;
  var out: InstOut;
  out.world = world_pos;
  out.nrm = normalize((model_u.model * inst * vec4<f32>(input.nrm, 0.0)).xyz);
  out.pos = globals.view_proj * vec4<f32>(world_pos, 1.0);
  out.sel = input.iselected;
  out.icolor = input.icolor;
  return out;
}

@fragment
fn fs_inst(in: InstOut) -> @location(0) vec4<f32> {
  let light_dir = normalize(vec3<f32>(0.3, 1.0, 0.4));
  let ndl = max(dot(in.nrm, light_dir), 0.0);
  var base = in.icolor * (0.2 + 0.8 * ndl) + model_u.emissive;
  if (in.sel > 0.5) {
    base = vec3<f32>(1.0, 1.0, 0.1);
  }
  return vec4<f32>(base, 1.0);
}

