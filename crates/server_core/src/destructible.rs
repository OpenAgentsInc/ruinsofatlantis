//! Voxel destructible helpers: grid raycast and carved debris spawn.
//!
//! Determinism
//! - Debris selection/jitter seeded by `(global_seed, impact_id)`; tests use fixed seeds.
//! - DDA guards against zero components and starting-inside-solid cases.

#![forbid(unsafe_code)]

use core_units::{Length, Mass};
use glam::{DVec3, UVec3, Vec3};
use rand::{Rng, SeedableRng, rngs::SmallRng};
use voxel_proxy::{RemovedVoxels, VoxelGrid};

/// Result of a raycast into a voxel grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RayHit {
    /// First solid voxel coordinate hit.
    pub voxel: UVec3,
}

/// Simple Amanatides & Woo DDA for axis-aligned voxel grids.
pub fn raycast_voxels(
    grid: &VoxelGrid,
    origin_m: DVec3,
    dir_m: DVec3,
    max_dist: Length,
) -> Option<RayHit> {
    let vm = grid.voxel_m().0;
    if dir_m.length_squared() <= 1e-18 {
        return None;
    }
    // Convert to voxel space
    let o = origin_m - grid.origin_m();
    let d = dir_m.normalize();
    let o_v = o / vm;
    let d_v = d;
    let mut x = o_v.x.floor() as i32;
    let mut y = o_v.y.floor() as i32;
    let mut z = o_v.z.floor() as i32;
    let dims = grid.dims();
    let inside = |x: i32, y: i32, z: i32| -> bool {
        x >= 0
            && y >= 0
            && z >= 0
            && (x as u32) < dims.x
            && (y as u32) < dims.y
            && (z as u32) < dims.z
    };
    // Early inside-solid
    if inside(x, y, z) && grid.is_solid(x as u32, y as u32, z as u32) {
        return Some(RayHit {
            voxel: UVec3::new(x as u32, y as u32, z as u32),
        });
    }
    let step_x = if d_v.x > 0.0 {
        1
    } else if d_v.x < 0.0 {
        -1
    } else {
        0
    };
    let step_y = if d_v.y > 0.0 {
        1
    } else if d_v.y < 0.0 {
        -1
    } else {
        0
    };
    let step_z = if d_v.z > 0.0 {
        1
    } else if d_v.z < 0.0 {
        -1
    } else {
        0
    };
    let inf = f64::INFINITY;
    let next_boundary = |p: f64, dir: i32| -> f64 {
        let f = p - p.floor();
        if dir > 0 { 1.0 - f } else { f }
    };
    let mut t_max_x = if step_x == 0 {
        inf
    } else {
        next_boundary(o_v.x, step_x) / d_v.x.abs()
    };
    let mut t_max_y = if step_y == 0 {
        inf
    } else {
        next_boundary(o_v.y, step_y) / d_v.y.abs()
    };
    let mut t_max_z = if step_z == 0 {
        inf
    } else {
        next_boundary(o_v.z, step_z) / d_v.z.abs()
    };
    let t_delta_x = if step_x == 0 { inf } else { 1.0 / d_v.x.abs() };
    let t_delta_y = if step_y == 0 { inf } else { 1.0 / d_v.y.abs() };
    let t_delta_z = if step_z == 0 { inf } else { 1.0 / d_v.z.abs() };

    let mut t = 0.0f64;
    let t_max = max_dist.0 / vm;
    let safety_steps = (dims.x as usize + dims.y as usize + dims.z as usize) * 4;
    for _ in 0..safety_steps {
        if t > t_max {
            break;
        }
        // step along the smallest t_max
        if t_max_x <= t_max_y && t_max_x <= t_max_z {
            x += step_x;
            t = t_max_x;
            t_max_x += t_delta_x;
        } else if t_max_y <= t_max_z {
            y += step_y;
            t = t_max_y;
            t_max_y += t_delta_y;
        } else {
            z += step_z;
            t = t_max_z;
            t_max_z += t_delta_z;
        }
        if !inside(x, y, z) {
            return None;
        }
        if grid.is_solid(x as u32, y as u32, z as u32) {
            return Some(RayHit {
                voxel: UVec3::new(x as u32, y as u32, z as u32),
            });
        }
    }
    None
}

/// Debris spawn summary for an impact.
#[derive(Debug, Clone)]
pub struct DebrisSpawn {
    pub positions_m: Vec<DVec3>,
    pub velocities_mps: Vec<DVec3>,
    pub masses: Vec<Mass>,
}

