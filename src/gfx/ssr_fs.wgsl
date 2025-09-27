// Very simple SSR-like overlay using linear depth and SceneColor.
// Approximates normals from depth gradients; marches a few steps in screen space.

@group(0) @binding(0) var lin_depth_tex: texture_2d<f32>;
@group(0) @binding(1) var depth_samp: sampler;
@group(1) @binding(0) var scene_tex: texture_2d<f32>;
@group(1) @binding(1) var scene_samp: sampler;

@fragment
fn fs_ssr(in: VSOut) -> @location(0) vec4<f32> {
  let texSize = vec2<f32>(textureDimensions(lin_depth_tex));
  let px = 1.0 / texSize;
  let uv0 = in.uv;
  let zc = textureSampleLevel(lin_depth_tex, depth_samp, uv0, 0.0).x;
  if (zc <= 0.0 || zc >= 1e6) {
    return vec4<f32>(0.0);
  }
  // approximate normal from depth gradients
  let zx = textureSampleLevel(lin_depth_tex, depth_samp, uv0 + vec2<f32>(px.x, 0.0), 0.0).x - zc;
  let zy = textureSampleLevel(lin_depth_tex, depth_samp, uv0 + vec2<f32>(0.0, px.y), 0.0).x - zc;
  var n = normalize(vec3<f32>(-zx, -zy, 1.0));
  // view vector ~ forward in view space
  let v = vec3<f32>(0.0, 0.0, -1.0);
  let r = reflect(-v, n);
  // project reflection to screen step (heuristic scale)
  let step_uv = r.xy * 0.05; // tuned empirically
  var uv = uv0;
  var color = vec3<f32>(0.0);
  var found = 0.0;
  for (var i = 0u; i < 24u; i++) {
    uv += step_uv;
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0))) { break; }
    let zh = textureSampleLevel(lin_depth_tex, depth_samp, uv, 0.0).x;
    // hit if sample depth isn't much farther than start depth (thickness heuristic)
    if (zh - zc < 0.1) {
      color = textureSampleLevel(scene_tex, scene_samp, uv, 0.0).rgb;
      found = 1.0;
      break;
    }
  }
  // Fresnel-ish weight from normal/view
  let f = pow(1.0 - saturate(dot(n, -v)), 5.0);
  let strength = 0.35;
  return vec4<f32>(color * found * (f * strength), found * strength);
}

fn saturate(x: f32) -> f32 { return clamp(x, 0.0, 1.0); }
