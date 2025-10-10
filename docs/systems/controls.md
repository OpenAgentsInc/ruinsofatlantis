# Controls and Input Profiles

This document describes the current client controls, input profiles, and configuration.

Overview
- Default profile: Action/Reticle — persistent mouselook with a center reticle.
- Classic fallback: Classic Cursor — cursor visible by default; holding RMB engages temporary mouselook; releasing RMB restores the cursor.
- ALT behavior is configurable (toggle or hold‑to‑hold). In hold mode, pressing ALT releases the cursor and on release returns to mouselook.

Movement & Camera
- WASD — movement
  - RMB held: A/D become strafes; W/S move relative to camera forward.
  - RMB released: A/D turn the character; W/S move relative to the character’s facing (no camera drift).
- Q/E — dedicated strafes (Q = left, E = right) regardless of RMB.
- Space — Jump (when grounded; WoW-like short hop)
- Mouse Wheel — camera zoom.
- RMB drag — orbit the camera around the player while keeping the player as the orbit target.
  - Pointer‑lock is requested only while RMB is held; camera yaw/pitch update from mouse deltas.
  - Zoom and pitch limits are clamped to prevent flipping.

Abilities & Actions
- LMB / RMB — primary/secondary actions (emits `InputCommand::AtWillLMB/AtWillRMB`).
- Q/E/R — encounter actions (when bound); Shift — dodge/guard; Tab — class mechanic.

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
