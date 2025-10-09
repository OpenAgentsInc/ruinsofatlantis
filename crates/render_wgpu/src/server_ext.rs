// Empty file if feature not enabled; the module is gated in lib.rs
use server_core::ServerState;
use server_core::ecs::schedule::HitEvent;

/// Extension trait adding projectile collision to ServerState using the renderer's Projectile type.
pub trait CollideProjectiles {
    fn collide_and_damage(
        &mut self,
        projectiles: &mut Vec<crate::gfx::fx::Projectile>,
        dt: f32,
        damage: i32,
    ) -> Vec<HitEvent>;
}

impl CollideProjectiles for ServerState {
    fn collide_and_damage(
        &mut self,
        _projectiles: &mut Vec<crate::gfx::fx::Projectile>,
        _dt: f32,
        _damage: i32,
    ) -> Vec<HitEvent> {
        // ServerState no longer exposes an `npcs` list directly; projectile collision
        // is handled in ECS systems (`ecs::schedule`). This extension remains as a
        // compatibility shim for the demo feature and returns no hits.
        Vec::new()
    }
}

/// Segment-circle intersection in XZ (ignores Y to behave like a vertical cylinder).
#[allow(dead_code)]
fn segment_hits_circle_xz(p0: glam::Vec3, p1: glam::Vec3, center: glam::Vec3, r: f32) -> bool {
    let p0 = glam::vec2(p0.x, p0.z);
    let p1 = glam::vec2(p1.x, p1.z);
    let c = glam::vec2(center.x, center.z);
    let d = p1 - p0;
    let m = p0 - c;
    let a = d.dot(d);
    if a <= 1e-6 {
        return m.length() <= r;
    }
    let t = (-(m.dot(d)) / a).clamp(0.0, 1.0);
    let closest = p0 + d * t;
    (closest - c).length() <= r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_hits_despite_height_difference() {
        // Center at origin on ground; projectile sweeps across in XZ at higher Y.
        let c = glam::vec3(0.0, 0.6, 0.0);
        let p0 = glam::vec3(-2.0, 1.6, 0.0);
        let p1 = glam::vec3(2.0, 1.6, 0.0);
        // Radius ~cube bounding circle with small pad used in runtime (~0.95)
        assert!(segment_hits_circle_xz(p0, p1, c, 0.95));
    }

    #[test]
    fn segment_misses_in_xz_when_far() {
        let c = glam::vec3(0.0, 0.0, 0.0);
        let p0 = glam::vec3(-2.0, 0.0, 2.0);
        let p1 = glam::vec3(2.0, 0.0, 2.0);
        assert!(!segment_hits_circle_xz(p0, p1, c, 0.5));
    }
}
