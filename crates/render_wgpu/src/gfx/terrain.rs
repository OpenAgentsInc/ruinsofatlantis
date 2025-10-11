//! Terrain generation and zone baking (Phase 1)
//!
//! Scope
//! - Deterministic CPU heightmap generation (seeded), normals, and index buffer.
//! - Simple woodland placement: scatter “trees” on gentle slopes (returned as instance models).
//! - Persistence hooks: load/save a baked JSON zone when available (optional); generation is
//!   deterministic so saving is not required for reproducibility.
//!
//! Extension points
//! - Replace the noise with imported heightmaps, streaming tiles, biomes, and foliage meshes.
//! - Add texture splats (albedo/normal) and LOD.

use crate::gfx::types::{Instance, Vertex};
use anyhow::{Context, Result};
use glam::{Mat4, Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use wgpu::util::DeviceExt;

/// JSON structure for a baked zone file (minimal for now).
#[allow(dead_code)]
#[derive(Serialize, Deserialize)]
struct ZoneJson {
    size: u32,
    extent: f32,
    seed: u32,
}

pub struct TerrainBuffers {
    pub vb: wgpu::Buffer,
    pub ib: wgpu::Buffer,
    pub index_count: u32,
}

pub struct TerrainCPU {
    pub size: usize, // grid dimension (N x N vertices)
    pub extent: f32, // world-space half-extent (meters)
    pub heights: Vec<f32>,
    pub normals: Vec<[f32; 3]>,
}

/// Generate or load a deterministic heightmap and upload GPU buffers.
pub fn create_terrain(
    device: &wgpu::Device,
    size: usize,
    extent: f32,
    seed: u32,
) -> (TerrainCPU, TerrainBuffers) {
    let cpu = generate_heightmap(size, extent, seed);
    let bufs = upload_from_cpu(device, &cpu);
    (cpu, bufs)
}

/// CPU-only terrain generation for tools/baking.
pub fn generate_cpu(size: usize, extent: f32, seed: u32) -> TerrainCPU {
    generate_heightmap(size, extent, seed)
}

/// Create GPU buffers from a CPU terrain snapshot.
pub fn upload_from_cpu(device: &wgpu::Device, cpu: &TerrainCPU) -> TerrainBuffers {
    let (verts, indices) = build_mesh(cpu);
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terrain-vb"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terrain-ib"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    let index_count = indices.len() as u32;
    TerrainBuffers {
        vb,
        ib,
        index_count,
    }
}

/// Generate tree instance transforms using slope/height criteria (gentle slopes only).
pub fn place_trees(cpu: &TerrainCPU, count: usize, seed: u32) -> Vec<Instance> {
    let mut out: Vec<Instance> = Vec::with_capacity(count);
    let mut s = splitmix(seed as u64);
    let center_excl = cpu.extent * 0.25; // wider clearing near the player
    for _ in 0..count {
        let x = (rand01(&mut s) * 2.0 - 1.0) * cpu.extent;
        let z = (rand01(&mut s) * 2.0 - 1.0) * cpu.extent;
        if x.abs() < center_excl && z.abs() < center_excl {
            continue;
        }
        let p = Vec2::new(x, z);
        let (y, n) = sample_height_normal(cpu, p);
        if n.y < 0.85 {
            // avoid steep slopes
            continue;
        }
        let yaw = (rand01(&mut s) * 2.0 - 1.0) * std::f32::consts::PI;
        let sxy = 0.8 + rand01(&mut s) * 0.6; // slightly wider baseline
        let model = Mat4::from_scale_rotation_translation(
            // Shorter, fuller trees: broader XZ, lower Y
            Vec3::new(1.1 * sxy, 1.8 * sxy, 1.1 * sxy),
            glam::Quat::from_rotation_y(yaw),
            Vec3::new(x, y, z),
        );
        // Subtle per-instance color variation to avoid a flat look
        let tint = 0.9 + 0.2 * rand01(&mut s);
        let base = [0.16, 0.42, 0.20];
        out.push(Instance {
            model: model.to_cols_array_2d(),
            color: [base[0] * tint, base[1] * tint, base[2] * tint],
            selected: 0.0,
        });
    }
    out
}

/// Public sampler: get terrain height and normal at world XZ.
pub fn height_at(cpu: &TerrainCPU, x: f32, z: f32) -> (f32, glam::Vec3) {
    sample_height_normal(cpu, glam::Vec2::new(x, z))
}

// ----------------------
// Snapshot I/O (prototype, JSON)
// ----------------------

#[derive(Serialize, Deserialize)]
struct TerrainSnapshotJson {
    size: usize,
    extent: f32,
    seed: u32,
    heights: Vec<f32>,
}

#[derive(Serialize, Deserialize)]
struct TreesSnapshotJson {
    models: Vec<[[f32; 4]; 4]>,
    #[serde(default)]
    by_kind: std::collections::HashMap<String, Vec<[[f32; 4]; 4]>>,
}

fn zones_dir() -> PathBuf {
    // Prefer workspace-level data/zones when present so tools like zone-bake
    // (which write under the workspace root) are immediately picked up at runtime.
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../data/zones");
    if ws.is_dir() {
        ws
    } else {
        here.join("data").join("zones")
    }
}

fn snap_dir(slug: &str) -> PathBuf {
    zones_dir().join(slug).join("snapshot.v1")
}

/// Try loading a baked terrain snapshot from JSON; recompute normals on load.
pub fn load_terrain_snapshot(slug: &str) -> Option<TerrainCPU> {
    let path = snap_dir(slug).join("terrain.json");
    let txt = fs::read_to_string(&path).ok()?;
    let tj: TerrainSnapshotJson = serde_json::from_str(&txt).ok()?;
    if tj.heights.len() != tj.size * tj.size {
        log::warn!(
            "terrain snapshot has mismatched heights ({} != {}x{})",
            tj.heights.len(),
            tj.size,
            tj.size
        );
        return None;
    }
    let normals = compute_normals(tj.size, tj.extent, &tj.heights);
    Some(TerrainCPU {
        size: tj.size,
        extent: tj.extent,
        heights: tj.heights,
        normals,
    })
}

pub fn load_trees_snapshot(slug: &str) -> Option<Vec<[[f32; 4]; 4]>> {
    // 1) workspace data/zones/<slug>/snapshot.v1/trees.json
    let primary = snap_dir(slug).join("trees.json");
    if let Ok(txt) = fs::read_to_string(&primary)
        && let Ok(tj) = serde_json::from_str::<TreesSnapshotJson>(&txt)
    {
        return Some(tj.models);
    }
    // 2) packs/zones/<slug>/snapshot.v1/trees.json (fallback)
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let packs = here
        .join("../../packs/zones")
        .join(slug)
        .join("snapshot.v1")
        .join("trees.json");
    if let Ok(txt) = fs::read_to_string(&packs)
        && let Ok(tj) = serde_json::from_str::<TreesSnapshotJson>(&txt)
    {
        return Some(tj.models);
    }
    None
}

pub fn load_trees_snapshot_by_kind(
    slug: &str,
) -> Option<std::collections::HashMap<String, Vec<[[f32; 4]; 4]>>> {
    // 1) workspace
    let primary = snap_dir(slug).join("trees.json");
    if let Ok(txt) = fs::read_to_string(&primary)
        && let Ok(tj) = serde_json::from_str::<TreesSnapshotJson>(&txt)
        && !tj.by_kind.is_empty()
    {
        return Some(tj.by_kind);
    }
    // 2) packs fallback
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let packs = here
        .join("../../packs/zones")
        .join(slug)
        .join("snapshot.v1")
        .join("trees.json");
    if let Ok(txt) = fs::read_to_string(&packs)
        && let Ok(tj) = serde_json::from_str::<TreesSnapshotJson>(&txt)
        && !tj.by_kind.is_empty()
    {
        return Some(tj.by_kind);
    }
    None
}

pub fn write_terrain_snapshot(slug: &str, cpu: &TerrainCPU, seed: u32) -> Result<()> {
    let dir = snap_dir(slug);
    fs::create_dir_all(&dir).with_context(|| format!("mkdir {}", dir.display()))?;
    let sj = TerrainSnapshotJson {
        size: cpu.size,
        extent: cpu.extent,
        seed,
        heights: cpu.heights.clone(),
    };
    let path = dir.join("terrain.json");
    let txt = serde_json::to_string_pretty(&sj)?;
    fs::write(&path, txt).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn write_trees_snapshot(slug: &str, models: &[[[f32; 4]; 4]]) -> Result<()> {
    let dir = snap_dir(slug);
    fs::create_dir_all(&dir).with_context(|| format!("mkdir {}", dir.display()))?;
    let sj = TreesSnapshotJson {
        models: models.to_vec(),
        by_kind: std::collections::HashMap::new(),
    };
    let path = dir.join("trees.json");
    let txt = serde_json::to_string_pretty(&sj)?;
    fs::write(&path, txt).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Convert baked tree transforms to Instances with a consistent green color.
pub fn instances_from_models(models: &[[[f32; 4]; 4]]) -> Vec<Instance> {
    models
        .iter()
        .map(|m| Instance {
            model: *m,
            color: [0.14, 0.45, 0.18],
            selected: 0.0,
        })
        .collect()
}

// ----------------------
// Snapshot verification (fingerprints)
// ----------------------

#[derive(Deserialize)]
struct MetaTerrainMin {
    heights_fingerprint: u64,
}

#[derive(Deserialize)]
struct MetaTreesMin {
    fingerprint: u64,
}

#[derive(Deserialize)]
struct ZoneMetaMin {
    terrain: MetaTerrainMin,
    trees: MetaTreesMin,
}

fn fingerprint_heights(h: &[f32]) -> u64 {
    let mut hasher = DefaultHasher::new();
    h.len().hash(&mut hasher);
    for &v in h {
        v.to_bits().hash(&mut hasher);
    }
    hasher.finish()
}

fn fingerprint_models(mats: &[[[f32; 4]; 4]]) -> u64 {
    let mut hasher = DefaultHasher::new();
    mats.len().hash(&mut hasher);
    for m in mats {
        for row in m {
            for &v in row {
                v.to_bits().hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

/// Returns Some(true/false) if meta exists and could be verified; None if meta missing.
pub fn verify_snapshot_fingerprints(
    slug: &str,
    cpu: &TerrainCPU,
    tree_models: Option<&[[[f32; 4]; 4]]>,
) -> Option<bool> {
    let path = snap_dir(slug).join("zone_meta.json");
    let txt = fs::read_to_string(&path).ok()?;
    let meta: ZoneMetaMin = serde_json::from_str(&txt).ok()?;
    let ok_h = meta.terrain.heights_fingerprint == fingerprint_heights(&cpu.heights);
    let ok_t = if let Some(models) = tree_models {
        meta.trees.fingerprint == fingerprint_models(models)
    } else {
        true
    };
    Some(ok_h && ok_t)
}

fn generate_heightmap(size: usize, extent: f32, seed: u32) -> TerrainCPU {
    let mut heights = vec![0.0f32; size * size];
    let freq = 1.0 / 50.0; // meters → frequency
    let mut s = splitmix(seed as u64);
    let o1 = rand01(&mut s) * 1000.0;
    let o2 = rand01(&mut s) * 1000.0;
    let o3 = rand01(&mut s) * 1000.0;
    for j in 0..size {
        for i in 0..size {
            let x = ((i as f32) / (size as f32 - 1.0) * 2.0 - 1.0) * extent;
            let z = ((j as f32) / (size as f32 - 1.0) * 2.0 - 1.0) * extent;
            // fBm: 3 octaves of value noise, gentle hills
            let h = 8.0
                * (value_noise_2d((x + o1) * freq, (z + o1) * freq, seed)
                    + 0.5
                        * value_noise_2d(
                            (x + o2) * freq * 2.0,
                            (z + o2) * freq * 2.0,
                            seed ^ 0x9E37,
                        )
                    + 0.25
                        * value_noise_2d(
                            (x + o3) * freq * 4.0,
                            (z + o3) * freq * 4.0,
                            seed ^ 0xA2B3,
                        ))
                / (1.0 + 0.5 + 0.25);
            heights[j * size + i] = h;
        }
    }
    // Compute normals via central differences
    let normals = compute_normals(size, extent, &heights);
    TerrainCPU {
        size,
        extent,
        heights,
        normals,
    }
}

fn build_mesh(cpu: &TerrainCPU) -> (Vec<Vertex>, Vec<u16>) {
    let n = cpu.size;
    let mut verts = Vec::with_capacity(n * n);
    for j in 0..n {
        for i in 0..n {
            let x = ((i as f32) / (n as f32 - 1.0) * 2.0 - 1.0) * cpu.extent;
            let z = ((j as f32) / (n as f32 - 1.0) * 2.0 - 1.0) * cpu.extent;
            let y = cpu.heights[j * n + i];
            let nrm = cpu.normals[j * n + i];
            verts.push(Vertex {
                pos: [x, y, z],
                nrm,
            });
        }
    }
    let quads = (n - 1) * (n - 1);
    let mut indices: Vec<u16> = Vec::with_capacity(quads * 6);
    for j in 0..(n - 1) {
        for i in 0..(n - 1) {
            let i0 = (j * n + i) as u32;
            let i1 = (j * n + (i + 1)) as u32;
            let i2 = ((j + 1) * n + i) as u32;
            let i3 = ((j + 1) * n + (i + 1)) as u32;
            for &idx in &[i0, i2, i1, i1, i2, i3] {
                assert!(idx <= u16::MAX as u32, "terrain vertex index exceeds u16");
                indices.push(idx as u16);
            }
        }
    }
    (verts, indices)
}

fn compute_normals(size: usize, extent: f32, h: &[f32]) -> Vec<[f32; 3]> {
    let step = (2.0 * extent) / (size as f32 - 1.0);
    let mut nrm = vec![[0.0; 3]; size * size];
    let idx = |i: isize, j: isize| -> usize {
        let ii = i.clamp(0, (size - 1) as isize) as usize;
        let jj = j.clamp(0, (size - 1) as isize) as usize;
        jj * size + ii
    };
    for j in 0..size as isize {
        for i in 0..size as isize {
            let h_l = h[idx(i - 1, j)];
            let h_r = h[idx(i + 1, j)];
            let h_d = h[idx(i, j - 1)];
            let h_u = h[idx(i, j + 1)];
            // Gradient
            let sx = (h_r - h_l) / (2.0 * step);
            let sz = (h_u - h_d) / (2.0 * step);
            let n = Vec3::new(-sx, 1.0, -sz).normalize();
            nrm[(j as usize) * size + (i as usize)] = [n.x, n.y, n.z];
        }
    }
    nrm
}

fn sample_height_normal(cpu: &TerrainCPU, p: Vec2) -> (f32, Vec3) {
    // Convert world x,z to grid space
    let n = cpu.size as i32;
    let gx = ((p.x / cpu.extent) * 0.5 + 0.5) * (n as f32 - 1.0);
    let gz = ((p.y / cpu.extent) * 0.5 + 0.5) * (n as f32 - 1.0);
    let x0 = gx.floor() as i32;
    let z0 = gz.floor() as i32;
    let x1 = (x0 + 1).clamp(0, n - 1);
    let z1 = (z0 + 1).clamp(0, n - 1);
    let tx = (gx - x0 as f32).clamp(0.0, 1.0);
    let tz = (gz - z0 as f32).clamp(0.0, 1.0);
    let idx = |x: i32, z: i32| -> usize { (z as usize) * cpu.size + (x as usize) };
    let cx0 = x0.clamp(0, n - 1);
    let cz0 = z0.clamp(0, n - 1);
    let cx1 = x1.clamp(0, n - 1);
    let cz1 = z1.clamp(0, n - 1);
    let h00 = cpu.heights[idx(cx0, cz0)];
    let h10 = cpu.heights[idx(cx1, cz0)];
    let h01 = cpu.heights[idx(cx0, cz1)];
    let h11 = cpu.heights[idx(cx1, cz1)];
    let h0 = h00 * (1.0 - tx) + h10 * tx;
    let h1 = h01 * (1.0 - tx) + h11 * tx;
    let h = h0 * (1.0 - tz) + h1 * tz;
    // Normal: bilinear blend then normalize
    let n00 = Vec3::from_array(cpu.normals[idx(cx0, cz0)]);
    let n10 = Vec3::from_array(cpu.normals[idx(cx1, cz0)]);
    let n01 = Vec3::from_array(cpu.normals[idx(cx0, cz1)]);
    let n11 = Vec3::from_array(cpu.normals[idx(cx1, cz1)]);
    let n0 = n00.lerp(n10, tx);
    let n1 = n01.lerp(n11, tx);
    let n = n0.lerp(n1, tz).normalize();
    (h, n)
}

// ----------------------
// Deterministic utilities
// ----------------------

fn splitmix(mut z: u64) -> u64 {
    // Advance once before first use (so seed=0 != first state 0)
    z = z.wrapping_add(0x9E3779B97F4A7C15);
    z
}

fn next_u64(state: &mut u64) -> u64 {
    // Advance the splitmix64 state and return a scrambled value.
    // IMPORTANT: also write the advanced state back so successive calls differ.
    let mut z = *state;
    z = z.wrapping_add(0x9E3779B97F4A7C15);
    *state = z; // persist advance
    let mut x = z;
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
    x ^ (x >> 31)
}

fn rand01(state: &mut u64) -> f32 {
    (next_u64(state) as f64 / (u64::MAX as f64)) as f32
}

fn hash_i(i: i32, j: i32, seed: u32) -> f32 {
    // 2D integer hash → [0,1)
    let mut x = (i as u64).wrapping_mul(0x27d4_eb2d);
    x ^= (j as u64).wrapping_mul(0x1656_6791_9E37_79F9);
    x ^= (seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let u = x ^ (x >> 33);
    (u as f64 / (u64::MAX as f64)) as f32
}

fn value_noise_2d(x: f32, y: f32, seed: u32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let tx = x - xi as f32;
    let ty = y - yi as f32;
    // quintic smoothstep for C2 continuity
    let sx = tx * tx * tx * (tx * (tx * 6.0 - 15.0) + 10.0);
    let sy = ty * ty * ty * (ty * (ty * 6.0 - 15.0) + 10.0);
    let c00 = hash_i(xi, yi, seed);
    let c10 = hash_i(xi + 1, yi, seed);
    let c01 = hash_i(xi, yi + 1, seed);
    let c11 = hash_i(xi + 1, yi + 1, seed);
    let a = c00 * (1.0 - sx) + c10 * sx;
    let b = c01 * (1.0 - sx) + c11 * sx;
    // Map to [-1,1]
    ((a * (1.0 - sy) + b * sy) * 2.0) - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_deterministic() {
        let a = value_noise_2d(12.34, -56.78, 42);
        let b = value_noise_2d(12.34, -56.78, 42);
        assert!((a - b).abs() < 1e-6);
    }

    #[test]
    fn normals_are_unit_lengthish() {
        let cpu = generate_heightmap(33, 50.0, 7);
        for n in cpu.normals.iter() {
            let v = Vec3::from_array(*n);
            let len = v.length();
            assert!(len > 0.98 && len < 1.02, "normal not unit ({})", len);
        }
    }
}
