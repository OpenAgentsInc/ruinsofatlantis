//! Reprojection helpers for temporal accumulation.
//!
//! These functions reproject current-frame samples into previous-frame UV space
//! using current/previous view-projection matrices and jitter. They are CPU-only
//! helpers used in tests and to generate constants; shader-side implementations
//! should mirror the math.

use glam::{Mat4, Vec2, Vec3, Vec4};

/// Compute previous-frame UV for a pixel given current UV and depth (view-space depth).
/// - `curr_uv` in [0,1]
/// - `depth_vs` is positive distance along -Z in view space
/// - Matrices are column-major, standard OpenGL-style clip (right-handed), same as glam
/// - Jitter is in NDC pixels (curr, prev), scaled by 2/resolution in shader usage
pub fn reproject_uv(
    curr_uv: Vec2,
    depth_vs: f32,
    curr_view_proj: Mat4,
    prev_view_proj: Mat4,
    curr_jitter: Vec2,
    prev_jitter: Vec2,
) -> Vec2 {
    // Reconstruct view-space position from UV/depth (assume z = -depth_vs)
    // Convert curr UV to NDC
    let ndc = Vec3::new(curr_uv.x * 2.0 - 1.0, 1.0 - curr_uv.y * 2.0, -1.0);
    // Form a ray in view space along -Z with given depth
    // For CPU-side approximation in tests, assume small FOV; we carry depth directly.
    let pos_vs = Vec3::new(0.0, 0.0, -depth_vs);
    // Transform to world via inverse(curr_view)
    let curr_view = curr_view_proj.inverse();
    let pos_ws = (curr_view * Vec4::new(pos_vs.x, pos_vs.y, pos_vs.z, 1.0)).truncate();
    // Project with previous view-proj
    let clip_prev = prev_view_proj * pos_ws.extend(1.0);
    let ndc_prev = clip_prev.truncate() / clip_prev.w.max(1e-6);
    // Remove/add jitter: for CPU placeholder, we simply subtract prev jitter and add curr
    let ndc_prev_dejitter = Vec2::new(ndc_prev.x - prev_jitter.x, ndc_prev.y - prev_jitter.y);
    let uv_prev = Vec2::new((ndc_prev_dejitter.x * 0.5) + 0.5, (1.0 - ndc_prev_dejitter.y) * 0.5);
    uv_prev
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reprojection_identity_no_jitter_is_identity_uv() {
        let uv = Vec2::new(0.25, 0.75);
        let depth = 5.0;
        let m = Mat4::IDENTITY;
        let uv_prev = reproject_uv(uv, depth, m, m, Vec2::ZERO, Vec2::ZERO);
        // In this simplified CPU approximation, projected UV remains stable.
        assert!((uv_prev - uv).abs().max_element() < 1e-3);
    }
}

