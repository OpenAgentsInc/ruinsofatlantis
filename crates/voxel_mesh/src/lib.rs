//! voxel_mesh: greedy meshing for voxel grids (CPU-only).
//!
//! Scope
//! - Convert a `voxel_proxy::VoxelGrid` occupancy to triangle mesh buffers by extracting
//!   faces on solid→empty boundaries.
//! - Greedy merge co-planar faces per slice to minimize quad count.
//!
//! Extending
//! - Per-chunk meshing for dirty sets (integrate with queue from voxel_proxy).
//! - Material IDs per quad for mixed-material P1.

#![forbid(unsafe_code)]

use glam::{UVec3, Vec3};
use voxel_proxy::VoxelGrid;

/// Simple mesh buffers (positions, normals, indices).
#[derive(Default, Clone)]
pub struct MeshBuffers {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

impl MeshBuffers {
    pub fn quad_count(&self) -> usize {
        self.indices.len() / 6
    }
}

/// Generate a greedy-meshed surface for the entire grid.
pub fn greedy_mesh_all(grid: &VoxelGrid) -> MeshBuffers {
    let d = grid.meta().dims;
    let vm = grid.meta().voxel_m.0 as f32;
    let origin = grid.meta().origin_m.as_vec3();
    let mut mesh = MeshBuffers::default();
    // Iterate three axes
    for axis in 0..3 {
        let (w, h, ld, step_w, step_h, step_ld) = plane_dims(axis, d);
        // For each layer along the axis
        for layer in 0..ld {
            // Build face mask for this slice for positive normal
            let mut mask = vec![false; (w * h) as usize];
            let mut mask_neg = vec![false; (w * h) as usize];
            for j in 0..h {
                for i in 0..w {
                    let (x, y, z) = to_xyz(axis, i, j, layer);
                    // neighbor in -axis
                    let (xn, yn, zn) = match axis {
                        0 => (x.saturating_sub(1), y, z),
                        1 => (x, y.saturating_sub(1), z),
                        _ => (x, y, z.saturating_sub(1)),
                    };
                    let here = if inside(d, x, y, z) {
                        grid.is_solid(x, y, z)
                    } else {
                        false
                    };
                    let there = if inside(d, xn, yn, zn) {
                        grid.is_solid(xn, yn, zn)
                    } else {
                        false
                    };
                    let m_idx = (i + j * w) as usize;
                    // Solid→empty: positive face; empty→solid: negative face
                    mask[m_idx] = here && !there;
                    mask_neg[m_idx] = !here && there;
                }
            }
            // Emit quads for both directions
            greedy_emit(&mut mesh, &mask, axis, layer, w, h, vm, origin, true);
            greedy_emit(&mut mesh, &mask_neg, axis, layer, w, h, vm, origin, false);
        }
    }
    mesh
}

/// Generate a meshed surface for a single chunk by filtering the full mesh to triangles
/// whose centroids belong to the given chunk coordinate. This prioritizes correctness
/// and chunk ownership of faces; optimized per-slice greedy meshing can layer on later.
pub fn greedy_mesh_chunk(grid: &VoxelGrid, chunk: UVec3) -> MeshBuffers {
    let all = greedy_mesh_all(grid);
    if all.indices.is_empty() {
        return all;
    }
    let (xr, yr, zr) = grid.chunk_bounds_voxels(chunk);
    let origin = grid.meta().origin_m.as_vec3();
    let vm = grid.meta().voxel_m.0 as f32;
    let csz = grid.meta().chunk;
    let mut keep = Vec::with_capacity(all.indices.len() / 3);
    // Select triangles whose centroid falls within the chunk voxel bounds
    for tri in all.indices.chunks_exact(3) {
        let p0 = Vec3::from(all.positions[tri[0] as usize]);
        let p1 = Vec3::from(all.positions[tri[1] as usize]);
        let p2 = Vec3::from(all.positions[tri[2] as usize]);
        let centroid = (p0 + p1 + p2) / 3.0;
        let v = (centroid - origin) / vm; // voxel coords (float)
        let vx = v.x.floor() as i32;
        let vy = v.y.floor() as i32;
        let vz = v.z.floor() as i32;
        if vx >= xr.start as i32
            && vx < xr.end as i32
            && vy >= yr.start as i32
            && vy < yr.end as i32
            && vz >= zr.start as i32
            && vz < zr.end as i32
        {
            keep.push([tri[0], tri[1], tri[2]]);
        }
    }
    if keep.is_empty() {
        return MeshBuffers::default();
    }
    // Reindex vertices to only include those referenced by kept triangles
    use std::collections::HashMap;
    let mut map: HashMap<u32, u32> = HashMap::new();
    let mut out = MeshBuffers::default();
    for tri in keep.into_iter() {
        for &old_i in &tri {
            let next = *map.entry(old_i).or_insert_with(|| {
                let p = all.positions[old_i as usize];
                let n = all.normals[old_i as usize];
                out.positions.push(p);
                out.normals.push(n);
                (out.positions.len() as u32) - 1
            });
            out.indices.push(next);
        }
    }
    out
}

fn plane_dims(axis: u32, d: UVec3) -> (u32, u32, u32, UVec3, UVec3, UVec3) {
    match axis {
        // (w,h,ld)
        0 => (
            d.y,
            d.z,
            d.x,
            UVec3::new(0, 1, 0),
            UVec3::new(0, 0, 1),
            UVec3::new(1, 0, 0),
        ),
        1 => (
            d.x,
            d.z,
            d.y,
            UVec3::new(1, 0, 0),
            UVec3::new(0, 0, 1),
            UVec3::new(0, 1, 0),
        ),
        _ => (
            d.x,
            d.y,
            d.z,
            UVec3::new(1, 0, 0),
            UVec3::new(0, 1, 0),
            UVec3::new(0, 0, 1),
        ),
    }
}

#[inline]
fn to_xyz(axis: u32, i: u32, j: u32, l: u32) -> (u32, u32, u32) {
    match axis {
        0 => (l, i, j),
        1 => (i, l, j),
        _ => (i, j, l),
    }
}

#[inline]
fn inside(d: UVec3, x: u32, y: u32, z: u32) -> bool {
    x < d.x && y < d.y && z < d.z
}

fn greedy_emit(
    mesh: &mut MeshBuffers,
    mask: &[bool],
    axis: u32,
    layer: u32,
    w: u32,
    h: u32,
    vm: f32,
    origin: Vec3,
    pos_normal: bool,
) {
    let mut skipped = vec![false; mask.len()];
    let normal = match (axis, pos_normal) {
        (0, true) => Vec3::X,
        (0, false) => -Vec3::X,
        (1, true) => Vec3::Y,
        (1, false) => -Vec3::Y,
        (2, true) => Vec3::Z,
        (2, false) => -Vec3::Z,
        _ => Vec3::Z,
    };
    let mut y = 0;
    while y < h {
        let mut x = 0;
        while x < w {
            let idx = (x + y * w) as usize;
            if mask[idx] && !skipped[idx] {
                // Find maximal width
                let mut w_run = 1;
                while x + w_run < w {
                    let ii = (x + w_run + y * w) as usize;
                    if mask[ii] && !skipped[ii] {
                        w_run += 1;
                    } else {
                        break;
                    }
                }
                // Find maximal height
                let mut h_run = 1;
                'outer: while y + h_run < h {
                    for xx in x..(x + w_run) {
                        let jj = (xx + (y + h_run) * w) as usize;
                        if !mask[jj] || skipped[jj] {
                            break 'outer;
                        }
                    }
                    h_run += 1;
                }
                // Mark visited
                for yy in y..(y + h_run) {
                    for xx in x..(x + w_run) {
                        skipped[(xx + yy * w) as usize] = true;
                    }
                }
                // Emit quad at rectangle [x..x+w_run), [y..y+h_run)
                emit_rect(mesh, axis, layer, x, y, w_run, h_run, vm, origin, normal);
                x += w_run; // advance past rect
            } else {
                x += 1;
            }
        }
        y += 1;
    }
}

