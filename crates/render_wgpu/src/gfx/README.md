This directory contains the migrated renderer modules from the original root `src/gfx/`.
Internal paths are preserved (modules live under `crate::gfx::...`).

Modules overview (selected):
- camera.rs — camera type and view/projection helpers
- camera_sys.rs — camera system integration and movement
- pipeline.rs — pipelines and bind‑group layouts
- mesh.rs — CPU mesh helpers (cube/plane)
- terrain.rs — CPU terrain + tree scattering and snapshot I/O
- foliage.rs — tree instancing: builds transforms, loads tree GLTF, uploads buffers
- rocks.rs — rock instancing: loads `assets/models/rock.glb`, scatters, uploads buffers
- ruins.rs — ruins GLTF upload + base offset/radius metrics for placement
- castle.rs — castle GLB upload + base offset/radius metrics for placement
- scene.rs — demo scene assembly (wizards/ruins placement, single distant castle)
- ui.rs — HUD/nameplates/bars atlases and draw helpers
