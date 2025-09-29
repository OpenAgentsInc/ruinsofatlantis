# Platform & Tools Audit

Platform (`crates/platform_winit`)
- Clean usage of winit 0.30 `ApplicationHandler` API; headless detection guards interactive runs (`crates/platform_winit/src/lib.rs:78`).
- Logging and error handling are appropriate for a prototype.

Recommendations
- Separate input mapping from renderer by emitting a typed input state to the app; renderer should consume derived inputs only.
- Add CLI flags parsing at the shell/app level; renderer only reads toggles from injected config.
- For robustness, detect and log adapter/surface selection details and requested vs actual formats.

Tools
- `tools/model-viewer`: good for asset inspection; consider extracting shared render bootstrap to a small `ra_viewer` helper if logic grows.
- `tools/zone-bake`: deterministic metadata with fingerprints; add golden test on `zone_meta.json`.
- `tools/sim-harness`: useful for headless scenario runs; wire into CI with a sample scenario and assert end-state invariants.
- `tools/gltf-decompress` and `shared/assets`: clarify the single source of truth for Draco handling in docs and ensure CLI is used in CI for any new models.

Recommendations (Tools)
- Add `--schema` flags to lint JSON content locally.
- Expose `xtask bake-zone --slug <slug>` in `README.md` with expected artifacts.
- Add smoke test commands for each tool in `xtask ci` (non-interactive, headless paths only).

