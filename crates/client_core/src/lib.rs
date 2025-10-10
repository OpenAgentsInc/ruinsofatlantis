//! Client glue: input state and a simple third‑person controller.
//!
//! Scaffolds added for replication and GPU upload coordination live in
//! `replication`, `upload`, and `systems` modules.

#![deny(warnings, clippy::all, clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::struct_excessive_bools
)]

pub mod input {
    /// Input snapshot for one frame of local player intent.
    ///
    /// This mirrors WoW-style semantics:
    /// - `turn_left/right` apply keyboard yaw when not in mouselook (RMB not held)
    /// - `strafe_left/right` apply lateral movement; also used when RMB is held and A/D are pressed
    /// - `forward`/`backward` are movement intents; `click_move_forward` reflects LMB+RMB chord
    /// - `mouse_look` indicates RMB (or pointer-locked look) is active
    #[derive(Default, Debug, Clone, Copy)]
    pub struct InputState {
        pub forward: bool,
        pub backward: bool,
        pub strafe_left: bool,
        pub strafe_right: bool,
        pub turn_left: bool,
        pub turn_right: bool,
        pub click_move_forward: bool,
        pub mouse_look: bool,
        // Legacy: not used by WoW controller (run is default); retained for compatibility
        pub run: bool,
    }
    impl InputState {
        pub fn clear(&mut self) {
            *self = Self::default();
        }
    }
}

pub mod controller {
    use super::input::InputState;
    use glam::Vec3;

    #[derive(Debug, Clone, Copy)]
    pub struct PlayerController {
        pub pos: Vec3,
        pub yaw: f32,
        // WoW-style toggles
        pub autorun: bool,
        pub walk_mode: bool,
    }
    impl PlayerController {
        #[must_use]
        pub fn new(initial_pos: Vec3) -> Self {
            Self {
                pos: initial_pos,
                yaw: 0.0,
                autorun: false,
                walk_mode: false,
            }
        }
        /// Toggle autorun (behaves like holding `W` until canceled).
        pub fn toggle_autorun(&mut self) {
            self.autorun = !self.autorun;
        }
        /// Cancel autorun immediately (e.g., on `S`).
        pub fn cancel_autorun(&mut self) {
            self.autorun = false;
        }
        /// Toggle walk mode (reduces speed to walk).
        pub fn toggle_walk(&mut self) {
            self.walk_mode = !self.walk_mode;
        }

        /// Update WoW-style character controller: turn/strafe/autorun.
        pub fn update(&mut self, input: &InputState, dt: f32, cam_forward: Vec3) {
            // Constants converted from yards/sec
            const RUN_SPEED: f32 = 6.4008; // 7.0 yd/s
            const WALK_SPEED: f32 = 2.2860; // 2.5 yd/s
            const BACKPEDAL_SPEED: f32 = 4.1148; // 4.5 yd/s
            const TURN_SPEED_DEG: f32 = 180.0; // keyboard turn
            let turn_speed = TURN_SPEED_DEG.to_radians();

            // 1) Modes & modifiers
            let mut fwd_intent = input.forward || input.click_move_forward || self.autorun;
            if input.backward {
                fwd_intent = false; /* backpedal wins */
            }

            // 2) Intent axes (camera-relative)
            let mut fwd = 0.0;
            if fwd_intent {
                fwd += 1.0;
            }
            if input.backward {
                fwd = -1.0;
            }
            let mut strafe = 0.0;
            if input.strafe_left {
                strafe -= 1.0;
            }
            if input.strafe_right {
                strafe += 1.0;
            }

            // 3) Choose speed bucket
            let base_speed = if input.backward && fwd <= 0.0 {
                BACKPEDAL_SPEED
            } else if self.walk_mode {
                WALK_SPEED
            } else {
                RUN_SPEED
            };

            // 4) Build local move vector (normalize to avoid diagonal boost)
            let mut v_local = glam::Vec3::new(strafe, 0.0, fwd);
            if v_local.length_squared() > 0.0 {
                v_local = v_local.normalize() * base_speed;
            }

            // 5) Orientation rules
            if input.mouse_look {
                // Character yaw follows camera yaw every frame
                let cam_yaw = cam_forward.x.atan2(cam_forward.z);
                self.yaw = wrap_angle(cam_yaw);
            } else {
                // Keyboard turn: left decreases yaw, right increases yaw
                if input.turn_left {
                    self.yaw = wrap_angle(self.yaw - turn_speed * dt);
                }
                if input.turn_right {
                    self.yaw = wrap_angle(self.yaw + turn_speed * dt);
                }
            }

            // 6) Rotate local velocity by yaw and integrate
            if v_local.length_squared() > 0.0 {
                let cy = self.yaw;
                let world_x = v_local.x * cy.cos() + v_local.z * cy.sin();
                let world_z = -v_local.x * cy.sin() + v_local.z * cy.cos();
                let v_world = glam::Vec3::new(world_x, 0.0, world_z);
                self.pos += v_world * dt;
                // Stop conditions are immediate; no inertia (handled by inputs above)
            }
        }
    }
    fn wrap_angle(a: f32) -> f32 {
        let mut x = a;
        while x > std::f32::consts::PI {
            x -= std::f32::consts::TAU;
        }
        while x < -std::f32::consts::PI {
            x += std::f32::consts::TAU;
        }
        x
    }
}

/// Replication apply scaffolding and buffers.
pub mod replication;
/// Placeholder for client-side systems (prediction/lag-comp/etc.).
pub mod systems;
pub mod facade {
    pub mod controller;
}
/// Mesh upload interface used by renderer or client runtime integration.
pub mod upload;
/// Client-side zone snapshot loader (CPU presentation for renderer).
pub mod zone_client;
