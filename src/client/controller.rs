//! Basic third-person character controller for the PC.
//!
//! This controller implements simple WASD movement relative to the camera's
//! forward/right axes and rotates the character to face the movement direction.

use glam::Vec3;

use super::input::InputState;

#[derive(Debug, Clone, Copy)]
pub struct PlayerController {
    pub pos: Vec3,
    pub yaw: f32,
}

impl PlayerController {
    pub fn new(initial_pos: Vec3) -> Self {
        Self { pos: initial_pos, yaw: 0.0 }
    }

    pub fn update(&mut self, input: &InputState, dt: f32, cam_forward: Vec3) {
        let speed = if input.run { 6.0 } else { 3.0 };
        let mut fwd = cam_forward;
        let mut move_dir = Vec3::ZERO;
        // Flatten forward and compute right
        fwd.y = 0.0;
        fwd = fwd.normalize_or_zero();
        let mut right = fwd.cross(Vec3::Y);
        right = right.normalize_or_zero();
        if input.forward { move_dir += fwd; }
        if input.backward { move_dir -= fwd; }
        if input.right { move_dir += right; }
        if input.left { move_dir -= right; }
        if move_dir.length_squared() > 1e-6 {
            move_dir = move_dir.normalize();
            self.pos += move_dir * speed * dt;
            self.yaw = move_dir.x.atan2(move_dir.z);
        }
    }
}

