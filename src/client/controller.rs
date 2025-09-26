//! Basic third-person character controller for the PC.
//!
//! This controller implements a simple third-person scheme:
//! - A/D turn the character in place (no strafing).
//! - W moves forward along the character's facing.
//! - S does not translate (as requested) — it can be wired to backwards later.

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

    pub fn update(&mut self, input: &InputState, dt: f32, _cam_forward: Vec3) {
        // Tunables: faster forward movement, slower turning
        let speed = if input.run { 9.0 } else { 5.0 };
        let yaw_rate = 1.8; // rad/s

        // In-place yaw
        if input.left {
            self.yaw = wrap_angle(self.yaw + yaw_rate * dt);
        }
        if input.right {
            self.yaw = wrap_angle(self.yaw - yaw_rate * dt);
        }

        // Forward-only translation
        if input.forward {
            let fwd = Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos());
            self.pos += fwd.normalize_or_zero() * speed * dt;
        }
    }
}

#[allow(dead_code)]
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
