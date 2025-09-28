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
    let tan_half = (cam.fovy * 0.5).tan();
    let globals = Globals {
        view_proj: cam.view_proj().to_cols_array_2d(),
        cam_right_time: [right.x, right.y, right.z, t],
        cam_up_pad: [up.x, up.y, up.z, tan_half],
        sun_dir_time: [0.0, 1.0, 0.0, 0.0],
        sh_coeffs: [[0.0, 0.0, 0.0, 0.0]; 9],
        fog_params: [0.0, 0.0, 0.0, 0.0],
        clip_params: [cam.znear, cam.zfar, aspect, 0.0],
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
        Self {
            current_pos: glam::Vec3::ZERO,
            current_look: glam::Vec3::ZERO,
        }
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
    let tan_half = (cam.fovy * 0.5).tan();
    let globals = Globals {
        view_proj: cam.view_proj().to_cols_array_2d(),
        cam_right_time: [right.x, right.y, right.z, 0.0],
        cam_up_pad: [up.x, up.y, up.z, tan_half],
        sun_dir_time: [0.0, 1.0, 0.0, 0.0],
        sh_coeffs: [[0.0, 0.0, 0.0, 0.0]; 9],
        fog_params: [0.0, 0.0, 0.0, 0.0],
        clip_params: [cam.znear, cam.zfar, aspect, 0.0],
    };
    (cam, globals)
}

/// Compute local-space offsets for a third-person orbit camera.
/// Returns (camera_offset_local, look_offset_local).
pub fn compute_local_orbit_offsets(
    distance: f32,
    yaw: f32,
    pitch: f32,
    lift: f32,
    look_height: f32,
) -> (glam::Vec3, glam::Vec3) {
    let base = glam::vec3(0.0, 0.0, -distance.max(0.1));
    let q = glam::Quat::from_rotation_y(yaw) * glam::Quat::from_rotation_x(pitch);
    let mut off = q * base;
    off.y += lift;
    let look_off = glam::vec3(0.0, look_height, 0.0);
    (off, look_off)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbit_offset_yaw_rotates_horizontally() {
        let (off, _) =
            compute_local_orbit_offsets(10.0, std::f32::consts::FRAC_PI_2, 0.0, 0.0, 0.0);
        // Positive yaw rotates the camera to the character's left (negative X)
        assert!((off.x + 10.0).abs() < 1e-3);
        assert!(off.z.abs() < 1e-3);
    }

    #[test]
    fn orbit_offset_pitch_adds_height() {
        let (off, _) =
            compute_local_orbit_offsets(10.0, 0.0, std::f32::consts::FRAC_PI_4, 0.0, 0.0);
        assert!(off.y > 0.0);
    }

    #[test]
    fn smoothing_prevents_snap() {
        let mut st = FollowState::default();
        let target = glam::Vec3::ZERO;
        let aspect = 1.0;
        // Initialize at an initial ideal pose
        let (off0, look0) = compute_local_orbit_offsets(8.0, 0.0, 0.0, 2.0, 1.6);
        let _ = third_person_follow(
            &mut st,
            target,
            glam::Quat::IDENTITY,
            off0,
            look0,
            aspect,
            0.016,
        );
        let prev = st.current_pos;
        // Now jump the ideal direction by 90 deg; small dt should not snap
        let (off1, look1) =
            compute_local_orbit_offsets(8.0, std::f32::consts::FRAC_PI_2, 0.0, 2.0, 1.6);
        let _ = third_person_follow(
            &mut st,
            target,
            glam::Quat::IDENTITY,
            off1,
            look1,
            aspect,
            0.016,
        );
        let newp = st.current_pos;
        // It should move, but not equal the new ideal immediately
        assert_ne!(prev, newp);
        assert!(newp.distance(off1) > 0.01);
    }
}
