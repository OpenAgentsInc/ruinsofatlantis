// Sky background: evaluates Hosek–Wilkie with CPU-provided parameters.

struct Globals { view_proj: mat4x4<f32>, camRightTime: vec4<f32>, camUpPad: vec4<f32>, sunDirTime: vec4<f32>, sh: array<vec4<f32>, 9>, fog: vec4<f32>, clip: vec4<f32> };
@group(0) @binding(0) var<uniform> globals: Globals;

// Packed HW params: each vec4 packs {p_i_R, p_i_G, p_i_B, _}
struct SkyU { params: array<vec4<f32>, 9>, radiances: vec4<f32>, sun_dir_time: vec4<f32> };
@group(1) @binding(0) var<uniform> sky: SkyU;

struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) ndc: vec2<f32> };

@vertex
fn vs_sky(@builtin(vertex_index) vi: u32) -> VsOut {
  // Fullscreen triangle
  var pos = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -3.0),
    vec2<f32>(-1.0,  1.0),
    vec2<f32>( 3.0,  1.0)
  );
  var out: VsOut;
  out.pos = vec4<f32>(pos[vi], 0.0, 1.0);
  out.ndc = pos[vi];
  return out;
}

fn sky_radiance(theta: f32, gamma: f32) -> vec3<f32> {
  // Unpack params per channel
  let r = sky.radiances.x; let g = sky.radiances.y; let b = sky.radiances.z;
  // For i in 0..9, p_i.{x,y,z} are RGB
  // Recreate the scalar function per channel
  let cos_gamma = cos(gamma);
  let cos_gamma2 = cos_gamma * cos_gamma;
  let cos_theta = abs(cos(theta));
  let exp_m = exp(sky.params[4].x * gamma);
  let ray_m = cos_gamma2;
  let mie_m_lhs = 1.0 + cos_gamma2;
  let mie_m_rhs_r = pow(1.0 + sky.params[8].x * sky.params[8].x - 2.0 * sky.params[8].x * cos_gamma, 1.5);
  let mie_m_rhs_g = pow(1.0 + sky.params[8].y * sky.params[8].y - 2.0 * sky.params[8].y * cos_gamma, 1.5);
  let mie_m_rhs_b = pow(1.0 + sky.params[8].z * sky.params[8].z - 2.0 * sky.params[8].z * cos_gamma, 1.5);
  let mie_m_r = mie_m_lhs / mie_m_rhs_r;
  let mie_m_g = mie_m_lhs / mie_m_rhs_g;
  let mie_m_b = mie_m_lhs / mie_m_rhs_b;
  let zenith = sqrt(cos_theta);

  let rr = eval_channel(r,
    sky.params[0].x, sky.params[1].x, sky.params[2].x, sky.params[3].x, sky.params[4].x, sky.params[5].x, sky.params[6].x, sky.params[7].x, sky.params[8].x,
    exp_m, ray_m, mie_m_r, zenith, cos_theta);
  let gg = eval_channel(g,
    sky.params[0].y, sky.params[1].y, sky.params[2].y, sky.params[3].y, sky.params[4].y, sky.params[5].y, sky.params[6].y, sky.params[7].y, sky.params[8].y,
    exp_m, ray_m, mie_m_g, zenith, cos_theta);
  let bb = eval_channel(b,
    sky.params[0].z, sky.params[1].z, sky.params[2].z, sky.params[3].z, sky.params[4].z, sky.params[5].z, sky.params[6].z, sky.params[7].z, sky.params[8].z,
    exp_m, ray_m, mie_m_b, zenith, cos_theta);
  return vec3<f32>(rr, gg, bb);
}

fn eval_channel(r: f32, p0: f32, p1: f32, p2: f32, p3: f32, p4: f32, p5: f32, p6: f32, p7: f32, p8: f32,
                exp_m: f32, ray_m: f32, mie_m: f32, zenith: f32, cos_theta: f32) -> f32 {
  let radiance_lhs = 1.0 + p0 * exp(p1 / (cos_theta + 0.01));
  let radiance_rhs = p2 + p3 * exp_m + p5 * ray_m + p6 * mie_m + p7 * zenith;
  return r * radiance_lhs * radiance_rhs;
}

@fragment
fn fs_sky(in: VsOut) -> @location(0) vec4<f32> {
  // Build an approximate world ray using camera basis
  let right = globals.camRightTime.xyz;
  let up = globals.camUpPad.xyz;
  let fwd = normalize(cross(right, up));
  // Use perspective-correct ray based on tan(fovy/2) and aspect
  let tan_half = globals.camUpPad.w;
  let aspect = globals.clip.z;
  let dir = normalize(fwd + right * (in.ndc.x * tan_half * aspect) + up * (in.ndc.y * tan_half));
  let theta = acos(clamp(dir.y, -1.0, 1.0));
  let gamma = acos(clamp(dot(dir, sky.sun_dir_time.xyz), -1.0, 1.0));
  var col = sky_radiance(theta, gamma);
  // Simple tonemap (Reinhard) to keep things in-gamut
  col = col / (1.0 + col);
  return vec4<f32>(col, 1.0);
}
