//! Small math helpers and unit tests (scaffold).

/// Clamp a value to [0, 1].
pub fn clamp01(x: f32) -> f32 {
    if x < 0.0 {
        0.0
    } else if x > 1.0 {
        1.0
    } else {
        x
    }
}

/// Degrees to radians.
pub fn deg_to_rad(d: f32) -> f32 {
    d.to_radians()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp01_bounds() {
        assert_eq!(clamp01(-1.0), 0.0);
        assert_eq!(clamp01(0.0), 0.0);
        assert_eq!(clamp01(0.5), 0.5);
        assert_eq!(clamp01(1.0), 1.0);
        assert_eq!(clamp01(2.0), 1.0);
    }

    #[test]
    fn deg_to_rad_basic() {
        let pi = std::f32::consts::PI;
        assert!((deg_to_rad(180.0) - pi).abs() < 1e-6);
        assert!((deg_to_rad(90.0) - (pi / 2.0)).abs() < 1e-6);
    }
}
