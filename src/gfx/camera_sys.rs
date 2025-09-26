//! Camera system helpers: orbiting camera + globals buffer prep.

use crate::gfx::camera::Camera;
use crate::gfx::types::Globals;

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
