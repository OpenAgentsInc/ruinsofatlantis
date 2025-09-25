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
        Self { eye, target, up: Vec3::Y, aspect, fovy: 60f32.to_radians(), znear: 0.1, zfar: 1000.0 }
    }

    pub fn view_proj(&self) -> Mat4 {
        let view = Mat4::look_at_rh(self.eye, self.target, self.up);
        let proj = Mat4::perspective_rh(self.fovy, self.aspect, self.znear, self.zfar);
        proj * view
    }
}
