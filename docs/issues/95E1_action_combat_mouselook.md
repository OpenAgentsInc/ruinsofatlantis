# 95E1 — Action Combat & Mouselook (Neverwinter‑style default)

Labels: client, input, camera, UX, accessibility
Depends on: 95B (client_core scaffold), 95E (controller/camera systems)
Type: Feature (supplements 95E; does not replace)

## Intent

Adopt a Neverwinter‑style action combat baseline: persistent mouselook with a center reticle, LMB/RMB for at‑will powers, ALT to free the cursor for UI, and class actions on Shift/Tab — while providing optional alternate schemes.

Research basis:
Neverwinter preview controls documented reticle targeting with constant mouselook; At‑Will on LMB/RMB; Encounter on Q/E (and sometimes R); Shift for mobility/guard; Tab for class mechanic. ALT toggles cursor for UI.

---

## Scope & Outcomes

Default mode (“Action/Reticle”):
- Mouse is locked (pointer‑lock) while in gameplay; center reticle visible.
- LMB / RMB fire the current At‑Will abilities.
- Q/E/R trigger Encounter abilities (configurable per class/spec).
- Shift: class mobility/guard (dodge, blink, guard, etc.).
- Tab: class stance/mechanic (class‑specific ability bar).
- ALT: toggle (or hold‑to‑hold) Cursor Mode — unlock cursor, hide reticle; UI is interactive; gameplay inputs suppressed except movement.
- Casting while moving supported by default; LoS is authoritative; “out of range” minimized (use reticle + range feedback, not click‑targeting).

Optional modes (user‑selectable):
- Classic Cursor: cursor default; hold RMB → mouselook + reticle; LMB remains UI click unless in mouselook.
- Toggle vs Hold for ALT (cursor unlock).
- Accessibility: jump‑to‑target assist (cone/aim assist threshold), enlarged reticle, high‑contrast reticle, reduced camera sway.

Networking / ECS (ties to 95E & Phase 2):
Client never mutates gameplay; it emits InputCommand events (AtWillLMB/AtWillRMB, Encounter(Q/E/R), Dodge, ClassMechanic, CursorToggle, LookDelta). Server performs validation, GCD, LoS/range, and authoritative results.

---

## Design

Client state machine (client_core)
- MouseMode: { Mouselook, Cursor }
- CombatMode: { Idle, Casting, Releasing } (for charge‑ups/channeling)
- InputMapping (profile’d): ActionReticleDefault, ClassicCursor, Custom
- ControllerState:
  - yaw, pitch (quats)
  - reticle_world (ray from camera)
  - flags: is_sprinting, is_dodging, is_guarding, in_cursor_mode

Systems
- MouselookSystem: consumes raw mouse deltas when MouseMode::Mouselook and produces yaw/pitch deltas; integrates dead‑zone clamp + sensitivity.
- CursorToggleSystem: ALT press/release transitions; pointer‑lock acquire/release (Desktop: winit; WASM: Pointer Lock API).
- CameraFollowSystem: applies yaw/pitch to camera rig, optional recoil/sway.
- ReticleSystem: screen‑space reticle draw; maps to world ray (Later used for LoS feedback). No gameplay mutation here; LoS feedback can be hints only.
- ActionBindingsSystem: map LMB/RMB/Q/E/R/Shift/Tab to InputCommand; gating by cooldown/GCD is server‑driven (optional client hints).

Renderer integration (render_wgpu)
- Host only: forwards WindowEvent to client_core adapter; applies pointer‑lock per client_core HostEvent.
- Draws reticle and aim hints from client_core facade; does not compute controller math.

Settings & Profiles
- data/config/input_camera.toml with sensitivity, invert‑y, pitch clamps, ALT behavior, input profile.
- In‑game settings pane: profile switch, ALT hold/toggle, sensitivity/FOV, reticle style.

Telemetry (see 95T)
- Counters: controller.mode_transitions{from,to}, cursor_toggle{hold,toggle}, cast_input vs cast_authorized.
- Timers: client input → server ack (later via replication).

Acceptance Criteria
- Default (Action/Reticle): pointer‑lock on spawn; reticle visible; ALT toggles cursor; LMB/RMB fire at‑wills; Q/E/R, Shift, Tab emit commands; Desktop/WASM parity with Classic fallback if lock fails.
- Options: Classic profile; ALT hold/toggle; persisted sensitivity/FOV.

References
- Community and docs describing Neverwinter’s controls patterns (reticle, LMB/RMB, Q/E, Shift/Tab, ALT for cursor).

