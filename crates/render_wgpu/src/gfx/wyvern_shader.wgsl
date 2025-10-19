// Wyvern-only skinned pipeline (viewer-parity binding order)
// Groups:
//   set(0) Globals (matches engine Globals layout)
//   set(1) Skin palettes storage buffer
//   set(2) Material (sampler + texture)

struct Globals { view_proj: mat4x4<f32>, camRightTime: vec4<f32>, camUpPad: vec4<f32>, sunDirTime: vec4<f32>, sh: array<vec4<f32>, 9>, fog: vec4<f32>, clip: vec4<f32> };
@group(0) @binding(0) var<uniform> globals: Globals;

// Skin palette (joint matrices)
struct Palettes { mats: array<mat4x4<f32>> };
@group(1) @binding(0) var<storage, read> palettes: Palettes;

// Match engine material BGL order: binding(0)=texture, binding(1)=sampler
@group(2) @binding(0) var base_tex: texture_2d<f32>;
@group(2) @binding(1) var base_sam: sampler;

// Instance + skinned vertex input: mirrors engine VertexSkinned + InstanceSkin
struct WyvIn {
  @location(0) pos: vec3<f32>,
  @location(1) nrm: vec3<f32>,
  // instance mat4 + color/sel
  @location(2) i0: vec4<f32>,
  @location(3) i1: vec4<f32>,
  @location(4) i2: vec4<f32>,
  @location(5) i3: vec4<f32>,
  @location(6) icolor: vec3<f32>,
  @location(7) iselected: f32,
  // skin
  @location(8) joints: vec4<u32>,
  @location(9) weights: vec4<f32>,
  // palette base (per-instance)
  @location(10) palette_base: u32,
  // uvs
  @location(11) uv: vec2<f32>,
};

struct WyvOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) nrm: vec3<f32>,
  @location(1) world: vec3<f32>,
  @location(2) uv: vec2<f32>,
};

@vertex
fn vs_wyvern(input: WyvIn) -> WyvOut {
  let inst = mat4x4<f32>(input.i0, input.i1, input.i2, input.i3);
  let b = input.palette_base;
  let i0 = b + input.joints.x;
  let i1 = b + input.joints.y;
  let i2 = b + input.joints.z;
  let i3 = b + input.joints.w;

  let skinned_pos =
      (palettes.mats[i0] * vec4<f32>(input.pos, 1.0)) * input.weights.x +
      (palettes.mats[i1] * vec4<f32>(input.pos, 1.0)) * input.weights.y +
      (palettes.mats[i2] * vec4<f32>(input.pos, 1.0)) * input.weights.z +
      (palettes.mats[i3] * vec4<f32>(input.pos, 1.0)) * input.weights.w;

  let skinned_nrm = normalize(
      (palettes.mats[i0] * vec4<f32>(input.nrm, 0.0)).xyz * input.weights.x +
      (palettes.mats[i1] * vec4<f32>(input.nrm, 0.0)).xyz * input.weights.y +
      (palettes.mats[i2] * vec4<f32>(input.nrm, 0.0)).xyz * input.weights.z +
      (palettes.mats[i3] * vec4<f32>(input.nrm, 0.0)).xyz * input.weights.w);

  let world_pos = (inst * skinned_pos).xyz;
  var out: WyvOut;
  out.world = world_pos;
  out.nrm = normalize((inst * vec4<f32>(skinned_nrm, 0.0)).xyz);
  out.pos = globals.view_proj * vec4<f32>(world_pos, 1.0);
  out.uv = input.uv;
  return out;
}

@fragment
fn fs_wyvern(input: WyvOut) -> @location(0) vec4<f32> {
  let albedo = textureSample(base_tex, base_sam, input.uv).rgb;
  // Simple lambert vs sun dir for readability (like viewer)
  let L = normalize(globals.sunDirTime.xyz);
  let ndl = max(dot(normalize(input.nrm), L), 0.0);
  let shade = 0.25 + 0.75 * ndl;
  return vec4<f32>(albedo * shade, 1.0);
}
