// Shared fullscreen triangle vertex shaders.

struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

// Offscreen passes (no Y flip)
@vertex
fn vs_fullscreen_noflip(@builtin(vertex_index) vid: u32) -> VSOut {
  var p = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 3.0, -1.0),
    vec2<f32>(-1.0,  3.0)
  );
  var out: VSOut;
  out.pos = vec4<f32>(p[vid], 0.0, 1.0);
  out.uv  = 0.5 * (p[vid] + vec2<f32>(1.0, 1.0));
  return out;
}

// Present to swapchain (flip Y exactly once)
@vertex
fn vs_fullscreen_present_flip(@builtin(vertex_index) vid: u32) -> VSOut {
  var p = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 3.0, -1.0),
    vec2<f32>(-1.0,  3.0)
  );
  var out: VSOut;
  out.pos = vec4<f32>(p[vid], 0.0, 1.0);
  out.uv  = vec2<f32>(0.5 * (p[vid].x + 1.0), 0.5 * (1.0 - p[vid].y));
  return out;
}

