# Controls and Input Profiles

This document describes the current client controls, input profiles, and configuration.

Overview
- Default profile: third‑person MMO with camera orbit and auto‑face.
- Classic fallback: Classic Cursor — cursor visible by default; holding RMB engages temporary mouselook; releasing RMB restores the cursor.
- ALT behavior is configurable (toggle or hold‑to‑hold). In hold mode, pressing ALT releases the cursor and on release returns to mouselook.

Movement & Camera
- WASD — movement
  - A/D swing the camera left/right (orbit). The player auto‑faces the camera forward after a short delay.
  - Q/E are dedicated strafes (Q = left, E = right).
  - Basis: when RMB is held (or LMB+RMB chord), movement is relative to the camera’s forward; otherwise movement is relative to the character’s facing (yaw).
- Space — Jump (when grounded; short hop). Sprinting slightly speeds the jump start animation.
- Mouse Wheel — camera zoom.
- RMB drag — orbit the camera around the player while keeping the player as the orbit target.
  - Pointer‑lock is requested only while RMB is held; camera yaw/pitch update from mouse deltas.
  - Zoom and pitch limits are clamped to prevent flipping.
- Auto‑face (camera → character):
  - Normal: after rotating the camera, the character smoothly turns to face the camera’s forward after ~0.25 s (turn rate ≈ 180°/s).
  - While mouse‑look is active (RMB held): the delay is halved to ~0.125 s for a snappier feel.
  - Large swings: if the camera deviates by more than 90° from the character’s facing, the character begins turning immediately but trails just under the 90° threshold to ensure it exits “panic,” then, after the short delay, finishes turning to fully face forward.
- Strafing visuals: strafing uses a walk cadence for readability (not a sprint/jog).

Abilities & Actions
- 1 / 2 / 3 — cast bound spells (demo bindings). No other default cast keys.
- LMB / RMB — no at‑will actions; used only for look and LMB+RMB forward chord.

Autorun & Walk
- Num Lock — toggle autorun (pressing S cancels autorun).
- Numpad Divide — toggle walk/run.
- Shift — run modifier only applies when holding `W` without strafing/backpedaling.
  - Boosts forward speed by ~30% (tunable sprint multiplier).

Profiles & Cursor
- Profiles: `ActionCombat` or `ClassicCursor`.
- ALT: toggle or hold (configurable) to show/hide the cursor.

Configuration (optional)
- Path: `data/config/input_camera.toml`
- Keys:
  - `sensitivity_deg_per_count` (float)
  - `invert_y` (bool)
  - `min_pitch_deg` / `max_pitch_deg` (float degrees)
  - `alt_hold` (bool): true = hold‑to‑hold; false = toggle
  - `profile` (string): `ActionCombat` or `ClassicCursor`

Example
```
sensitivity_deg_per_count = 0.12
invert_y = false
min_pitch_deg = -75
max_pitch_deg = 75
alt_hold = true
profile = "ClassicCursor"
```

Environment overrides
- `MOUSE_SENS_DEG`, `INVERT_Y`, `MIN_PITCH_DEG`, `MAX_PITCH_DEG`, `ALT_HOLD`, `INPUT_PROFILE`

Logging
- Controller mode transitions are logged at `info` with target `controls`.
- Action bindings enqueue events are logged at `info` with a `debug` snapshot of pressed inputs.
- Set `RUST_LOG=info,client_core=info` (or configure `tracing` subscriber) to observe transitions; use `debug` for detailed snapshots.

Notes
- The client only emits `InputCommand` events. Server remains authoritative for gameplay.
- On platforms where pointer‑lock is denied (browser/OS), the client falls back to cursor mode automatically.
- Animation details for the PC rig (locomotion, jump, cast phases) are documented in `docs/graphics/pc_animations.md`.

