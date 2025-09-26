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
        // Tunables: faster forward movement, slower turning
        let speed = if input.run { 9.0 } else { 5.0 };
        let mut fwd = cam_forward;
        let mut move_dir = Vec3::ZERO;
        // Flatten forward and compute right
        fwd.y = 0.0;
        fwd = fwd.normalize_or_zero();
        let mut right = Vec3::Y.cross(fwd);
        right = right.normalize_or_zero();
        if input.forward { move_dir += fwd; }
        if input.backward { move_dir -= fwd; }
        // Expected behavior: D moves right, A moves left relative to camera.
        // If you observe opposite behavior due to asset/world handedness,
        // swap the signs here. Based on feedback, we invert compared to
        // previous iteration to match in-game expectation.
        if input.right { move_dir -= right; }
        if input.left { move_dir += right; }
        if move_dir.length_squared() > 1e-6 {
            move_dir = move_dir.normalize();
            self.pos += move_dir * speed * dt;
            // Smoothly rotate toward target yaw
            let target_yaw = move_dir.x.atan2(move_dir.z);
            let max_turn = 2.5 * dt; // rad/s (slower for smoother turns)
            self.yaw = turn_towards(self.yaw, target_yaw, max_turn);
        }
    }
}

fn turn_towards(current: f32, target: f32, max_delta: f32) -> f32 {
    let delta = wrap_angle(target - current);
    if delta.abs() <= max_delta {
        return target;
    }
    if delta > 0.0 {
        wrap_angle(current + max_delta)
    } else {
        wrap_angle(current - max_delta)
    }
}

fn wrap_angle(a: f32) -> f32 {
    let mut x = a;
    while x > std::f32::consts::PI { x -= std::f32::consts::TAU; }
    while x < -std::f32::consts::PI { x += std::f32::consts::TAU; }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turn_towards_limits_angular_velocity() {
        let cur = 0.0;
        let target = std::f32::consts::FRAC_PI_2; // 90 deg
        let next = turn_towards(cur, target, 0.1);
        assert!((next - 0.1).abs() < 1e-6);
    }

    #[test]
    fn update_rotates_smoothly() {
        let mut pc = PlayerController::new(Vec3::ZERO);
        let input = InputState { right: true, ..Default::default() };
        // camera forward along +Z
        let cam_fwd = Vec3::new(0.0, 0.0, 1.0);
        pc.update(&input, 0.016, cam_fwd);
        // Should change yaw smoothly (magnitude less than 90deg)
        assert!(pc.yaw.abs() > 0.0 && pc.yaw.abs() < std::f32::consts::FRAC_PI_2);
    }
}
