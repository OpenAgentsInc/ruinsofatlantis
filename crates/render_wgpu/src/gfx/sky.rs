//! Sky and lighting: Hosek–Wilkie sky, sun motion, and SH ambient.
//!
//! This module owns time-of-day, sun direction, Hosek–Wilkie coefficient prep,
//! and SH-L2 ambient projection. The GPU background is rendered via `sky.wgsl`
//! using the raw HW parameters; geometry lighting consumes sun direction and
//! SH irradiance coefficients from `Globals`.
//!
//! Extending:
//! - Add zone JSON parsing in `data/weather/` and blend overrides in `update()`.
//! - Add shadow map setup in a dedicated `shadows.rs` (Phase 2).

use glam::{Vec3, vec3};
use hw_skymodel::rgb::{Channel, SkyParams, SkyState};

/// Packed sky uniform for `sky.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkyUniform {
    /// Packed params: 0..=2 = R (p0..p8), 3..=5 = G, 6..=8 = B
    pub params_packed: [[f32; 4]; 9],
    /// radiances.xyz = RGB radiance scale
    pub radiances: [f32; 4],
    /// sun_dir.xyz, .w = time/day_frac (debug)
    pub sun_dir_time: [f32; 4],
}

/// Simple weather parameters for clear sky.
#[derive(Clone, Copy, Debug)]
pub struct Weather {
    pub turbidity: f32, // 1..10
    pub ground_albedo: [f32; 3],
}

impl Default for Weather {
    fn default() -> Self {
        Self {
            turbidity: 3.0,
            ground_albedo: [0.1, 0.1, 0.1],
        }
    }
}

/// Runtime sky state used by the renderer.
#[derive(Clone, Debug)]
pub struct SkyStateCPU {
    pub day_frac: f32,   // [0..1]
    pub time_scale: f32, // x realtime
    pub paused: bool,
    pub weather: Weather,
    // Derived per update
    pub sun_dir: Vec3,
    pub sh9_rgb: [[f32; 3]; 9], // Irradiance SH (L2), RGB per coefficient
    pub sky_uniform: SkyUniform,
}

impl SkyStateCPU {
    pub fn new() -> Self {
        let mut s = Self {
            day_frac: 0.35,
            time_scale: 6.0,
            paused: false,
            weather: Weather::default(),
            sun_dir: vec3(0.0, 1.0, 0.0),
            sh9_rgb: [[0.0; 3]; 9],
            sky_uniform: SkyUniform {
                params_packed: [[0.0; 4]; 9],
                radiances: [1.0, 1.0, 1.0, 0.0],
                sun_dir_time: [0.0; 4],
            },
        };
        s.recompute();
        s
    }

    /// Advance time-of-day and recompute lighting.
    pub fn update(&mut self, dt: f32) {
        if !self.paused {
            self.day_frac = (self.day_frac + dt * self.time_scale / 86400.0).fract();
            // For prototyping, treat 1.0 game-day == 60s when time_scale=1440
            // Default time_scale=6.0 => ~4 minutes/day. Designers can scrub via hotkeys.
        }
        self.recompute();
    }

