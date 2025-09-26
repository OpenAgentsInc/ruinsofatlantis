//! Camera utilities.
//!
//! The renderer uses a very simple orbit camera for the prototype. In a real client
//! you would drive this from player input and game state.

use glam::{Mat4, Vec3};

pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Camera {
    pub fn orbit(target: Vec3, radius: f32, angle: f32, aspect: f32) -> Self {
        let offset = Vec3::new(angle.cos() * radius, radius * 0.6, angle.sin() * radius);
        let eye = target + offset;
        Self {
            eye,
            target,
            up: Vec3::Y,
            aspect,
            fovy: 60f32.to_radians(),
            znear: 0.1,
            zfar: 1000.0,
        }
    }

    pub fn view_proj(&self) -> Mat4 {
        let view = Mat4::look_at_rh(self.eye, self.target, self.up);
        let proj = Mat4::perspective_rh(self.fovy, self.aspect, self.znear, self.zfar);
        proj * view
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbit_sets_eye_above_target() {
        let cam = Camera::orbit(Vec3::new(0.0, 0.0, 0.0), 10.0, 0.0, 16.0 / 9.0);
        assert!(cam.eye.y > 0.0);
        assert!((cam.aspect - 16.0 / 9.0).abs() < 1e-6);
    }

    #[test]
    fn view_proj_has_perspective_properties() {
        let cam = Camera::orbit(Vec3::ZERO, 5.0, 1.0, 1.0);
        let vp = cam.view_proj();
        let arr = vp.to_cols_array();
        // Matrix should be invertible and not identity
        assert!(vp.determinant().abs() > 1e-6);
        assert!(arr.iter().any(|&x| (x - 1.0).abs() > 1e-3));
    }

    #[test]
    fn view_proj_transforms_target_to_depth_range() {
        let cam = Camera::orbit(Vec3::ZERO, 5.0, 0.3, 1.0);
        let vp = cam.view_proj();
        let p = vp * glam::Vec4::new(0.0, 0.0, 0.0, 1.0);
        assert!(p.w > 0.0);
    }

    #[test]
    fn orbit_radius_changes_distance() {
        let cam1 = Camera::orbit(Vec3::ZERO, 2.0, 0.0, 1.0);
        let cam2 = Camera::orbit(Vec3::ZERO, 6.0, 0.0, 1.0);
        let d1 = cam1.eye.length();
        let d2 = cam2.eye.length();
        assert!(d2 > d1);
    }
}