fn emit_rect(
    mesh: &mut MeshBuffers,
    axis: u32,
    layer: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    vm: f32,
    origin: Vec3,
    normal: Vec3,
) {
    // Compute the 4 corners in voxel space at face plane
    let (ax_u, ax_v, ax_w) = match axis {
        0 => (Vec3::Y, Vec3::Z, Vec3::X),
        1 => (Vec3::X, Vec3::Z, Vec3::Y),
        _ => (Vec3::X, Vec3::Y, Vec3::Z),
    };
    // layer is along ax_w
    let base = ax_w * (layer as f32);
    let p0 = base + ax_u * (x as f32) + ax_v * (y as f32);
    let p1 = base + ax_u * ((x + w) as f32) + ax_v * (y as f32);
    let p2 = base + ax_u * ((x + w) as f32) + ax_v * ((y + h) as f32);
    let p3 = base + ax_u * (x as f32) + ax_v * ((y + h) as f32);
    let add = |mesh: &mut MeshBuffers, p: Vec3, n: Vec3| {
        let wm = origin + p * vm; // place on voxel edge lines in meters
        mesh.positions.push([wm.x, wm.y, wm.z]);
        mesh.normals.push([n.x, n.y, n.z]);
    };
    let i0 = mesh.positions.len() as u32;
    add(mesh, p0, normal);
    add(mesh, p1, normal);
    add(mesh, p2, normal);
    add(mesh, p3, normal);
    // Two triangles (i0,i1,i2) and (i0,i2,i3)
    mesh.indices
        .extend_from_slice(&[i0, i0 + 1, i0 + 2, i0, i0 + 2, i0 + 3]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_materials::find_material_id;
    use core_units::Length;
    use glam::{DVec3, UVec3};
    use voxel_proxy::{GlobalId, VoxelProxyMeta};

    fn mk_full_grid(d: UVec3) -> voxel_proxy::VoxelGrid {
        let meta = VoxelProxyMeta {
            object_id: GlobalId(1),
            origin_m: DVec3::ZERO,
            voxel_m: Length::meters(1.0),
            dims: d,
            chunk: UVec3::new(16, 16, 16),
            material: find_material_id("stone").unwrap(),
        };
        let mut g = voxel_proxy::VoxelGrid::new(meta);
        for z in 0..d.z {
            for y in 0..d.y {
                for x in 0..d.x {
                    g.set(x, y, z, true);
                }
            }
        }
        g
    }

    #[test]
    fn solid_cube_produces_six_quads() {
        let g = mk_full_grid(UVec3::new(3, 3, 3));
        let m = greedy_mesh_all(&g);
        assert_eq!(m.quad_count(), 6);
    }

    #[test]
    fn rod_1x1x8_produces_six_quads() {
        let mut g = mk_full_grid(UVec3::new(1, 1, 8));
        // Already filled by mk_full_grid; just mesh
        let m = greedy_mesh_all(&g);
        assert_eq!(m.quad_count(), 6);
    }

    #[test]
    fn single_voxel_produces_36_indices() {
        let meta = VoxelProxyMeta {
            object_id: GlobalId(1),
            origin_m: DVec3::ZERO,
            voxel_m: Length::meters(1.0),
            dims: UVec3::new(1, 1, 1),
            chunk: UVec3::new(1, 1, 1),
            material: find_material_id("stone").unwrap(),
        };
        let mut g = voxel_proxy::VoxelGrid::new(meta);
        g.set(0, 0, 0, true);
        let m = greedy_mesh_all(&g);
        assert_eq!(m.indices.len(), 36);
    }

    #[test]
    fn chunk_mesher_filters_to_correct_chunk() {
        // 32x16x16 with chunk size 16^3: voxel at x=17,y=2,z=3
        let meta = VoxelProxyMeta {
            object_id: GlobalId(1),
            origin_m: DVec3::ZERO,
            voxel_m: Length::meters(1.0),
            dims: UVec3::new(32, 16, 16),
            chunk: UVec3::new(16, 16, 16),
            material: find_material_id("stone").unwrap(),
        };
        let mut g = voxel_proxy::VoxelGrid::new(meta);
        g.set(17, 2, 3, true);
        let m0 = greedy_mesh_chunk(&g, UVec3::new(0, 0, 0));
        let m1 = greedy_mesh_chunk(&g, UVec3::new(1, 0, 0));
        assert_eq!(m0.indices.len(), 0);
        assert_eq!(m1.indices.len(), 36);
    }
}
