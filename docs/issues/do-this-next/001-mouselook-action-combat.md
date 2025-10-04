# Client Controller: Mouselook & Action Combat

> Thanks now given this structure where should I put the character controller customization like I want to toggle mouse look on and off and just evolve the character controller like where should all of that go and how should it be structured?

Great question. Short answer: **put all character‑controller logic—including mouse‑look mode toggling—inside `client_core` as ECS systems and components**, and treat the renderer (`render_wgpu`) as a **thin host** that forwards raw OS input and performs **only** OS/window‑level side effects (cursor capture/visibility). That keeps gameplay/PvE logic testable and makes it easy to evolve without tangling it with graphics.

Below is a concrete structure and a ready‑to‑paste GitHub issue to implement it.

---

## Where things should live

### 1) `client_core` (owns controller, modes, camera rig, prediction)

**Responsibilities**

* Map raw input → high‑level **Actions**.
* Maintain **controller mode** (MouseLook vs UI) and emit **pointer‑lock requests** to the host.
* Compute **MovementIntent** (move vector, sprint, jump), **LookDelta** (yaw/pitch deltas), and **CameraRig** updates.
* Drive **local prediction** of the player transform (optional), and generate **InputCommand** messages for the server.

**Components (ECS)**

* `LocalPlayer` (marker)
* `ControllerParams { walk_speed, run_mult, mouse_sens, invert_y, pitch_min, pitch_max }`
* `ControllerMode { mouselook: bool }`
* `MovementIntent { move: Vec2, sprint: bool, jump: bool }`
* `LookDelta { yaw: f32, pitch: f32 }` (cleared each frame after consumption)
* `CameraRig { yaw, pitch, boom_len, boom_offset, mode: ThirdPerson/First/Free }`
* `TransformPredicted { pos: Vec3, rot: Quat }` (client‑side only)
* `UiFocus { chat_open: bool, menu_open: bool }` (gates mouse capture)

**Systems**

* `InputCollectSystem` (pure): build `InputState` (pressed/just_pressed, analog axes, mouse delta).
* `ModeToggleSystem`: toggles `ControllerMode.mouselook` on keybind (e.g., `Alt`/`Esc`/`~`) **unless** `UiFocus` blocks it; emits `PointerLockRequest(want: bool)`.
* `ControllerSystem`: consumes `InputState`, writes `MovementIntent` + `LookDelta` (respecting `ControllerMode`).
* `CameraSystem`: integrates `CameraRig` yaw/pitch from `LookDelta` and clamps pitch; computes camera target for renderer.
* (Optional) `PredictionSystem`: integrates `TransformPredicted` using `MovementIntent` (client‐only), later reconciled from server.

**Bridges (host <-> core)**

* `client_core::input::winit_adapter`: translate `winit` events to `InputState` deltas.
* `client_core::host_api::CursorApi` (trait): `set_pointer_lock(bool)`, `set_cursor_visible(bool)`.

  * Renderer implements this using `winit::window::Window`.

### 2) `render_wgpu` (host for OS input & cursor)

**Responsibilities**

* Forward `winit::event::WindowEvent` to `client_core::input::winit_adapter`.
* Apply **pointer lock/visibility** when `client_core` emits `PointerLockRequest`.
* Read **camera target** (position/forward/up) and **TransformPredicted** to place the camera and the player mesh.
* **Does not** compute gameplay; no controller math here.

### 3) Server authority compatibility

* `client_core` turns `MovementIntent`/`LookDelta` into **InputCommand** messages; server integrates authoritative movement and replicates `Transform`.
* Client **optionally** predicts `TransformPredicted` and blends to server state when snapshots arrive.

---

## Data flow (one frame)

