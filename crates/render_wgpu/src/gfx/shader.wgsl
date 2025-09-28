// Basic WGSL used for both non-instanced and instanced draws.

struct Globals { view_proj: mat4x4<f32>, camRightTime: vec4<f32>, camUpPad: vec4<f32>, sunDirTime: vec4<f32>, sh: array<vec4<f32>, 9>, fog: vec4<f32>, clip: vec4<f32> };
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
  let t = globals.camRightTime.w;
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
  let light_dir = normalize(globals.sunDirTime.xyz);
  let ndl = max(dot(in.nrm, light_dir), 0.0);
  // SH-L2 ambient irradiance
  let n = in.nrm;
  let shb = array<f32,9>(
    0.282095,
    0.488603 * n.y,
    0.488603 * n.z,
    0.488603 * n.x,
    1.092548 * n.x * n.y,
    1.092548 * n.y * n.z,
    0.315392 * (3.0 * n.z * n.z - 1.0),
    1.092548 * n.x * n.z,
    0.546274 * (n.x * n.x - n.y * n.y)
  );
  var amb = vec3<f32>(0.0, 0.0, 0.0);
  for (var i:u32=0u; i<9u; i++) {
    let c = globals.sh[i].xyz;
    amb += c * shb[i];
  }
  // Convert ambient to a scalar intensity to avoid tinting albedo blue
  let amb_int = max(dot(amb, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.0);
  var base = model_u.color * (0.2 + 0.5 * amb_int + 0.8 * ndl) + model_u.emissive;
  // Subtle hemisphere ground bounce: greenish tint near low sun
  let sun = normalize(globals.sunDirTime.xyz);
  let sun_elev = max(sun.y, 0.0);
  let low_sun = smoothstep(0.0, 0.4, 1.0 - sun_elev);
  let hemi = clamp(0.5 * (1.0 - n.y), 0.0, 1.0);
  let tint_strength = 0.25 * low_sun * hemi; // up to 25% at horizon & downward normals
  let ground_tint = vec3<f32>(0.10, 0.14, 0.10);
  base += ground_tint * tint_strength;
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
  let light_dir = normalize(globals.sunDirTime.xyz);
  let ndl = max(dot(in.nrm, light_dir), 0.0);
  // SH ambient
  let n = in.nrm;
  let shb = array<f32,9>(
    0.282095,
    0.488603 * n.y,
    0.488603 * n.z,
    0.488603 * n.x,
    1.092548 * n.x * n.y,
    1.092548 * n.y * n.z,
    0.315392 * (3.0 * n.z * n.z - 1.0),
    1.092548 * n.x * n.z,
    0.546274 * (n.x * n.x - n.y * n.y)
  );
  var amb = vec3<f32>(0.0);
  for (var i:u32=0u; i<9u; i++) { amb += globals.sh[i].xyz * shb[i]; }
  // Scalar ambient to preserve material hue
  let amb_int = max(dot(amb, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.0);
  var base = in.icolor * (0.2 + 0.5 * amb_int + 0.8 * ndl) + model_u.emissive;
  // Subtle hemisphere ground bounce (packed like above)
  let sun = normalize(globals.sunDirTime.xyz);
  let sun_elev = max(sun.y, 0.0);
  let low_sun = smoothstep(0.0, 0.4, 1.0 - sun_elev);
  let hemi = clamp(0.5 * (1.0 - n.y), 0.0, 1.0);
  let tint_strength = 0.25 * low_sun * hemi;
  let ground_tint = vec3<f32>(0.10, 0.14, 0.10);
  base += ground_tint * tint_strength;
  if (in.sel > 0.5) {
    base = vec3<f32>(1.0, 1.0, 0.1);
  }
  return vec4<f32>(base, 1.0);
}

@fragment
fn fs_wizard(in: WizOut) -> @location(0) vec4<f32> {
  // Base color from material (keep viewer parity)
  let albedo = textureSample(base_tex, base_sam, in.uv).rgb;
  // Subtle Fresnel-like rim term to hint gloss under sun lighting.
  // Approximate view direction using camera right/up from Globals.
  let right = globals.camRightTime.xyz;
  let up = globals.camUpPad.xyz;
  // Forward is up x right (note: right x up = -forward)
  let fwd = normalize(cross(up, right));
  let ndv = max(dot(in.nrm, -fwd), 0.0);
  let rim = pow(1.0 - ndv, 3.0);
  let rim_strength = 0.15; // keep very subtle
  let color = clamp(albedo + vec3<f32>(rim * rim_strength), vec3<f32>(0.0), vec3<f32>(1.0));
  return vec4<f32>(color, 1.0);
}

// Skinned instanced pipeline (wizards)
struct WizIn {
  @location(0) pos: vec3<f32>,
  @location(1) nrm: vec3<f32>,
  // instance mat4 + color/sel (locations 2..7)
  @location(2) i0: vec4<f32>,
  @location(3) i1: vec4<f32>,
  @location(4) i2: vec4<f32>,
  @location(5) i3: vec4<f32>,
  @location(6) icolor: vec3<f32>,
  @location(7) iselected: f32,
  // vertex skinning inputs
  @location(8) joints: vec4<u32>,
  @location(9) weights: vec4<f32>,
  // per-instance palette base index
  @location(10) palette_base: u32,
  // UVs
  @location(11) uv: vec2<f32>,
};

struct WizOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) nrm: vec3<f32>,
  @location(1) world: vec3<f32>,
  @location(2) sel: f32,
  @location(3) icolor: vec3<f32>,
  @location(4) uv: vec2<f32>,
};

