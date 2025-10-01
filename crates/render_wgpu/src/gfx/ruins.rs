//! Ruins: loads the ruins glTF, computes a ground-aligned base offset + radius,
//! and uploads VB/IB buffers for instanced rendering.
//!
//! The scene builder handles placement of ruins instances; this module focuses
//! on mesh upload and basic metrics needed for terrain alignment/tilt.

use anyhow::Result;
use ra_assets::gltf::load_gltf_mesh;
use wgpu::util::DeviceExt;

pub struct RuinsGpu {
    pub vb: wgpu::Buffer,
    pub ib: wgpu::Buffer,
    pub index_count: u32,
    pub base_offset: f32,
    pub radius: f32,
}

pub fn build_ruins(device: &wgpu::Device) -> Result<RuinsGpu> {
    let path = asset_path("assets/models/ruins.gltf");
    let ruins_cpu = match load_gltf_mesh(&path) {
        Ok(m) => m,
        Err(e) => {
            log::warn!("ruins mesh load FAILED; falling back to cube: {}", e);
            // On wasm, attempt to use rock.glb as a visible placeholder mesh
            // (better than cubes) so ruins are clearly present.
            #[cfg(target_arch = "wasm32")]
            {
                let rock_path = asset_path("assets/models/rock.glb");
                if let Ok(cpu) = load_gltf_mesh(&rock_path) {
                    log::info!(
                        "ruins fallback: using rock.glb (vtx={}, idx={})",
                        cpu.vertices.len(),
                        cpu.indices.len()
                    );
                    cpu
                } else {
                    // Build a small placeholder cube so the pipeline/bindings remain valid.
                    let (vb, ib, index_count) = super::mesh::create_cube(device);
                    return Ok(RuinsGpu {
                        vb,
                        ib,
                        index_count,
                        base_offset: 0.0,
                        radius: 1.0,
                    });
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Build a small placeholder cube so the pipeline/bindings remain valid.
                let (vb, ib, index_count) = super::mesh::create_cube(device);
                return Ok(RuinsGpu {
                    vb,
                    ib,
                    index_count,
                    base_offset: 0.0,
                    radius: 1.0,
                });
            }
        }
    };

    // Compute base offset so min Y sits slightly below ground, plus a horizontal radius.
    let mut min_y = f32::INFINITY;
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for v in &ruins_cpu.vertices {
        min_y = min_y.min(v.pos[1]);
        min_x = min_x.min(v.pos[0]);
        max_x = max_x.max(v.pos[0]);
        min_z = min_z.min(v.pos[2]);
        max_z = max_z.max(v.pos[2]);
    }
    let sx = (max_x - min_x).abs();
    let sz = (max_z - min_z).abs();
    let radius = 0.5 * sx.max(sz);
    let base_offset = (-min_y) - 0.05; // small embed to avoid hovering

    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ruins-vb"),
        contents: bytemuck::cast_slice(&ruins_cpu.vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ruins-ib"),
        contents: bytemuck::cast_slice(&ruins_cpu.indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    Ok(RuinsGpu {
        vb,
        ib,
        index_count: ruins_cpu.indices.len() as u32,
        base_offset,
        radius,
    })
}

fn asset_path(rel: &str) -> std::path::PathBuf {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../").join(rel);
    if ws.exists() { ws } else { here.join(rel) }
}