```
winit::WindowEvent
   └─ render_wgpu::input.rs → client_core::input::winit_adapter
        └─ client_core::InputState (pressed, axis, mouse_delta)

client_core systems (fixed/frame):
   InputCollectSystem  → updates InputState
   ModeToggleSystem    → may emit PointerLockRequest(true/false)
   ControllerSystem    → MovementIntent + LookDelta
   CameraSystem        → CameraRig (yaw/pitch/boom) → CameraTarget
   (optional) PredictionSystem → TransformPredicted

render_wgpu:
   apply CursorApi when PointerLockRequest received
   read CameraTarget + TransformPredicted → set camera & draw
```

---

## Minimal API sketch

**`client_core/src/modes.rs`**

```rust
#[derive(Clone, Copy, Debug, Default)]
pub struct ControllerMode { pub mouselook: bool }

impl ControllerMode {
    pub fn toggle(&mut self) { self.mouselook = !self.mouselook; }
}
```

**`client_core/src/input/state.rs`**

```rust
#[derive(Default, Clone, Debug)]
pub struct InputState {
    pub move_axis: glam::Vec2,     // WASD → [-1,1]^2
    pub mouse_delta: glam::Vec2,   // pixels or normalized
    pub sprint: bool,
    pub jump: bool,
    pub toggle_mouselook_just_pressed: bool,
    pub rmb_held: bool, // optional: temp capture while held
}
```

**`client_core/src/systems/controller.rs`**

```rust
pub fn controller_system(
    dt: f32,
    input: &InputState,
    mode: &ControllerMode,
    params: &ControllerParams,
    intent: &mut MovementIntent,
    look: &mut LookDelta,
) {
    intent.move_ = input.move_axis;
    intent.sprint = input.sprint;
    intent.jump   = input.jump;

    if mode.mouselook || input.rmb_held {
        let sens = params.mouse_sens * dt; // or scale by dpi, etc.
        look.yaw   += input.mouse_delta.x * sens;
        look.pitch -= input.mouse_delta.y * sens * if params.invert_y { -1.0 } else { 1.0 };
        look.pitch = look.pitch.clamp(params.pitch_min, params.pitch_max);
    }
}
```

**`client_core/src/systems/mode_toggle.rs`**

```rust
pub enum HostEvent { PointerLockRequest(bool) }

pub fn mode_toggle_system(
    input: &InputState,
    ui: &UiFocus,
    mode: &mut ControllerMode,
    out_host_events: &mut Vec<HostEvent>,
) {
    if input.toggle_mouselook_just_pressed && !ui.chat_open && !ui.menu_open {
        mode.toggle();
        out_host_events.push(HostEvent::PointerLockRequest(mode.mouselook));
    }
}
```

**Renderer host applying lock (winit)**

```rust
impl CursorApi for Renderer {
    fn set_pointer_lock(&mut self, want: bool) {
        use winit::window::CursorGrabMode;
        let _ = self.window.set_cursor_grab(if want {
            CursorGrabMode::Locked
        } else {
            CursorGrabMode::None
        });
        self.window.set_cursor_visible(!want);
    }
}
```

---

## Practical tips

* **Right‑mouse hold to capture**: even when `mouselook=false`, holding RMB can temporarily capture for camera orbit; release restores cursor.
* **UI focus wins**: any UI that requires a cursor (chat, inventory) must force `mouselook=false` and release pointer lock; restore when closed.
* **Configurable binds/params**: keep keybinds and controller params in user config (JSON/TOML via `data_runtime`) so you can tune without code changes.
* **Prediction toggles**: start with no prediction for simplicity; add `TransformPredicted` later, blending toward authoritative `Transform` with a short time constant.

---

# 📌 GitHub Issue

**Title:** 95E — Client Character Controller & Mouse‑Look (client_core ownership)

**Labels:** client, input, ecs, ui/ux

**Depends on:** 95B (Scaffolds), 95A (Feature gates)

---

## Intent

Move all character controller logic (including mouse‑look on/off) into `client_core` as ECS components + systems. The renderer remains a thin host for OS input and cursor capture. This keeps gameplay modular and server‑friendly while letting us iterate fast on feel.

## Outcomes

