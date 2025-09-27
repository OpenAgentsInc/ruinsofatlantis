// Compute Hi-Z pyramid from depth.

struct Params { znear: f32, zfar: f32, _pad: vec2<f32> };

// Mip0 linearization: depth (0..1) -> linear view-space depth
@group(0) @binding(0) var depth_tex: texture_depth_2d;
@group(0) @binding(1) var depth_samp: sampler; // unused in compute, kept for layout stability
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var dst0: texture_storage_2d<r32float, write>;

fn linearize(d: f32, znear: f32, zfar: f32) -> f32 {
  // OpenGL-style projection depth linearization
  return (2.0 * znear) / (zfar + znear - d * (zfar - znear));
}

@compute @workgroup_size(8, 8, 1)
fn cs_linearize_mip0(@builtin(global_invocation_id) gid: vec3<u32>) {
  let size = textureDimensions(dst0);
  if (gid.x >= u32(size.x) || gid.y >= u32(size.y)) { return; }
  let coord = vec2<i32>(gid.xy);
  let d = textureLoad(depth_tex, coord, 0);
  let z = linearize(d, params.znear, params.zfar);
  textureStore(dst0, vec2<i32>(gid.xy), vec4<f32>(z, 0.0, 0.0, 0.0));
}

// Downsample: dst = max over 2x2 of src
@group(0) @binding(0) var src_mip: texture_2d<f32>;
@group(0) @binding(1) var dst_mip: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8, 1)
fn cs_downsample_max(@builtin(global_invocation_id) gid: vec3<u32>) {
  let sz = textureDimensions(dst_mip);
  if (gid.x >= u32(sz.x) || gid.y >= u32(sz.y)) { return; }
  let base = vec2<i32>(gid.xy) * 2;
  let s00 = textureLoad(src_mip, base + vec2<i32>(0, 0), 0).x;
  let s10 = textureLoad(src_mip, base + vec2<i32>(1, 0), 0).x;
  let s01 = textureLoad(src_mip, base + vec2<i32>(0, 1), 0).x;
  let s11 = textureLoad(src_mip, base + vec2<i32>(1, 1), 0).x;
  let z = max(max(s00, s10), max(s01, s11));
  textureStore(dst_mip, vec2<i32>(gid.xy), vec4<f32>(z, 0.0, 0.0, 0.0));
}
