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

pub mod queue {
    //! Deterministic per-chunk work queues (remesh + colliders) with budgets.
    use glam::UVec3;
    use std::collections::BTreeSet;

    #[derive(Default)]
    pub struct ChunkQueue {
        set: BTreeSet<(u32, u32, u32)>,
    }

    impl ChunkQueue {
        pub fn new() -> Self {
            Self {
                set: BTreeSet::new(),
            }
        }
        pub fn len(&self) -> usize {
            self.set.len()
        }
        pub fn is_empty(&self) -> bool {
            self.set.is_empty()
        }
        pub fn enqueue_many<I: IntoIterator<Item = UVec3>>(&mut self, it: I) {
            for c in it.into_iter() {
                self.set.insert((c.x, c.y, c.z));
            }
        }
        pub fn pop_budget(&mut self, n: usize) -> Vec<UVec3> {
            let mut out = Vec::new();
            for _ in 0..n {
                if let Some(first) = self.set.iter().next().copied() {
                    self.set.take(&first);
                    out.push(UVec3::new(first.0, first.1, first.2));
                } else {
                    break;
                }
            }
            out
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn budget_yields_sorted_chunks() {
            let mut q = ChunkQueue::new();
            q.enqueue_many([
                UVec3::new(2, 0, 0),
                UVec3::new(1, 0, 0),
                UVec3::new(1, 0, 1),
            ]);
            let a = q.pop_budget(2);
            assert_eq!(a, vec![UVec3::new(1, 0, 0), UVec3::new(1, 0, 1)]);
            let b = q.pop_budget(2);
            assert_eq!(b, vec![UVec3::new(2, 0, 0)]);
        }
    }
}

pub mod config {
    //! Minimal flag parser for destructible demo configuration.
    use core_materials::{MaterialId, find_material_id};
    use core_units::Length;
    use glam::UVec3;

    #[derive(Debug, Clone)]
    pub struct DestructibleConfig {
        pub voxel_size_m: Length,
        pub chunk: UVec3,
        pub material: MaterialId,
        pub max_debris: usize,
        pub max_chunk_remesh: usize,
        pub close_surfaces: bool,
        pub profile: bool,
        pub seed: u64,
        pub debris_vs_world: bool,
        pub demo_grid: bool,
    }

    impl Default for DestructibleConfig {
        fn default() -> Self {
            Self {
                voxel_size_m: Length::meters(0.05),
                chunk: UVec3::new(32, 32, 32),
                material: find_material_id("stone").unwrap_or(MaterialId(0)),
                max_debris: 3000,
                max_chunk_remesh: 3,
                close_surfaces: false,
                profile: false,
                seed: 0xC0FFEE,
                debris_vs_world: false,
                demo_grid: false,
            }
        }
    }

    impl DestructibleConfig {
        pub fn from_args<I, S>(args: I) -> Self
        where
            I: IntoIterator<Item = S>,
            S: AsRef<str>,
        {
            let mut cfg = Self::default();
            let mut it = args.into_iter();
            use std::sync::{
                Once,
                atomic::{AtomicBool, Ordering},
            };
            static HELP_ONCE: Once = Once::new();
            static UNKNOWN_ONCE: AtomicBool = AtomicBool::new(false);
            while let Some(a) = it.next() {
                let a = a.as_ref();
                match a {
                    "--help-vox" => {
                        HELP_ONCE.call_once(|| {
                            eprintln!(
                                "--voxel-demo  --voxel-size <m>  --chunk-size <x y z>  --mat <name>  --max-debris <n>  --max-chunk-remesh <n>  --close-surfaces  --debris-vs-world  --seed <u64>"
                            );
                        });
                    }
                    "--voxel-size" => {
                        if let Some(v) = it.next()
                            && let Ok(f) = v.as_ref().parse::<f64>()
                        {
                            cfg.voxel_size_m = Length::meters(f);
                        }
                    }
                    "--chunk-size" => {
                        if let (Some(x), Some(y), Some(z)) = (it.next(), it.next(), it.next())
                            && let (Ok(x), Ok(y), Ok(z)) =
                                (x.as_ref().parse(), y.as_ref().parse(), z.as_ref().parse())
                        {
                            cfg.chunk = UVec3::new(x, y, z);
                        }
                    }
                    "--mat" => {
                        if let Some(n) = it.next()
                            && let Some(id) = find_material_id(n.as_ref())
                        {
                            cfg.material = id;
                        }
                    }
                    "--max-debris" => {
                        if let Some(v) = it.next()
                            && let Ok(n) = v.as_ref().parse()
                        {
                            cfg.max_debris = n;
                        }
                    }
                    "--max-chunk-remesh" => {
                        if let Some(v) = it.next()
                            && let Ok(n) = v.as_ref().parse()
                        {
                            cfg.max_chunk_remesh = n;
                        }
                    }
                    "--close-surfaces" => {
                        cfg.close_surfaces = true;
                    }
                    "--profile" => {
                        cfg.profile = true;
                    }
                    "--seed" => {
                        if let Some(v) = it.next()
                            && let Ok(n) = v.as_ref().parse()
                        {
                            cfg.seed = n;
                        }
                    }
                    "--debris-vs-world" => {
                        cfg.debris_vs_world = true;
                    }
                    "--voxel-demo" | "--voxel-grid" => {
                        cfg.demo_grid = true;
                    }
                    other => {
                        if other.starts_with("--vox") && !UNKNOWN_ONCE.swap(true, Ordering::Relaxed)
                        {
                            eprintln!("[vox] warning: unknown flag `{}` (use --help-vox)", other);
                        }
                    }
                }
            }
            cfg
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn parse_minimal_flags() {
            let args = [
                "--voxel-size",
                "0.1",
                "--chunk-size",
                "16",
                "16",
                "16",
                "--mat",
                "wood",
                "--max-debris",
                "42",
                "--close-surfaces",
                "--seed",
                "1234",
                "--voxel-demo",
            ];
            let c = DestructibleConfig::from_args(args);
            assert!((f64::from(c.voxel_size_m) - 0.1).abs() < 1e-12);
            assert_eq!(c.chunk, UVec3::new(16, 16, 16));
            assert_eq!(c.max_debris, 42);
            assert!(c.close_surfaces);
            assert_eq!(c.seed, 1234);
            assert!(c.demo_grid);
        }
    }
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
    fn dda_negative_step_boundary_case() {
        // Ray starts just left of a voxel boundary, stepping negative along X
        let mut g = mk_grid(UVec3::new(16, 16, 16), UVec3::new(8, 8, 8), 1.0);
        // Solid voxel at x=10
        g.set(10, 5, 5, true);
        let o = DVec3::new(10.999, 5.2, 5.2);
        let dir = DVec3::new(-1.0, 0.0, 0.0);
        let hit = raycast_voxels(&g, o, dir, Length::meters(100.0)).unwrap();
        assert_eq!(hit.voxel, UVec3::new(10, 5, 5));
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