/// Carve a sphere and spawn debris from removed voxels. Selection and jitter are seeded.
pub fn carve_and_spawn_debris(
    grid: &mut VoxelGrid,
    impact_center_m: DVec3,
    radius: Length,
    global_seed: u64,
    impact_id: u64,
    max_debris: usize,
) -> DebrisSpawn {
    let removed: RemovedVoxels = voxel_proxy::carve_sphere(grid, impact_center_m, radius);
    let mut rng = SmallRng::seed_from_u64(hash64(global_seed, impact_id));
    let total = removed.centers_m.len();
    let take = total.min(max_debris);
    let mut positions = removed.centers_m;
    // Reservoir-sample a subset if needed
    if total > take {
        // Simple partial Fisher-Yates shuffle
        for i in 0..take {
            let j = rng.random_range(i..total);
            positions.swap(i, j);
        }
        positions.truncate(take);
    }
    let mut velocities = Vec::with_capacity(positions.len());
    let mut masses = Vec::with_capacity(positions.len());
    let voxel_m = grid.voxel_m();
    let mat = grid.meta().material;
    for p in &positions {
        let dir = (*p - impact_center_m).as_vec3();
        let base = if dir.length_squared() > 1e-12 {
            dir.normalize()
        } else {
            Vec3::Y
        };
        let jitter = Vec3::new(
            rng.random_range(-0.5..0.5),
            rng.random_range(0.0..0.5),
            rng.random_range(-0.5..0.5),
        );
        let v = (base + 0.35 * jitter).normalize_or_zero() * 6.0; // ~6 m/s burst
        velocities.push(v.as_dvec3());
        let m = core_materials::mass_for_voxel(mat, voxel_m).unwrap();
        masses.push(m);
    }
    DebrisSpawn {
        positions_m: positions,
        velocities_mps: velocities,
        masses,
    }
}

#[inline]
fn hash64(a: u64, b: u64) -> u64 {
    // xorshift-like mix; stable across platforms
    let mut x = a ^ 0x9E3779B97F4A7C15u64;
    x ^= b.wrapping_mul(0xBF58476D1CE4E5B9u64).rotate_left(31);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D049BB133111EBu64);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_materials::find_material_id;
    use voxel_proxy::{GlobalId, VoxelProxyMeta};

    fn mk_grid(d: UVec3, c: UVec3, vox_m: f64) -> VoxelGrid {
        let meta = VoxelProxyMeta {
            object_id: GlobalId(1),
            origin_m: DVec3::ZERO,
            voxel_m: Length::meters(vox_m),
            dims: d,
            chunk: c,
            material: find_material_id("stone").unwrap(),
        };
        VoxelGrid::new(meta)
    }

    #[test]
    fn dda_hits_axis_aligned_voxel() {
        let mut g = mk_grid(UVec3::new(16, 16, 16), UVec3::new(8, 8, 8), 1.0);
        g.set(5, 5, 5, true);
        let o = DVec3::new(0.0, 5.2, 5.2);
        let dir = DVec3::new(1.0, 0.0, 0.0);
        let hit = raycast_voxels(&g, o, dir, Length::meters(100.0)).unwrap();
        assert_eq!(hit.voxel, UVec3::new(5, 5, 5));
    }

    #[test]
    fn dda_diagonal_hits() {
        let mut g = mk_grid(UVec3::new(16, 16, 16), UVec3::new(8, 8, 8), 1.0);
        g.set(7, 7, 7, true);
        let o = DVec3::new(0.2, 0.2, 0.2);
        let dir = DVec3::new(1.0, 1.0, 1.0);
        let hit = raycast_voxels(&g, o, dir, Length::meters(100.0)).unwrap();
        assert_eq!(hit.voxel, UVec3::new(7, 7, 7));
    }

    #[test]
    fn carve_spawns_capped_debris_with_mass() {
        let mut g = mk_grid(UVec3::new(16, 16, 16), UVec3::new(8, 8, 8), 0.5);
        // Fill a 5x5x5 block around the center
        for z in 5..10 {
            for y in 5..10 {
                for x in 5..10 {
                    g.set(x, y, z, true);
                }
            }
        }
        let out = carve_and_spawn_debris(
            &mut g,
            DVec3::new(8.0, 8.0, 8.0),
            Length::meters(1.25),
            12345,
            1,
            50,
        );
        assert!(out.positions_m.len() <= 50);
        assert_eq!(out.positions_m.len(), out.velocities_mps.len());
        assert_eq!(out.positions_m.len(), out.masses.len());
        // Mass should be density * voxel^3; compare wood vs stone
        let wood = find_material_id("wood").unwrap();
        let stone = find_material_id("stone").unwrap();
        let mw = core_materials::mass_for_voxel(wood, g.voxel_m()).unwrap();
        let ms = core_materials::mass_for_voxel(stone, g.voxel_m()).unwrap();
        assert!(f64::from(ms) > f64::from(mw));
    }
}
