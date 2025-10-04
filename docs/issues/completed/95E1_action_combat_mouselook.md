# 95E1 — Action Combat & Mouselook (Neverwinter‑style default)

Status: COMPLETE (Neverwinter-style controls implemented; tests added)

Labels: client, input, camera, UX, accessibility
Depends on: 95B (client_core scaffold), 95E (controller/camera systems)
Type: Feature (supplements 95E; does not replace)

Intent
- Adopt a Neverwinter‑style action combat baseline: persistent mouselook with a center reticle, LMB/RMB for at‑will powers, ALT to free the cursor for UI (toggle or hold), and class actions on Shift/Tab — while providing optional alternate schemes.

Scope & Outcomes
- Default mode (“Action/Reticle”): pointer‑lock by default, center reticle visible.
- LMB/RMB fire at‑wills; Q/E/R trigger encounters; Shift for dodge/guard; Tab for class mechanic.
- ALT toggles or holds cursor (configurable via `input_camera.toml`).
- Classic Cursor profile: RMB hold engages temporary look; release returns to cursor.
- Client emits `InputCommand`s only; no client gameplay mutation.

Design (implemented)
- client_core systems: mouselook (yaw/pitch clamped), cursor (toggle/hold), action_bindings (map buttons → InputCommand), camera rig helper.
- renderer host: forwards key/mouse to client_core systems; applies pointer lock; draws reticle when in mouselook.
- data_runtime config: `config/input_camera.toml` for sensitivity, invert_y, pitch clamps, alt_hold; env overrides supported.

Test Coverage
- `client_core/tests/mouselook.rs`: pitch clamp, invert‑Y, yaw accumulation.
- `client_core/tests/cursor.rs`: ALT toggle pointer‑lock + Classic RMB hold capture/release; ALT hold press/release.
- `client_core/tests/camera.rs`: third‑person boom geometry sanity.
- `client_core` action_bindings module tests: verifies command emission (LMB, Q, Shift; RMB case).
- `data_runtime/tests/input_camera_cfg.rs`: env overrides parse including `ALT_HOLD`.

Notes
- The renderer mirrors controller yaw/pitch into existing orbit fields to avoid broader refactors.
- Pointer‑lock denial falls back to cursor mode and keeps UI interactive (desktop/WASM).