* Mouse‑look toggle works (toggle key + RMB hold behavior).
* Controller math (move vector, yaw/pitch) lives in `client_core`.
* Renderer only forwards input and applies pointer‑lock visibility on request.
* Camera rig updated via ECS; renderer reads a simple target.
* Unit tests cover the mode state machine and pitch clamp.

## File plan

**New (client_core)**

* `crates/client_core/src/input/{mod.rs,state.rs,bindings.rs,winit_adapter.rs}`
* `crates/client_core/src/modes.rs`
* `crates/client_core/src/components.rs`

  * `ControllerParams`, `ControllerMode`, `MovementIntent`, `LookDelta`, `CameraRig`, `TransformPredicted`, `UiFocus`, `LocalPlayer`
* `crates/client_core/src/systems/{controller.rs,mode_toggle.rs,camera.rs,prediction.rs}`
* `crates/client_core/src/host_api.rs` (trait `CursorApi`)
* `crates/client_core/tests/{mode_toggle.rs,camera_pitch.rs}`

**Touch (render_wgpu)**

* `crates/render_wgpu/src/gfx/renderer/input.rs`: forward `winit` events → `client_core::input::winit_adapter`.
* `crates/render_wgpu/src/gfx/renderer/update.rs`: poll `HostEvent::PointerLockRequest` and call `CursorApi`.
* `crates/render_wgpu/src/gfx/renderer/camera.rs` (or existing camera spot): read `CameraRig` target from `client_core`.

## Tasks

* [ ] **Components**: define `ControllerParams`, `ControllerMode`, `MovementIntent`, `LookDelta`, `CameraRig`, `TransformPredicted`, `UiFocus`, `LocalPlayer`.
* [ ] **Input adapter**: implement `winit_adapter` translating `WindowEvent`/`DeviceEvent` to `InputState` (keys → axes, mouse delta accumulation).
* [ ] **Mode toggle system**:

  * Toggle on key (configurable; default `Alt` or `~`).
  * Temporary capture while RMB held.
  * Respect `UiFocus` (chat/menu forces mouselook off).
  * Emit `HostEvent::PointerLockRequest(bool)`.
* [ ] **Controller system**: compute `MovementIntent` + `LookDelta` when in mouselook; scale by `ControllerParams.mouse_sens`, clamp pitch.
* [ ] **Camera system**: integrate `CameraRig` yaw/pitch; provide a function to return `CameraTarget { eye, at, up }`.
* [ ] **Renderer bridge**:

  * Implement `CursorApi` using `winit::window::Window`.
  * On host event, call `set_cursor_grab` + `set_cursor_visible`.
  * Read `CameraTarget` each frame to place the camera.
* [ ] **Config/data**:

  * Add `controller.toml` (or extend existing user config) with defaults: speeds, sensitivity, invert_y, pitch clamp, binds.
  * Loader in `data_runtime` (optional in this PR, or stub values).
* [ ] **Tests**:

  * `mode_toggle.rs`: open/close UI, toggle key, RMB hold; assert pointer‑lock requests sequence.
  * `camera_pitch.rs`: ensure pitch clamp works across large mouse deltas.

## Acceptance criteria

* With UI closed, pressing the toggle key switches between **MouseLook ON** (cursor hidden, pointer locked) and **MouseLook OFF** (cursor visible).
* Holding **RMB** temporarily engages camera look even when `MouseLook OFF`; releasing RMB restores the previous mode.
* `ControllerSystem` produces stable `MovementIntent` and `LookDelta` (zero when not in look mode).
* Renderer shows correct camera behavior; no direct gameplay math lives in `render_wgpu`.
* Unit tests pass; `cargo clippy -D warnings` is clean.

## Nice‑to‑have (follow‑ups)

* Rebindable keys UI.
* Save/load controller settings per user.
* Optional client‑side **prediction** with reconciliation from server transforms.
* First‑person camera option with weapon/hand bob toggle.

---

If you want, I can also draft the small `CursorApi` glue and the `winit_adapter` skeleton so an agent can start coding immediately.
