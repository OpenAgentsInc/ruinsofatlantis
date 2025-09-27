// Placeholder SSR WGSL; full implementation will be added later.
// Provides entry point stubs so pipeline wiring can compile.

@compute @workgroup_size(8,8,1)
fn cs_ssr_dummy(@builtin(global_invocation_id) gid: vec3<u32>) {
  // no-op placeholder
}

