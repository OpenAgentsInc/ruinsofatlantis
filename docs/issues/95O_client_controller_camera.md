# 95O — Client Controller & Camera in client_core

Labels: client, controls
Depends on: Epic #95, 95B (Scaffolds)

Intent
- Move player controller integration and camera follow into client_core systems; renderer only passes input and draws.

Files
- `crates/client_core/src/systems/controller.rs` (new)
- `crates/render_wgpu/src/gfx/renderer/update.rs` — remove `apply_pc_transform` math; call into client_core

Tasks
- [ ] Implement `update(dt, &InputState, &mut Transform, &TerrainCpu)` in client_core.
- [ ] Keep winit plumbing in renderer `input.rs`; propagate yaw/zoom to controller/camera.
- [ ] Controller speeds/yaw rates from `data_runtime` client config.

Acceptance
- No transform/camera math in renderer; visuals unchanged.
