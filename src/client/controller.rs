//! Basic third-person character controller for the PC.
//!
//! This controller implements a simple third-person scheme:
//! - A/D turn the character in place (no strafing).
//! - W moves forward along the character's facing.
//! - S moves straight back along the character's facing (no strafing).

use glam::Vec3;

use super::input::InputState;

#[derive(Debug, Clone, Copy)]
pub struct PlayerController {
    pub pos: Vec3,
    pub yaw: f32,
}

impl PlayerController {
    pub fn new(initial_pos: Vec3) -> Self {
        Self {
            pos: initial_pos,
            yaw: 0.0,
        }
    }

    pub fn update(&mut self, input: &InputState, dt: f32, _cam_forward: Vec3) {
        // Tunables: faster forward movement, slower turning
        let speed = if input.run { 9.0 } else { 5.0 };
        let yaw_rate = 1.8; // rad/s

        // Yaw updates:
        // - If only S is held (no A/D), keep yaw fixed (straight back).
        // - Otherwise, apply A/D turning (including while backing up).
        let only_backward = input.backward && !input.left && !input.right && !input.forward;
        if !only_backward {
            if input.left {
                self.yaw = wrap_angle(self.yaw + yaw_rate * dt);
            }
            if input.right {
                self.yaw = wrap_angle(self.yaw - yaw_rate * dt);
            }
        }

        // Forward/backward translation only (no strafing)
        let fwd = Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos()).normalize_or_zero();
        if input.forward && !input.backward {
            self.pos += fwd * speed * dt;
        } else if input.backward && !input.forward {
            self.pos -= fwd * speed * dt;
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
    while x > std::f32::consts::PI {
        x -= std::f32::consts::TAU;
    }
    while x < -std::f32::consts::PI {
        x += std::f32::consts::TAU;
    }
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
        let input = InputState {
            right: true,
            ..Default::default()
        };
        // camera forward along +Z
        let cam_fwd = Vec3::new(0.0, 0.0, 1.0);
        pc.update(&input, 0.016, cam_fwd);
        // Should change yaw smoothly (magnitude less than 90deg)
        assert!(pc.yaw.abs() > 0.0 && pc.yaw.abs() < std::f32::consts::FRAC_PI_2);
    }

    #[test]
    fn backward_moves_straight_back_no_yaw_change() {
        let mut pc = PlayerController::new(Vec3::ZERO);
        pc.yaw = 0.7; // arbitrary facing
        let input = InputState {
            backward: true,
            ..Default::default()
        };
        let cam_fwd = Vec3::new(0.0, 0.0, 1.0);
        let yaw0 = pc.yaw;
        pc.update(&input, 0.2, cam_fwd);
        // No yaw change when S is held alone
        assert!((pc.yaw - yaw0).abs() < 1e-6);
        // Displacement aligns with -forward
        let fwd = Vec3::new(yaw0.sin(), 0.0, yaw0.cos()).normalize_or_zero();
        let disp = pc.pos;
        assert!(disp.dot(-fwd) > 0.0);
    }

    #[test]
    fn backward_with_turn_changes_yaw() {
        let mut pc = PlayerController::new(Vec3::ZERO);
        pc.yaw = 0.7;
        let input = InputState {
            backward: true,
            left: true,
            ..Default::default()
        };
        let cam_fwd = Vec3::new(0.0, 0.0, 1.0);
        let yaw0 = pc.yaw;
        pc.update(&input, 0.2, cam_fwd);
        assert!(pc.yaw != yaw0);
        // Still moves approximately backward relative to updated facing
        let fwd1 = Vec3::new(pc.yaw.sin(), 0.0, pc.yaw.cos()).normalize_or_zero();
        assert!(pc.pos.dot(-fwd1) > 0.0);
    }
}
