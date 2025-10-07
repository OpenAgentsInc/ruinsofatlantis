//! Small geometry helpers reusable by systems.

use glam::{Vec2, Vec3};

#[inline]
pub fn segment_hits_circle_xz(p0: Vec3, p1: Vec3, center: Vec3, radius: f32) -> bool {
    let a = Vec2::new(p0.x, p0.z);
    let b = Vec2::new(p1.x, p1.z);
    let c = Vec2::new(center.x, center.z);
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 <= 1e-12 {
        return (a - c).length_squared() <= radius * radius;
    }
    let t = ((c - a).dot(ab) / len2).clamp(0.0, 1.0);
    let closest = a + ab * t;
    (closest - c).length_squared() <= radius * radius
}

