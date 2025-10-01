//! Rocks: load a GLB rock mesh and scatter instances over gentle slopes.
//! Uses the instanced static pipeline (same path as trees/ruins).
//!
//! Future: move counts/seeds into zone prototypes and support multiple rock types.

use anyhow::Result;
use ra_assets::gltf::load_gltf_mesh;
use wgpu::util::DeviceExt;

use super::terrain;

pub struct RocksGpu {
    pub instances: wgpu::Buffer,
    pub count: u32,
    pub vb: wgpu::Buffer,
    pub ib: wgpu::Buffer,
    pub index_count: u32,
}

pub fn build_rocks(
    device: &wgpu::Device,
    terrain_cpu: &terrain::TerrainCPU,
    zone_slug: &str,
    config: Option<(usize, u32)>,
) -> Result<RocksGpu> {
    // Simple procedural placement for now: moderate count, deterministic seed.
    let (count, seed) = config.unwrap_or((80usize, 0xA5F00Du32));
    let mut inst = place_rocks(terrain_cpu, count, seed);
    // Tint rocks light gray
    for i in &mut inst {
        i.color = [0.58, 0.58, 0.60];
        i.selected = 0.0;
    }
    let instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rocks-instances"),
        contents: bytemuck::cast_slice(&inst),
        usage: wgpu::BufferUsages::VERTEX,
    });

    // Load mesh: prefer `assets/models/rock.glb` placed in repo assets.
    let rock_path = asset_path("assets/models/rock.glb");
    let (vb, ib, index_count) = match load_gltf_mesh(&rock_path) {
        Ok(cpu) => {
            log::info!(
                "rocks mesh loaded (vtx={}, idx={})",
                cpu.vertices.len(),
                cpu.indices.len()
            );
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("rocks-vb"),
                contents: bytemuck::cast_slice(&cpu.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("rocks-ib"),
                contents: bytemuck::cast_slice(&cpu.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            (vb, ib, cpu.indices.len() as u32)
        }
        Err(e) => {
            log::warn!("failed to load rock mesh; falling back to cube: {}", e);
            super::mesh::create_cube(device)
        }
    };

    // Optionally, verify snapshot fingerprints (not used for rocks yet)
    let _ = zone_slug; // reserved for future baked rock snapshots

    Ok(RocksGpu {
        instances,
        count: inst.len() as u32,
        vb,
        ib,
        index_count,
    })
}

fn place_rocks(cpu: &terrain::TerrainCPU, count: usize, seed: u32) -> Vec<super::types::Instance> {
    use glam::{Mat4, Quat, Vec3};
    let mut out = Vec::with_capacity(count);
    let mut s = splitmix(seed as u64);
    let center_excl = cpu.extent * 0.18; // keep spawn area clearer
    for _ in 0..count {
        let x = (rand01(&mut s) * 2.0 - 1.0) * cpu.extent;
        let z = (rand01(&mut s) * 2.0 - 1.0) * cpu.extent;
        if x.abs() < center_excl && z.abs() < center_excl {
            continue;
        }
        let (y, n) = terrain::height_at(cpu, x, z);
        if n.y < 0.85 {
            // avoid steeper slopes for rocks too
            continue;
        }
        let yaw = (rand01(&mut s) * 2.0 - 1.0) * std::f32::consts::PI;
        let sxy = 0.75 + rand01(&mut s) * 0.8; // some size variety
        let model = Mat4::from_scale_rotation_translation(
            Vec3::new(0.8 * sxy, 0.8 * sxy, 0.8 * sxy),
            Quat::from_rotation_y(yaw),
            Vec3::new(x, y, z),
        );
        out.push(super::types::Instance {
            model: model.to_cols_array_2d(),
            color: [0.6, 0.6, 0.62],
            selected: 0.0,
        });
    }
    out
}

// Local, minimal RNG — reuse terrain’s helpers for determinism
fn splitmix(mut x: u64) -> impl FnMut() -> u64 {
    move || {
        x = x.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

fn rand01(s: &mut impl FnMut() -> u64) -> f32 {
    (s() >> 11) as f32 / ((1u64 << 53) as f32)
}

fn asset_path(rel: &str) -> std::path::PathBuf {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../").join(rel);
    if ws.exists() { ws } else { here.join(rel) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rocks_placement_deterministic() {
        let cpu = terrain::generate_cpu(33, 20.0, 4242);
        let a = place_rocks(&cpu, 25, 7);
        let b = place_rocks(&cpu, 25, 7);
        assert_eq!(a.len(), b.len());
        // Check first few matrices match exactly
        for i in 0..a.len().min(5) {
            assert_eq!(a[i].model, b[i].model);
        }
    }
}
