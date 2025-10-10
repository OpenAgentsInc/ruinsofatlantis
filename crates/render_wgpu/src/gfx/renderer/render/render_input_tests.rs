#[cfg(test)]
mod wow_controller_input_tests {
    use glam::{Vec2, vec2};

    /// Mirrors the production per-frame A/D resolution in render.rs:
    /// - RMB held  -> A/D = strafes (A left, D right), Q/E preserved.
    /// - RMB not held -> A/D = turns  (A left, D right), Q/E preserved.
    fn resolve_ad_to_turn_or_strafe(
        rmb_down: bool,
        a_down: bool,
        d_down: bool,
        q_strafe_left: bool,
        q_strafe_right: bool,
    ) -> (bool, bool, bool, bool) {
        // returns (turn_left, turn_right, strafe_left, strafe_right)
        let mut turn_left = false;
        let mut turn_right = false;
        let mut strafe_left = q_strafe_left;
        let mut strafe_right = q_strafe_right;
        if rmb_down {
            strafe_left |= a_down;
            strafe_right |= d_down;
        } else {
            turn_left |= a_down;
            turn_right |= d_down;
        }
        (turn_left, turn_right, strafe_left, strafe_right)
    }

    /// Mirrors the dx/dz packaging in render.rs (camera-relative planar intent).
    /// Right vector is (fy, -fx); diagonal intents are normalized.
    fn compute_move_dx_dz(
        forward: bool,
        backward: bool,
        strafe_left: bool,
        strafe_right: bool,
        cam_fwd_xz: Vec2,
    ) -> (f32, f32) {
        let mut mx = 0.0f32; // right (+)/left (-)
        let mut mz = 0.0f32; // forward (+)/back (-)
        if strafe_right {
            mx += 1.0;
        }
        if strafe_left {
            mx -= 1.0;
        }
        if forward {
            mz += 1.0;
        }
        if backward {
            mz -= 1.0;
        }

        let fwd = if cam_fwd_xz.length_squared() > 0.0 {
            cam_fwd_xz.normalize()
        } else {
            Vec2::ZERO
        };
        // 90° CW on XZ
        let right = vec2(fwd.y, -fwd.x);
        let mut v = right * mx + fwd * mz;
        if v.length_squared() > 1.0 {
            v = v.normalize();
        }
        (v.x, v.y) // (dx, dz)
    }

    #[inline]
    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() <= 1e-6
    }
    fn assert_vec2_eq(got: (f32, f32), want: (f32, f32)) {
        assert!(
            approx_eq(got.0, want.0) && approx_eq(got.1, want.1),
            "expected ({:.6},{:.6}) but got ({:.6},{:.6})",
            want.0,
            want.1,
            got.0,
            got.1
        );
    }

    // --- A/D resolution (root of the left/right inversion bugs) ---

    #[test]
    fn ad_maps_to_turn_when_rmb_is_not_held() {
        // Q/E not pressed; RMB up => A/D turn, not strafe.
        let (tl, tr, sl, sr) = resolve_ad_to_turn_or_strafe(false, true, false, false, false);
        assert!(tl && !tr && !sl && !sr, "A should turn left when RMB is up");

        let (tl, tr, sl, sr) = resolve_ad_to_turn_or_strafe(false, false, true, false, false);
        assert!(
            !tl && tr && !sl && !sr,
            "D should turn right when RMB is up"
        );
    }

    #[test]
    fn ad_maps_to_strafe_when_rmb_is_held() {
        // Q/E not pressed; RMB down => A/D strafe, not turn.
        let (tl, tr, sl, sr) = resolve_ad_to_turn_or_strafe(true, true, false, false, false);
        assert!(!tl && !tr && sl && !sr, "RMB+A should strafe left");

        let (tl, tr, sl, sr) = resolve_ad_to_turn_or_strafe(true, false, true, false, false);
        assert!(!tl && !tr && !sl && sr, "RMB+D should strafe right");
    }

    #[test]
    fn qe_are_dedicated_strafes_and_preserved() {
        // Q/E must remain strafes regardless of RMB, and not become turns.
        let (tl, tr, sl, sr) = resolve_ad_to_turn_or_strafe(false, false, false, true, false);
        assert!(!tl && !tr && sl && !sr, "Q should strafe left (RMB up)");
        let (tl, tr, sl, sr) = resolve_ad_to_turn_or_strafe(true, false, false, false, true);
        assert!(!tl && !tr && !sl && sr, "E should strafe right (RMB down)");
    }

    #[test]
    fn click_move_forward_requires_both_mouse_buttons() {
        // Exact mapping used in render_impl: lmb && rmb
        let cmf = |lmb: bool, rmb: bool| -> bool { lmb && rmb };

        assert!(!cmf(false, false));
        assert!(!cmf(true, false));
        assert!(!cmf(false, true));
        assert!(cmf(true, true), "LMB+RMB must engage click-move-forward");
    }

    // --- Camera-relative intent packaging (dx, dz) ---

    #[test]
    fn packing_basic_with_camera_forward_plus_z() {
        // Camera looking straight +Z (fwd=(0,1), right=(1,0))
        let fwd = vec2(0.0, 1.0);

        // Forward only -> (0, 1)
        assert_vec2_eq(
            compute_move_dx_dz(true, false, false, false, fwd),
            (0.0, 1.0),
        );

        // Strafe right only -> (1, 0)
        assert_vec2_eq(
            compute_move_dx_dz(false, false, false, true, fwd),
            (1.0, 0.0),
        );

        // Forward + strafe left -> (-1/√2, +1/√2)
        let s2 = std::f32::consts::SQRT_2;
        assert_vec2_eq(
            compute_move_dx_dz(true, false, true, false, fwd),
            (-1.0 / s2, 1.0 / s2),
        );

        // Backward + strafe right -> (+1/√2, -1/√2)
        assert_vec2_eq(
            compute_move_dx_dz(false, true, false, true, fwd),
            (1.0 / s2, -1.0 / s2),
        );
    }

    #[test]
    fn packing_with_45deg_camera_forward_normalizes_diagonals() {
        // Camera at 45° in XZ -> fwd=(1,1)/√2; right=(+1,-1)/√2
        let fwd = vec2(1.0, 1.0).normalize();
        let s2 = std::f32::consts::SQRT_2;

        // Forward only -> (1/√2, 1/√2)
        assert_vec2_eq(
            compute_move_dx_dz(true, false, false, false, fwd),
            (1.0 / s2, 1.0 / s2),
        );

        // Strafe right only -> (1/√2, -1/√2)
        assert_vec2_eq(
            compute_move_dx_dz(false, false, false, true, fwd),
            (1.0 / s2, -1.0 / s2),
        );

        // Forward + strafe right -> (1, 0) after normalization
        assert_vec2_eq(
            compute_move_dx_dz(true, false, false, true, fwd),
            (1.0, 0.0),
        );

        // Forward + strafe left -> (0, 1) after normalization
        assert_vec2_eq(
            compute_move_dx_dz(true, false, true, false, fwd),
            (0.0, 1.0),
        );
    }

    #[test]
    fn zero_length_camera_forward_yields_zero_intent() {
        // Defensive edge case: if cam_fwd becomes zero, intents should be zero.
        let zero = Vec2::ZERO;
        assert_vec2_eq(
            compute_move_dx_dz(true, false, false, false, zero),
            (0.0, 0.0),
        );
        assert_vec2_eq(
            compute_move_dx_dz(false, false, true, true, zero),
            (0.0, 0.0),
        );
    }
}