    pub fn scrub(&mut self, delta: f32) {
        self.day_frac = (self.day_frac + delta).rem_euclid(1.0);
        self.recompute();
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub fn speed_mul(&mut self, k: f32) {
        self.time_scale = (self.time_scale * k).clamp(0.01, 1000.0);
    }

    /// Recompute sun direction, HW sky params, and SH ambient from current state.
    ///
    /// Night mode
    /// - We deliberately darken sky radiance and ambient SH when `sun_dir.y <= 0.0`
    ///   so that midnight scenes are truly dark (good contrast for emissive VFX).
    /// - A tiny floor avoids fully‑black banding and keeps UI readable.
    pub fn recompute(&mut self) {
        self.sun_dir = sun_dir_from_day_frac(self.day_frac);
        let elev = self
            .sun_dir
            .y
            .max(0.0)
            .asin()
            .clamp(0.0, std::f32::consts::FRAC_PI_2);
        // HW state for current weather and elevation
        let sky = SkyState::new(&SkyParams {
            elevation: elev,
            turbidity: self.weather.turbidity,
            albedo: self.weather.ground_albedo,
        })
        .expect("valid HW params");
        let (params, mut radiances) = sky.raw();
        // Night darkening: when the sun is below the horizon, drastically reduce
        // sky radiance and ambient. Use a smooth ramp near the horizon and keep
        // a tiny floor to avoid pure black banding.
        let sun_y = self.sun_dir.y;
        // Slightly gentler night ramp so the sky remains readable on web
        let ramp = sun_y.max(0.0).powf(2.2);
        let night_floor = 0.08; // minimal residual radiance (was 0.015)
        radiances[0] = radiances[0] * ramp + night_floor;
        radiances[1] = radiances[1] * ramp + night_floor;
        radiances[2] = radiances[2] * ramp + night_floor;
        self.sky_uniform = pack_hw_uniform(params, radiances, self.sun_dir, self.day_frac);
        // Project to SH (irradiance) for ambient, then scale similarly
        self.sh9_rgb = project_irradiance_sh9(self.sun_dir, &self.weather);
        let amb_scale = ramp * 1.0 + (0.12); // stronger ambient floor at night for readability
        for i in 0..9 {
            self.sh9_rgb[i][0] *= amb_scale;
            self.sh9_rgb[i][1] *= amb_scale;
            self.sh9_rgb[i][2] *= amb_scale;
        }
    }
}

/// Map day fraction [0..1] to a simple sun direction in world space.
/// Noon at 0.5, sunrise ~0.25, sunset ~0.75; path is a vertical half-circle
/// with a slight azimuth offset so shadows aren't perfectly aligned with axes.
pub fn sun_dir_from_day_frac(day_frac: f32) -> Vec3 {
    let theta = day_frac * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2; // -pi/2 .. 3pi/2
    // vertical half-circle in X-Y, then rotate around Y for azimuth
    let dir_v = vec3(theta.cos(), theta.sin(), 0.0);
    let az = 0.6; // ~34 degrees
    let rot_y = glam::Mat3::from_rotation_y(az);
    (rot_y * dir_v).normalize()
}

/// Pack 27 params (9 per RGB) + 3 radiances into 9 vec4 + 1 vec4 layout.
fn pack_hw_uniform(
    params: [f32; 27],
    radiances: [f32; 3],
    sun_dir: Vec3,
    day_frac: f32,
) -> SkyUniform {
    let mut out = [[0.0f32; 4]; 9];
    for i in 0..9 {
        // R channel
        out[i][0] = params[i];
        // G channel (offset 9)
        out[i][1] = params[9 + i];
        // B channel (offset 18)
        out[i][2] = params[18 + i];
    }
    SkyUniform {
        params_packed: out,
        radiances: [radiances[0], radiances[1], radiances[2], 0.0],
        sun_dir_time: [sun_dir.x, sun_dir.y, sun_dir.z, day_frac],
    }
}

/// Compute SH-L2 irradiance coefficients (RGB) from the sky model by sampling.
/// Returns 9 coefficients, each is [R,G,B].
pub fn project_irradiance_sh9(sun_dir: Vec3, weather: &Weather) -> [[f32; 3]; 9] {
    // Build HW state once for this frame (elevation from sun_dir)
    let elev = sun_dir
        .y
        .max(0.0)
        .asin()
        .clamp(0.0, std::f32::consts::FRAC_PI_2);
    let sky = SkyState::new(&SkyParams {
        elevation: elev,
        turbidity: weather.turbidity,
        albedo: weather.ground_albedo,
    })
    .expect("valid HW params");
    // Integration over the upper hemisphere (y>=0)
    let n_theta = 16usize;
    let n_phi = 32usize;
    let dtheta = (std::f32::consts::FRAC_PI_2) / (n_theta as f32);
    let dphi = (std::f32::consts::TAU) / (n_phi as f32);
    let mut c_r = [0.0f32; 9];
    let mut c_g = [0.0f32; 9];
    let mut c_b = [0.0f32; 9];
    let mut total_weight = 0.0f32;
    for it in 0..n_theta {
        let theta = (it as f32 + 0.5) * dtheta; // 0..pi/2
        let sin_t = theta.sin();
        let cos_t = theta.cos();
        for ip in 0..n_phi {
            let phi = (ip as f32 + 0.5) * dphi; // 0..2pi
            let dir = Vec3::new(sin_t * phi.cos(), cos_t, sin_t * phi.sin());
            // HW expects theta from zenith and gamma angle to sun
            let gamma = (dir.dot(sun_dir)).clamp(-1.0, 1.0).acos();
            let r = sky.radiance(theta, gamma, Channel::R);
            let g = sky.radiance(theta, gamma, Channel::G);
            let b = sky.radiance(theta, gamma, Channel::B);
            let y = sh_basis(dir);
            let w = sin_t * dtheta * dphi; // solid angle element
            total_weight += w;
            for i in 0..9 {
                c_r[i] += r * y[i] * w;
                c_g[i] += g * y[i] * w;
                c_b[i] += b * y[i] * w;
            }
        }
    }
    if total_weight > 0.0 {
        for i in 0..9 {
            c_r[i] /= 4.0 * std::f32::consts::PI;
            c_g[i] /= 4.0 * std::f32::consts::PI;
            c_b[i] /= 4.0 * std::f32::consts::PI;
        }
    }
    // Convolve with Lambert kernel: factors per band l=0,1,2.
    let k_l0 = std::f32::consts::PI; // π
    let k_l1 = 2.0 * std::f32::consts::PI / 3.0; // 2π/3
    let k_l2 = std::f32::consts::PI / 4.0; // π/4
    for i in 0..9 {
        let f = match i {
            0 => k_l0,     // l=0
            1..=3 => k_l1, // l=1 (3 coeffs)
            _ => k_l2,     // l=2 (5 coeffs)
        };
        c_r[i] *= f;
        c_g[i] *= f;
        c_b[i] *= f;
    }
    let mut out = [[0.0f32; 3]; 9];
    for i in 0..9 {
        out[i] = [c_r[i], c_g[i], c_b[i]];
    }
    out
}

/// Real SH basis (l<=2) evaluated for direction d (x,y,z).
fn sh_basis(d: Vec3) -> [f32; 9] {
    let x = d.x;
    let y = d.y;
    let z = d.z;
    [
        0.282095,                       // L00
        0.488603 * y,                   // L1-1
        0.488603 * z,                   // L10
        0.488603 * x,                   // L11
        1.092548 * x * y,               // L2-2
        1.092548 * y * z,               // L2-1
        0.315392 * (3.0 * z * z - 1.0), // L20
        1.092548 * x * z,               // L21
        0.546274 * (x * x - y * y),     // L22
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sun_dir_mapping_cycles() {
        let d0 = sun_dir_from_day_frac(0.0);
        let d5 = sun_dir_from_day_frac(0.5);
        // Night vs noon y signs differ
        assert!(d0.y < 0.0);
        assert!(d5.y > 0.0);
    }

    #[test]
    fn sh_projection_outputs9() {
        let w = Weather::default();
        let sh = project_irradiance_sh9(vec3(0.0, 1.0, 0.0), &w);
        assert_eq!(sh.len(), 9);
        // Basic sanity: L00 should be positive
        assert!(sh[0][0] > 0.0);
    }
}
