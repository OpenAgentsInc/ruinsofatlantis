// Bloom: sample scene color with a small 9‑tap blur around the current pixel,
// apply a soft knee threshold, and additively blend into the destination.
// The pass reads from SceneRead and writes into SceneColor (via pipeline.rs),
// adding a subtle highlight around bright emissive content (e.g., fire bolts).

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@fragment
fn fs_bloom(in: VsOut) -> @location(0) vec4<f32> {
  let sz = vec2<f32>(textureDimensions(scene_tex));
  let texel = 1.0 / sz;
  let uv = in.uv;
  // 9-tap kernel (approx Gaussian)
  let offs = array<vec2<f32>, 9>(
    vec2<f32>(-1.0, -1.0), vec2<f32>(0.0, -1.0), vec2<f32>(1.0, -1.0),
    vec2<f32>(-1.0,  0.0), vec2<f32>(0.0,  0.0), vec2<f32>(1.0,  0.0),
    vec2<f32>(-1.0,  1.0), vec2<f32>(0.0,  1.0), vec2<f32>(1.0,  1.0));
  let w = array<f32, 9>(1.0, 2.0, 1.0, 2.0, 4.0, 2.0, 1.0, 2.0, 1.0);
  var sum = vec3<f32>(0.0);
  var norm = 0.0;
  for (var i: i32 = 0; i < 9; i++) {
    let p = uv + offs[i] * texel * 1.5; // small radius
    let c = textureSample(scene_tex, samp, p).rgb;
    sum += c * w[i];
    norm += w[i];
  }
  let blur = sum / max(norm, 1e-5);
  // soft knee threshold
  let thr = 1.0; // linear HDR unit threshold
  let k = 2.0;  // knee sharpness
  let lum = dot(blur, vec3<f32>(0.2126, 0.7152, 0.0722));
  let t = smoothstep(thr - 0.5 / k, thr + 0.5 / k, lum);
  let bloom = blur * t * 0.35; // intensity
  return vec4<f32>(bloom, 0.0);
}
