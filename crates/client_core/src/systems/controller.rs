//! Player controller integration (movement + camera-facing) — v0.
//!
//! This module centralizes player movement updates so the renderer can
//! delegate transform math here and only upload GPU buffers.

use crate::controller::PlayerController;
use crate::input::InputState;
use glam::Vec3;

/// Update the player controller position/yaw based on input and camera forward.
/// Movement is camera-relative; mouse does not rotate the PC.
pub fn update(player: &mut PlayerController, input: &InputState, dt: f32, cam_forward: Vec3) {
    player.update(input, dt, cam_forward);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasd_camera_relative_mapping() {
        let mut pc = PlayerController::new(Vec3::ZERO);
        let mut input = InputState::default();
        let dt = 1.0;
        let cam = Vec3::Z; // looking +Z => forward is +Z
        // W moves +Z
        input.forward = true;
        update(&mut pc, &input, dt, cam);
        assert!(pc.pos.z > 0.9);
        // Reset and test A (left)
        pc.pos = Vec3::ZERO;
        input = InputState::default();
        input.left = true;
        update(&mut pc, &input, dt, cam);
        assert!(pc.pos.x < -0.9);
    }
}

