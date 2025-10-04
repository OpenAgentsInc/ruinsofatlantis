//! Server scene build helpers for destructibles.
//!
//! For v0, we provide a utility to transform a local-space AABB by a model
//! matrix and compute a tight world-space AABB (8-corner transform).

pub fn world_aabb_from_local(
    local_min: [f32; 3],
    local_max: [f32; 3],
    model: [[f32; 4]; 4],
) -> ([f32; 3], [f32; 3]) {
    let m = glam::Mat4::from_cols_array_2d(&model);
    let lm = glam::Vec3::from(local_min);
    let lmax = glam::Vec3::from(local_max);
    let corners = [
        glam::vec3(lm.x, lm.y, lm.z),
        glam::vec3(lmax.x, lm.y, lm.z),
        glam::vec3(lm.x, lmax.y, lm.z),
        glam::vec3(lmax.x, lmax.y, lm.z),
        glam::vec3(lm.x, lm.y, lmax.z),
        glam::vec3(lmax.x, lm.y, lmax.z),
        glam::vec3(lm.x, lmax.y, lmax.z),
        glam::vec3(lmax.x, lmax.y, lmax.z),
    ];
    let mut wmin = glam::Vec3::splat(f32::INFINITY);
    let mut wmax = glam::Vec3::splat(f32::NEG_INFINITY);
    for c in &corners {
        let wc = m.transform_point3(*c);
        wmin = wmin.min(wc);
        wmax = wmax.max(wc);
    }
    ([wmin.x, wmin.y, wmin.z], [wmax.x, wmax.y, wmax.z])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aabb_transform_matches_expected() {
        let local_min = [-1.0, -2.0, -3.0];
        let local_max = [2.0, 3.0, 4.0];
        let t = glam::Mat4::from_translation(glam::vec3(10.0, 0.5, -2.0))
            * glam::Mat4::from_scale(glam::vec3(2.0, 1.0, 0.5));
        let (wmin, wmax) = world_aabb_from_local(local_min, local_max, t.to_cols_array_2d());
        // Rough bounds check
        assert!(wmin[0] < wmax[0] && wmin[1] < wmax[1] && wmin[2] < wmax[2]);
        // Spot check one corner
        let c = glam::vec3(local_max[0], local_min[1], local_max[2]);
        let wc = t.transform_point3(c);
        assert!(wc.x <= wmax[0] + 1e-4 && wc.y >= wmin[1] - 1e-4);
    }
}
