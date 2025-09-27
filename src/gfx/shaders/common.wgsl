// Shared shader definitions: bindings, globals, jitter, frame index, seeds.
// This file is intended for @include-like usage via host-side string concat.

struct Globals {
  view_proj: mat4x4<f32>,
  camRightTime: vec4<f32>,
  camUpPad: vec4<f32>,
  sunDirTime: vec4<f32>,
  sh: array<vec4<f32>, 9>,
  fog: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

struct Temporal {
  curr_jitter: vec2<f32>,
  prev_jitter: vec2<f32>,
  frame_index: u32,
  _pad: u32,
};

// Bind group suggestions (documented; concrete layouts defined in pipeline.rs):
// 0 = Globals (camera, jitter, exposure, TOD, frame index)
// 1 = Per-view history & Hi-Z
// 2 = Material/textures
// 3 = Pass-local resources (SSR/SSGI histories)

