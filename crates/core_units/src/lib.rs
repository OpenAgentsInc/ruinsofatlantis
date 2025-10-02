//! core_units: strongly-typed base units in meters/seconds/kilograms.
//!
//! Scope
//! - Provide simple `Length`, `Time`, and `Mass` newtypes (f64 under the hood).
//! - Implement basic arithmetic with scalars and same-typed values.
//! - Keep this crate tiny and dependency-free; conversions are explicit.
//!
//! Extending
//! - Add velocity/acceleration types as needed in follow-ups (e.g., `Velocity` = m/s).
//! - Consider `serde` feature-gated derives when units cross process boundaries.

use core::fmt;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

/// Length in meters (f64).
#[derive(Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct Length(pub f64);

/// Time in seconds (f64).
#[derive(Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct Time(pub f64);

/// Mass in kilograms (f64).
#[derive(Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct Mass(pub f64);

impl fmt::Debug for Length {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.6} m", self.0)
    }
}
impl fmt::Debug for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.6} s", self.0)
    }
}
impl fmt::Debug for Mass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.6} kg", self.0)
    }
}

// Conversions
impl From<f64> for Length {
    fn from(v: f64) -> Self {
        Length(v)
    }
}
impl From<Length> for f64 {
    fn from(v: Length) -> Self {
        v.0
    }
}
impl From<f64> for Time {
    fn from(v: f64) -> Self {
        Time(v)
    }
}
impl From<Time> for f64 {
    fn from(v: Time) -> Self {
        v.0
    }
}
impl From<f64> for Mass {
    fn from(v: f64) -> Self {
        Mass(v)
    }
}
impl From<Mass> for f64 {
    fn from(v: Mass) -> Self {
        v.0
    }
}

// Basic arithmetic with same-type values
macro_rules! impl_ops_same {
    ($T:ty) => {
        impl Add for $T {
            type Output = $T;
            fn add(self, rhs: $T) -> $T {
                <$T>::from(f64::from(self) + f64::from(rhs))
            }
        }
        impl AddAssign for $T {
            fn add_assign(&mut self, rhs: $T) {
                *self = *self + rhs;
            }
        }
        impl Sub for $T {
            type Output = $T;
            fn sub(self, rhs: $T) -> $T {
                <$T>::from(f64::from(self) - f64::from(rhs))
            }
        }
        impl SubAssign for $T {
            fn sub_assign(&mut self, rhs: $T) {
                *self = *self - rhs;
            }
        }
        impl Mul<f64> for $T {
            type Output = $T;
            fn mul(self, rhs: f64) -> $T {
                <$T>::from(f64::from(self) * rhs)
            }
        }
        impl MulAssign<f64> for $T {
            fn mul_assign(&mut self, rhs: f64) {
                *self = *self * rhs;
            }
        }
        impl Div<f64> for $T {
            type Output = $T;
            fn div(self, rhs: f64) -> $T {
                <$T>::from(f64::from(self) / rhs)
            }
        }
        impl DivAssign<f64> for $T {
            fn div_assign(&mut self, rhs: f64) {
                *self = *self / rhs;
            }
        }
    };
}

impl_ops_same!(Length);
impl_ops_same!(Time);
impl_ops_same!(Mass);

/// Compute volume of a cube of edge `voxel` (meters^3).
pub fn cube_volume_m3(voxel: Length) -> f64 {
    let e = voxel.0;
    e * e * e
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_ops_and_convert() {
        let a = Length::from(2.0);
        let b = Length::from(3.5);
        let c = a + b;
        assert!((f64::from(c) - 5.5).abs() < 1e-12);
        let mut d = c;
        d *= 2.0;
        assert!((f64::from(d) - 11.0).abs() < 1e-12);
        d /= 4.0;
        assert!((f64::from(d) - 2.75).abs() < 1e-12);
    }

    #[test]
    fn mass_and_time_behave_like_scalars() {
        let mut m = Mass::from(5.0);
        m += Mass::from(1.25);
        assert!((f64::from(m) - 6.25).abs() < 1e-12);
        let mut t = Time::from(10.0);
        t -= Time::from(0.25);
        assert!((f64::from(t) - 9.75).abs() < 1e-12);
    }

    #[test]
    fn cube_volume_is_edge_cubed() {
        let v = cube_volume_m3(Length(0.5));
        assert!((v - 0.125).abs() < 1e-12);
    }
}
