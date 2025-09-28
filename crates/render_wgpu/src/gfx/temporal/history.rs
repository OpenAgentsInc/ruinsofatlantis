//! History textures and neighborhood clamp scaffolding.
//!
//! Runtime will allocate and manage per-pass history buffers; this module
//! declares configuration knobs and a small helper for clamp factors.

/// Temporal parameters shared across passes.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct TemporalParams {
    pub alpha: f32,
    pub clamp_k: f32,
    pub reactive_boost: f32,
}

impl Default for TemporalParams {
    fn default() -> Self {
        Self {
            alpha: 0.9,
            clamp_k: 3.0,
            reactive_boost: 0.0,
        }
    }
}

/// Compute a simple clamp range given mean and variance; returns (min,max).
#[allow(dead_code)]
pub fn clamp_range(mean: f32, variance: f32, k: f32) -> (f32, f32) {
    let sigma = variance.max(0.0).sqrt();
    (mean - k * sigma, mean + k * sigma)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clamp_range_symmetry() {
        let (lo, hi) = clamp_range(0.5, 0.04, 2.0); // sigma=0.2
        assert!((lo - 0.1).abs() < 1e-6);
        assert!((hi - 0.9).abs() < 1e-6);
    }
}
