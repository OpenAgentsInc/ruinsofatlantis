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
                    // Ignore movement/casting inputs if the PC is dead
                    PhysicalKey::Code(KeyCode::KeyW) if self.pc_alive => {
                        self.input.forward = pressed
                    }
                    PhysicalKey::Code(KeyCode::KeyS) if self.pc_alive => {
                        self.input.backward = pressed
                    }
                    PhysicalKey::Code(KeyCode::KeyA) if self.pc_alive => self.input.left = pressed,
                    PhysicalKey::Code(KeyCode::KeyD) if self.pc_alive => self.input.right = pressed,
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
                                log::debug!("PC cast queued: Fire Bolt");
                            } else {
                                log::debug!(
                                    "Fire Bolt on cooldown: {:.0} ms remaining",
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
                                log::debug!("PC cast queued: Magic Missile");
                            } else {
                                log::debug!(
                                    "Magic Missile on cooldown: {:.0} ms remaining",
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
                                log::debug!("PC cast queued: Fireball");
                            } else {
                                log::debug!(
                                    "Fireball on cooldown: {:.0} ms remaining",
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
                    PhysicalKey::Code(KeyCode::F1) => {
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
                    PhysicalKey::Code(KeyCode::F5) => {
                        if pressed {
                            // Start a 5-second smooth orbit capture
                            self.screenshot_start = Some(self.last_time);
                            log::info!("Screenshot mode: 5s orbit starting");
                        }
                    }
                    // Allow keyboard respawn as fallback when dead
                    PhysicalKey::Code(KeyCode::KeyR) | PhysicalKey::Code(KeyCode::Enter) => {
                        if pressed && !self.pc_alive {
                            log::info!("Respawn via keyboard");
                            self.respawn();
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
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                // Use previous cursor for deltas, then update to current.
                if self.rmb_down
                    && let Some((lx, ly)) = self.last_cursor_pos
                {
                    let dx = position.x - lx;
                    let dy = position.y - ly;
                    let sens = 0.005;
                    // Fully sync player facing with mouse drag; keep camera behind the player
                    let yaw_delta = dx as f32 * sens;
                    self.player.yaw = wrap_angle(self.player.yaw - yaw_delta);
                    // Propagate to controller so SceneInputs yaw stays in sync
                    self.scene_inputs.set_yaw(self.player.yaw);
                    self.cam_orbit_yaw = 0.0;
                    // Invert pitch control (mouse up pitches camera down, and vice versa)
                    self.cam_orbit_pitch =
                        (self.cam_orbit_pitch + dy as f32 * sens).clamp(-0.6, 1.2);
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
}
