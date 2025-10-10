//! Input and window event handling extracted from gfx/mod.rs

use winit::event::WindowEvent;
use winit::keyboard::{KeyCode, PhysicalKey};

use super::update::wrap_angle;
use crate::gfx::Renderer;

impl Renderer {
    /// Handle platform window events that affect input (keyboard focus/keys).
    pub fn handle_window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state.is_pressed();
                match event.physical_key {
                    // Cursor toggle/hold (ALT)
                    PhysicalKey::Code(KeyCode::AltLeft) | PhysicalKey::Code(KeyCode::AltRight) => {
                        let mut host_events = Vec::new();
                        let ui = client_core::systems::cursor::UiFocus::default();
                        if self.controller_alt_hold {
                            client_core::systems::cursor::handle_cursor_event(
                                &mut self.controller_state,
                                &ui,
                                client_core::systems::cursor::CursorEvent::Hold(pressed),
                                &mut host_events,
                            );
                        } else if pressed {
                            client_core::systems::cursor::handle_cursor_event(
                                &mut self.controller_state,
                                &ui,
                                client_core::systems::cursor::CursorEvent::Toggle,
                                &mut host_events,
                            );
                        }
                        for ev in host_events {
                            let client_core::systems::cursor::HostEvent::PointerLockRequest(b) = ev;
                            self.pointer_lock_request = Some(b);
                        }
                    }
                    // Ignore movement/casting inputs if the PC is dead
                    PhysicalKey::Code(KeyCode::KeyW) if self.pc_alive => {
                        self.input.forward = pressed
                    }
                    PhysicalKey::Code(KeyCode::KeyS) if self.pc_alive => {
                        self.input.backward = pressed;
                        if pressed {
                            self.scene_inputs.cancel_autorun();
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyA) if self.pc_alive => {
                        if self.rmb_down {
                            self.input.strafe_left = pressed;
                        } else {
                            self.input.turn_left = pressed;
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyD) if self.pc_alive => {
                        if self.rmb_down {
                            self.input.strafe_right = pressed;
                        } else {
                            self.input.turn_right = pressed;
                        }
                    }
                    PhysicalKey::Code(KeyCode::ShiftLeft)
                    | PhysicalKey::Code(KeyCode::ShiftRight)
                        if self.pc_alive =>
                    {
                        self.input.run = pressed
                    }
                    PhysicalKey::Code(KeyCode::Digit1) | PhysicalKey::Code(KeyCode::Numpad1)
                        if self.pc_alive =>
                    {
                        if pressed {
                            // Gate by cooldown via client_runtime ability state
                            let spell_id = "wiz.fire_bolt.srd521";
                            if self.scene_inputs.can_cast(spell_id, self.last_time) {
                                self.pc_cast_queued = true;
                                self.pc_cast_kind = Some(super::super::PcCast::FireBolt);
                                self.pc_cast_time = 0.0; // instant
                                log::info!("input: key 1 → queue Fire Bolt");
                                if let Some(tx) = &self.cmd_tx {
                                    // Use the character's facing (controller yaw), not camera forward.
                                    let yaw = self.scene_inputs.yaw();
                                    let fwd = glam::vec3(yaw.sin(), 0.0, yaw.cos());
                                    let p = self.scene_inputs.pos();
                                    let (h, _n) =
                                        crate::gfx::terrain::height_at(&self.terrain_cpu, p.x, p.z);
                                    // Chest-ish origin, nudged forward slightly
                                    let pos = glam::vec3(p.x, h + 1.4, p.z) + fwd * 0.25;
                                    let cmd = net_core::command::ClientCmd::FireBolt {
                                        pos: [pos.x, pos.y, pos.z],
                                        dir: [fwd.x, fwd.y, fwd.z],
                                    };
                                    let mut payload = Vec::new();
                                    cmd.encode(&mut payload);
                                    let mut framed = Vec::with_capacity(payload.len() + 8);
                                    net_core::frame::write_msg(&mut framed, &payload);
                                    let _ = tx.try_send(framed);
                                }
                            } else {
                                log::info!(
                                    "input: Fire Bolt cooldown {:.0} ms remaining",
                                    ((self.scene_inputs.cooldown_frac(
                                        spell_id,
                                        self.last_time,
                                        self.firebolt_cd_dur,
                                    ) * self.firebolt_cd_dur)
                                        * 1000.0)
                                        .max(0.0)
                                );
                            }
                        }
                    }
                    PhysicalKey::Code(KeyCode::Digit2) | PhysicalKey::Code(KeyCode::Numpad2)
                        if self.pc_alive =>
                    {
                        if pressed {
                            // Gate by cooldown via client_runtime ability state
                            let spell_id = "wiz.magic_missile.srd521";
                            if self.scene_inputs.can_cast(spell_id, self.last_time) {
                                self.pc_cast_queued = true;
                                self.pc_cast_kind = Some(super::super::PcCast::MagicMissile);
                                // Use SpecDb-provided cast time captured at init
                                self.pc_cast_time = self.magic_missile_cast_time.max(0.0);
                                log::info!(
                                    "input: key 2 → queue Magic Missile (cast={:.2}s)",
                                    self.magic_missile_cast_time
                                );
                                if let Some(tx) = &self.cmd_tx {
                                    let yaw = self.scene_inputs.yaw();
                                    let fwd = glam::vec3(yaw.sin(), 0.0, yaw.cos());
                                    let p = self.scene_inputs.pos();
                                    let (h, _n) =
                                        crate::gfx::terrain::height_at(&self.terrain_cpu, p.x, p.z);
                                    let pos = glam::vec3(p.x, h + 1.4, p.z) + fwd * 0.25;
                                    let cmd = net_core::command::ClientCmd::MagicMissile {
                                        pos: [pos.x, pos.y, pos.z],
                                        dir: [fwd.x, fwd.y, fwd.z],
                                    };
                                    let mut payload = Vec::new();
                                    cmd.encode(&mut payload);
                                    let mut framed = Vec::with_capacity(payload.len() + 8);
                                    net_core::frame::write_msg(&mut framed, &payload);
                                    let _ = tx.try_send(framed);
                                }
                            } else {
                                log::info!(
                                    "input: Magic Missile cooldown {:.0} ms remaining",
                                    ((self.scene_inputs.cooldown_frac(
                                        spell_id,
                                        self.last_time,
                                        self.magic_missile_cd_dur,
                                    ) * self.magic_missile_cd_dur)
                                        * 1000.0)
                                        .max(0.0)
                                );
                            }
                        }
                    }
                    PhysicalKey::Code(KeyCode::Digit3) | PhysicalKey::Code(KeyCode::Numpad3)
                        if self.pc_alive =>
                    {
                        if pressed {
                            let spell_id = "wiz.fireball.srd521";
                            if self.scene_inputs.can_cast(spell_id, self.last_time) {
                                self.pc_cast_queued = true;
                                self.pc_cast_kind = Some(super::super::PcCast::Fireball);
                                self.pc_cast_time = self.fireball_cast_time.max(0.0);
                                log::info!(
                                    "input: key 3 → queue Fireball (cast={:.2}s)",
                                    self.fireball_cast_time
                                );
                                if let Some(tx) = &self.cmd_tx {
                                    let yaw = self.scene_inputs.yaw();
                                    let fwd = glam::vec3(yaw.sin(), 0.0, yaw.cos());
                                    let p = self.scene_inputs.pos();
                                    let (h, _n) =
                                        crate::gfx::terrain::height_at(&self.terrain_cpu, p.x, p.z);
                                    let pos = glam::vec3(p.x, h + 1.4, p.z) + fwd * 0.25;
                                    let cmd = net_core::command::ClientCmd::Fireball {
                                        pos: [pos.x, pos.y, pos.z],
                                        dir: [fwd.x, fwd.y, fwd.z],
                                    };
                                    let mut payload = Vec::new();
                                    cmd.encode(&mut payload);
                                    let mut framed = Vec::with_capacity(payload.len() + 8);
                                    net_core::frame::write_msg(&mut framed, &payload);
                                    let _ = tx.try_send(framed);
                                }
                            } else {
                                log::info!(
                                    "input: Fireball cooldown {:.0} ms remaining",
                                    ((self.scene_inputs.cooldown_frac(
                                        spell_id,
                                        self.last_time,
                                        self.fireball_cd_dur,
                                    ) * self.fireball_cd_dur)
                                        * 1000.0)
                                        .max(0.0)
                                );
                            }
                        }
                    }
                    // Neverwinter-style action bindings: Q/E/R, Shift, Tab
                    PhysicalKey::Code(KeyCode::KeyQ)
                    | PhysicalKey::Code(KeyCode::KeyE)
                    | PhysicalKey::Code(KeyCode::KeyR)
                    | PhysicalKey::Code(KeyCode::ShiftLeft)
                    | PhysicalKey::Code(KeyCode::ShiftRight)
                    | PhysicalKey::Code(KeyCode::Tab) => {
                        if pressed {
                            // If dead and the user presses R, respawn instead of enqueuing an Encounter
                            if !self.pc_alive
                                && matches!(event.physical_key, PhysicalKey::Code(KeyCode::KeyR))
                            {
                                log::info!("Respawn via R key");
                                self.respawn();
                                return;
                            }
                            use client_core::systems::action_bindings::{Bindings, ButtonSnapshot};
                            let mut snap = ButtonSnapshot::default();
                            match event.physical_key {
                                PhysicalKey::Code(KeyCode::KeyQ) => snap.q_pressed = true,
                                PhysicalKey::Code(KeyCode::KeyE) => snap.e_pressed = true,
                                PhysicalKey::Code(KeyCode::KeyR) => snap.r_pressed = true,
                                PhysicalKey::Code(KeyCode::ShiftLeft)
                                | PhysicalKey::Code(KeyCode::ShiftRight) => {
                                    snap.shift_pressed = true
                                }
                                PhysicalKey::Code(KeyCode::Tab) => snap.tab_pressed = true,
                                _ => {}
                            }
                            let binds = Bindings::default();
                            client_core::systems::action_bindings::handle_buttons(
                                &binds,
                                &self.controller_state,
                                &snap,
                                &mut self.input_queue,
                            );
                        }
                    }
                    // Sky controls (pause/scrub/speed)
                    PhysicalKey::Code(KeyCode::Space) => {
                        if pressed {
                            self.sky.toggle_pause();
                        }
                    }

                    PhysicalKey::Code(KeyCode::BracketLeft) => {
                        if pressed {
                            self.sky.scrub(-0.01);
                        }
                    }
                    PhysicalKey::Code(KeyCode::BracketRight) => {
                        if pressed {
                            self.sky.scrub(0.01);
                        }
                    }
                    PhysicalKey::Code(KeyCode::Minus) => {
                        if pressed {
                            self.sky.speed_mul(0.5);
                            log::info!("time_scale: {:.2}", self.sky.time_scale);
                        }
                    }
                    PhysicalKey::Code(KeyCode::Equal) => {
                        if pressed {
                            self.sky.speed_mul(2.0);
                            log::info!("time_scale: {:.2}", self.sky.time_scale);
                        }
                    }
                    // Perf overlay toggle: avoid function keys in browsers/OS.
                    PhysicalKey::Code(KeyCode::KeyP) => {
                        if pressed {
                            self.hud_model.toggle_perf();
                            log::info!(
                                "Perf overlay {}",
                                if self.hud_model.perf_enabled() {
                                    "on"
                                } else {
                                    "off"
                                }
                            );
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyH) => {
                        if pressed {
                            self.hud_model.toggle_hud();
                            log::info!(
                                "HUD {}",
                                if self.hud_model.hud_enabled() {
                                    "shown"
                                } else {
                                    "hidden"
                                }
                            );
                        }
                    }
                    // 5s automated orbit capture (screenshots)
                    PhysicalKey::Code(KeyCode::KeyO) => {
                        if pressed {
                            // Start a 5-second smooth orbit capture
                            self.screenshot_start = Some(self.last_time);
                            log::info!("Screenshot mode: 5s orbit starting");
                        }
                    }
                    // Demo blast via Fireball (3) instead of F
                    // Allow keyboard respawn as fallback when dead
                    PhysicalKey::Code(KeyCode::Enter) => {
                        if pressed {
                            if !self.pc_alive {
                                log::info!("Respawn via keyboard");
                                self.respawn();
                            } else {
                                // Reset destructible grid and replay recent impacts (demo only)
                                #[cfg(feature = "vox_onepath_demo")]
                                {
                                    self.reset_voxel_and_replay();
                                }
                            }
                        }
                    }
                    // WoW-like toggles
                    PhysicalKey::Code(KeyCode::NumLock) => {
                        if pressed {
                            self.scene_inputs.toggle_autorun();
                        }
                    }
                    PhysicalKey::Code(KeyCode::NumpadDivide) => {
                        if pressed {
                            self.scene_inputs.toggle_walk();
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let mut step = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => *y,
                    winit::event::MouseScrollDelta::PixelDelta(p) => (p.y as f32) * 0.05,
                };
                if step.abs() < 1e-3 {
                    step = 0.0;
                }
                if step != 0.0 {
                    // Allow a closer near-zoom so the camera can sit just
                    // behind and slightly above the wizard's head.
                    self.cam_distance = (self.cam_distance - step).clamp(1.6, 25.0);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *button == winit::event::MouseButton::Right {
                    self.rmb_down = state.is_pressed();
                    if !self.rmb_down {
                        self.last_cursor_pos = None; // reset deltas
                    }
                    // Classic profile fallback: temporary capture while RMB held
                    let mut host_events = Vec::new();
                    let ui = client_core::systems::cursor::UiFocus::default();
                    client_core::systems::cursor::handle_cursor_event(
                        &mut self.controller_state,
                        &ui,
                        client_core::systems::cursor::CursorEvent::MouseRight(self.rmb_down),
                        &mut host_events,
                    );
                    for ev in host_events {
                        let client_core::systems::cursor::HostEvent::PointerLockRequest(b) = ev;
                        self.pointer_lock_request = Some(b);
                    }
                    // In mouselook, treat RMB as an action press
                    if self.controller_state.mode()
                        == ecs_core::components::ControllerMode::Mouselook
                        && self.rmb_down
                    {
                        let binds = client_core::systems::action_bindings::Bindings::default();
                        let snap = client_core::systems::action_bindings::ButtonSnapshot {
                            rmb_pressed: true,
                            ..Default::default()
                        };
                        client_core::systems::action_bindings::handle_buttons(
                            &binds,
                            &self.controller_state,
                            &snap,
                            &mut self.input_queue,
                        );
                    }
                }
                if *button == winit::event::MouseButton::Left {
                    self.lmb_down = state.is_pressed();
                    // Treat LMB as an action press on press-down
                    if state.is_pressed() {
                        let binds = client_core::systems::action_bindings::Bindings::default();
                        let snap = client_core::systems::action_bindings::ButtonSnapshot {
                            lmb_pressed: true,
                            ..Default::default()
                        };
                        client_core::systems::action_bindings::handle_buttons(
                            &binds,
                            &self.controller_state,
                            &snap,
                            &mut self.input_queue,
                        );
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                // Compute deltas (in px) and apply to controller when appropriate.
                let mut apply = false;
                let (dx, dy) = if let Some((lx, ly)) = self.last_cursor_pos {
                    ((position.x - lx) as f32, (position.y - ly) as f32)
                } else {
                    (0.0, 0.0)
                };
                // Apply in mouselook mode always; in Classic fallback apply while RMB is held.
                use ecs_core::components::ControllerMode;
                // If pointer is locked, we process relative motion via DeviceEvent::MouseMotion
                // and ignore CursorMoved to avoid double-applying deltas.
                if !self.pointer_locked
                    && (self.controller_state.mode() == ControllerMode::Mouselook || self.rmb_down)
                {
                    apply = true;
                }
                if apply {
                    client_core::systems::mouselook::apply_mouse_delta(
                        &self.controller_ml_cfg,
                        &mut self.controller_state,
                        dx,
                        dy,
                    );
                    // Mirror controller yaw/pitch into existing orbit fields for camera system.
                    // Positive pitch (mouse up) should tilt the orbit camera UP. Our orbit helper's
                    // X rotation tilts down for positive angles, so negate the controller pitch here.
                    // Also invert yaw sign so mouse-right turns camera right in the orbit system.
                    self.cam_orbit_yaw = wrap_angle(-self.controller_state.camera.yaw);
                    let orbit_pitch = -self.controller_state.camera.pitch;
                    self.cam_orbit_pitch = orbit_pitch.clamp(-1.2, 1.2);
                    // Do not rotate the PC from mouse movement; PC yaw follows movement (camera-relative)
                }
                // Track last cursor position
                self.last_cursor_pos = Some((position.x, position.y));
            }
            WindowEvent::Focused(false) => {
                // Clear sticky keys when window loses focus
                self.input.clear();
            }
            _ => {}
        }
    }

    /// Handle raw mouse motion deltas (used when the pointer is locked).
    pub fn handle_mouse_motion(&mut self, dx: f32, dy: f32) {
        use ecs_core::components::ControllerMode;
        if !self.pointer_locked {
            return; // only consume raw motion when locked
        }
        if self.controller_state.mode() == ControllerMode::Mouselook || self.rmb_down {
            client_core::systems::mouselook::apply_mouse_delta(
                &self.controller_ml_cfg,
                &mut self.controller_state,
                dx,
                dy,
            );
            // Mirror controller yaw/pitch into orbit fields
            self.cam_orbit_yaw = wrap_angle(-self.controller_state.camera.yaw);
            let orbit_pitch = -self.controller_state.camera.pitch;
            self.cam_orbit_pitch = orbit_pitch.clamp(-1.2, 1.2);
        }
    }
}
