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
    #[derive(Default, Debug, Clone, Copy)]
    pub struct InputState {
        pub forward: bool,
        pub backward: bool,
        pub left: bool,
        pub right: bool,
        pub run: bool, // Shift
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
    }
    impl PlayerController {
        #[must_use]
        pub fn new(initial_pos: Vec3) -> Self {
            Self {
                pos: initial_pos,
                yaw: 0.0,
            }
        }
        pub fn update(&mut self, input: &InputState, dt: f32, cam_forward: Vec3) {
            // Movement is camera-relative: W moves away from viewer (camera forward),
            // S toward camera, A left, D right. Mouse does not rotate the PC.
            let speed = if input.run { 9.0 } else { 5.0 };
            // Build camera-space basis on XZ plane
            let mut fwd = Vec3::new(cam_forward.x, 0.0, cam_forward.z).normalize_or_zero();
            if fwd.length_squared() <= 1e-6 {
                fwd = Vec3::Z; // default forward if camera forward degenerates
            }
            // Right-handed basis: right = fwd x up = (-fwd.z, 0, fwd.x)
            let right = Vec3::new(-fwd.z, 0.0, fwd.x).normalize_or_zero();
            // Aggregate movement intent
            let mut dir = Vec3::ZERO;
            if input.forward {
                dir += fwd;
            }
            if input.backward {
                dir -= fwd;
            }
            if input.right {
                dir += right;
            }
            if input.left {
                dir -= right;
            }
            if dir.length_squared() > 1e-6 {
                let move_dir = dir.normalize();
                self.pos += move_dir * speed * dt;
                // Optionally rotate PC to face movement direction (not mouse/camera)
                self.yaw = wrap_angle(move_dir.x.atan2(move_dir.z));
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
