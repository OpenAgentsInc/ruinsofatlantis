//! Camera system helpers: orbiting camera + globals buffer prep.

use crate::gfx::camera::Camera;
use crate::gfx::types::Globals;

#[allow(dead_code)] // legacy orbit mode kept for parity and quick toggles
pub fn orbit_and_globals(
    cam_target: glam::Vec3,
    radius: f32,
    speed: f32,
    aspect: f32,
    t: f32,
) -> (Camera, Globals) {
    let angle = t * speed;
    let cam = Camera::orbit(cam_target, radius, angle, aspect);
    let forward = (cam_target - cam.eye).normalize_or_zero();
    let right = forward.cross(glam::Vec3::Y).normalize_or_zero();
    let up = right.cross(forward).normalize_or_zero();
    let globals = Globals {
        view_proj: cam.view_proj().to_cols_array_2d(),
        cam_right_time: [right.x, right.y, right.z, t],
        cam_up_pad: [up.x, up.y, up.z, 0.0],
    };
    (cam, globals)
}

/// Follow camera state for third-person smoothing.
///
/// Maintains current eye/look positions to allow exponential smoothing towards
/// an "ideal" camera configuration based on a target's transform.
#[derive(Debug, Clone, Copy)]
pub struct FollowState {
    pub current_pos: glam::Vec3,
    pub current_look: glam::Vec3,
}

impl Default for FollowState {
    fn default() -> Self {
        Self { current_pos: glam::Vec3::ZERO, current_look: glam::Vec3::ZERO }
    }
}

/// Update a third-person follow camera toward an offset from `target_pos`.
///
/// - `offset` is in target-local space (rotated by `target_rot`).
/// - `look_offset` is also target-local and aims the camera slightly above/ahead.
/// - `dt` controls smoothing via an exponential factor: t = 1 - 0.01^dt.
pub fn third_person_follow(
    state: &mut FollowState,
    target_pos: glam::Vec3,
    target_rot: glam::Quat,
    offset: glam::Vec3,
    look_offset: glam::Vec3,
    aspect: f32,
    dt: f32,
) -> (Camera, Globals) {
    let ideal_pos = target_pos + target_rot * offset;
    let ideal_look = target_pos + target_rot * look_offset;
    // Exponential smoothing (ported from Quick_3D_RPG idea)
    let t = 1.0 - 0.01f32.powf(dt.max(0.0));
    state.current_pos = state.current_pos.lerp(ideal_pos, t);
    state.current_look = state.current_look.lerp(ideal_look, t);

    let cam = Camera {
        eye: state.current_pos,
        target: state.current_look,
        up: glam::Vec3::Y,
        aspect,
        fovy: 60f32.to_radians(),
        znear: 0.1,
        zfar: 1000.0,
    };
    let forward = (cam.target - cam.eye).normalize_or_zero();
    let right = forward.cross(glam::Vec3::Y).normalize_or_zero();
    let up = right.cross(forward).normalize_or_zero();
    let globals = Globals {
        view_proj: cam.view_proj().to_cols_array_2d(),
        cam_right_time: [right.x, right.y, right.z, 0.0],
        cam_up_pad: [up.x, up.y, up.z, 0.0],
    };
    (cam, globals)
}
