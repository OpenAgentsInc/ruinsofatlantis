//! Castle: loads the `assets/models/castle.glb`, computes a ground-aligned
//! base offset and horizontal radius, and uploads VB/IB buffers for instanced
//! rendering. Scene placement is handled in `scene.rs` so we keep this module
//! focused on mesh upload and basic metrics.

use anyhow::Result;
use crate::gfx::types::Vertex;
use wgpu::util::DeviceExt;

pub struct CastleGpu {
    pub vb: wgpu::Buffer,
    pub ib: wgpu::Buffer,
    pub index_count: u32,
    pub base_offset: f32,
    pub radius: f32,
}

pub fn build_castle(device: &wgpu::Device) -> Result<CastleGpu> {
    let path = asset_path("assets/models/castle.glb");
    // Load with gltf::import to support 32-bit indices; flatten first mesh primitive
    let (doc, buffers, _images) = match gltf::import(&path) {
        Ok(ok) => ok,
        Err(e) => {
            log::warn!("castle mesh import FAILED; falling back to cube: {}", e);
            let (vb, ib, index_count) = super::mesh::create_cube(device);
            return Ok(CastleGpu { vb, ib, index_count, base_offset: 0.0, radius: 1.0 });
        }
    };
    // Merge all mesh primitives into one static VB/IB (handles multi‑material splits)
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices_u32: Vec<u32> = Vec::new();
    let mut any = false;
    for mesh in doc.meshes() {
        for prim in mesh.primitives() {
            // Only triangles are supported in this simple path; skip others
            if prim.mode() != gltf::mesh::Mode::Triangles {
                log::warn!("castle.glb: skipping primitive with non-triangle mode: {:?}", prim.mode());
                continue;
            }
            let reader = prim.reader(|b| buffers.get(b.index()).map(|bb| bb.0.as_slice()));
            let positions: Vec<[f32; 3]> = if let Some(iter) = reader.read_positions() {
                iter.collect()
            } else {
                continue;
            };
            let normals: Vec<[f32; 3]> = if let Some(iter) = reader.read_normals() {
                iter.collect()
            } else {
                vec![[0.0, 1.0, 0.0]; positions.len()]
            };
            let base = vertices.len() as u32;
            for (i, p) in positions.iter().enumerate() {
                let n = *normals.get(i).unwrap_or(&[0.0, 1.0, 0.0]);
                vertices.push(Vertex { pos: *p, nrm: n });
            }
            let idx_local: Vec<u32> = if let Some(read) = reader.read_indices() {
                use gltf::mesh::util::ReadIndices;
                match read {
                    ReadIndices::U8(it) => it.map(|x| x as u32).collect(),
                    ReadIndices::U16(it) => it.map(|x| x as u32).collect(),
                    ReadIndices::U32(it) => it.collect(),
                }
            } else {
                (0..positions.len() as u32).collect()
            };
            indices_u32.extend(idx_local.into_iter().map(|i| base + i));
            any = true;
        }
    }
    if !any {
        log::warn!("castle.glb: no triangle primitives; using cube fallback");
        let (vb, ib, index_count) = super::mesh::create_cube(device);
        return Ok(CastleGpu { vb, ib, index_count, base_offset: 0.0, radius: 1.0 });
    }

    // Compute metrics
    let mut min_y = f32::INFINITY;
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for v in &vertices {
        min_y = min_y.min(v.pos[1]);
        min_x = min_x.min(v.pos[0]);
        max_x = max_x.max(v.pos[0]);
        min_z = min_z.min(v.pos[2]);
        max_z = max_z.max(v.pos[2]);
    }
    let sx = (max_x - min_x).abs();
    let sz = (max_z - min_z).abs();
    let radius = 0.5 * sx.max(sz);
    let base_offset = (-min_y) - 0.05;

    // Upload VB/IB (32-bit index buffer)
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("castle-vb"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("castle-ib"),
        contents: bytemuck::cast_slice(&indices_u32),
        usage: wgpu::BufferUsages::INDEX,
    });
    log::info!(
        "castle mesh loaded (vtx={}, idx={}, radius={:.2})",
        vertices.len(),
        indices_u32.len(),
        radius
    );
    Ok(CastleGpu { vb, ib, index_count: indices_u32.len() as u32, base_offset, radius })
}

fn asset_path(rel: &str) -> std::path::PathBuf {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../").join(rel);
    if ws.exists() { ws } else { here.join(rel) }
}