struct Palettes { mats: array<mat4x4<f32>> };
@group(2) @binding(0) var<storage, read> palettes: Palettes;

@vertex
fn vs_wizard(input: WizIn) -> WizOut {
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

  let world_pos = (model_u.model * inst * skinned_pos).xyz;

  var out: WizOut;
  out.world = world_pos;
  out.nrm = normalize((model_u.model * inst * vec4<f32>(skinned_nrm, 0.0)).xyz);
  out.pos = globals.view_proj * vec4<f32>(world_pos, 1.0);
  out.sel = input.iselected;
  out.icolor = input.icolor;
  out.uv = input.uv;
  return out;
}

@group(3) @binding(0) var base_tex: texture_2d<f32>;
@group(3) @binding(1) var base_sam: sampler;
struct MaterialXform { offset: vec2<f32>, scale: vec2<f32>, rot: f32, _pad: vec3<f32> };
@group(3) @binding(2) var<uniform> mat_xf: MaterialXform;

// ---- Particle billboard pipeline ----
struct PtcVert { @location(0) corner: vec2<f32> };
struct PtcInst {
  @location(1) pos: vec3<f32>,
  @location(2) size: f32,
  @location(3) color: vec3<f32>,
};
struct PtcOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) color: vec3<f32>,
};

@vertex
fn vs_particle(v: PtcVert, i: PtcInst) -> PtcOut {
  let right = globals.camRightTime.xyz;
  let up = globals.camUpPad.xyz;
  let world = i.pos + (right * v.corner.x + up * v.corner.y) * i.size;
  var o: PtcOut;
  o.pos = globals.view_proj * vec4<f32>(world, 1.0);
  o.color = i.color;
  return o;
}

@fragment
fn fs_particle(i: PtcOut) -> @location(0) vec4<f32> {
  return vec4<f32>(i.color, 1.0);
}

// ---- Text overlay pipeline (screen-space quads) ----
struct TextIn {
  @location(0) pos_ndc: vec2<f32>,
  @location(1) uv: vec2<f32>,
  @location(2) color: vec4<f32>,
};
struct TextOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) color: vec4<f32>,
};

@vertex
fn vs_text(v: TextIn) -> TextOut {
  var o: TextOut;
  o.pos = vec4<f32>(v.pos_ndc, 0.0, 1.0);
  o.uv = v.uv;
  o.color = v.color;
  return o;
}

@group(0) @binding(0) var text_atlas: texture_2d<f32>;
@group(0) @binding(1) var text_sampler: sampler;

@fragment
fn fs_text(i: TextOut) -> @location(0) vec4<f32> {
  let a = textureSample(text_atlas, text_sampler, i.uv).r;
  // Tinted text with alpha from atlas
  return vec4<f32>(i.color.rgb, i.color.a * a);
}

// ---- Health bar pipeline (screen-space colored quads) ----
struct BarIn { @location(0) pos_ndc: vec2<f32>, @location(1) color: vec4<f32> };
struct BarOut { @builtin(position) pos: vec4<f32>, @location(0) color: vec4<f32> };

@vertex
fn vs_bar(v: BarIn) -> BarOut {
  var o: BarOut;
  o.pos = vec4<f32>(v.pos_ndc, 0.0, 1.0);
  o.color = v.color;
  return o;
}

@fragment
fn fs_bar(i: BarOut) -> @location(0) vec4<f32> {
  return i.color;
}
