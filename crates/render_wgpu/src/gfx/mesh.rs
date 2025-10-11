//! CPU-side mesh helpers used to create simple vertex/index buffers.
//!
//! Everything here is intentionally minimal: a unit cube and a large XZ plane.

use crate::gfx::types::{Vertex, VertexPosNrmUv};
use wgpu::util::DeviceExt;

#[allow(dead_code)] // Kept for quick test geometry and future debug paths
pub fn create_cube(device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer, u32) {
    let p = 0.5f32;
    let vertices = [
        // +X
        ([p, -p, -p], [1.0, 0.0, 0.0]),
        ([p, p, -p], [1.0, 0.0, 0.0]),
        ([p, p, p], [1.0, 0.0, 0.0]),
        ([p, -p, p], [1.0, 0.0, 0.0]),
        // -X
        ([-p, -p, p], [-1.0, 0.0, 0.0]),
        ([-p, p, p], [-1.0, 0.0, 0.0]),
        ([-p, p, -p], [-1.0, 0.0, 0.0]),
        ([-p, -p, -p], [-1.0, 0.0, 0.0]),
        // +Y
        ([-p, p, -p], [0.0, 1.0, 0.0]),
        ([p, p, -p], [0.0, 1.0, 0.0]),
        ([p, p, p], [0.0, 1.0, 0.0]),
        ([-p, p, p], [0.0, 1.0, 0.0]),
        // -Y
        ([-p, -p, p], [0.0, -1.0, 0.0]),
        ([p, -p, p], [0.0, -1.0, 0.0]),
        ([p, -p, -p], [0.0, -1.0, 0.0]),
        ([-p, -p, -p], [0.0, -1.0, 0.0]),
        // +Z
        ([-p, -p, p], [0.0, 0.0, 1.0]),
        ([p, -p, p], [0.0, 0.0, 1.0]),
        ([p, p, p], [0.0, 0.0, 1.0]),
        ([-p, p, p], [0.0, 0.0, 1.0]),
        // -Z
        ([p, -p, -p], [0.0, 0.0, -1.0]),
        ([-p, -p, -p], [0.0, 0.0, -1.0]),
        ([-p, p, -p], [0.0, 0.0, -1.0]),
        ([p, p, -p], [0.0, 0.0, -1.0]),
    ];
    let verts: Vec<Vertex> = vertices
        .iter()
        .map(|(p, n)| Vertex { pos: *p, nrm: *n })
        .collect();
    let indices: [u16; 36] = [
        0, 1, 2, 0, 2, 3, // +X
        4, 5, 6, 4, 6, 7, // -X
        8, 9, 10, 8, 10, 11, // +Y
        12, 13, 14, 12, 14, 15, // -Y
        16, 17, 18, 16, 18, 19, // +Z
        20, 21, 22, 20, 22, 23, // -Z
    ];
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("cube-vb"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("cube-ib"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    (vb, ib, indices.len() as u32)
}

#[allow(dead_code)]
pub fn create_plane(device: &wgpu::Device, extent: f32) -> (wgpu::Buffer, wgpu::Buffer, u32) {
    // A large XZ plane centered at origin
    let s = extent;
    let verts = [
        Vertex {
            pos: [-s, 0.0, -s],
            nrm: [0.0, 1.0, 0.0],
        },
        Vertex {
            pos: [s, 0.0, -s],
            nrm: [0.0, 1.0, 0.0],
        },
        Vertex {
            pos: [s, 0.0, s],
            nrm: [0.0, 1.0, 0.0],
        },
        Vertex {
            pos: [-s, 0.0, s],
            nrm: [0.0, 1.0, 0.0],
        },
    ];
    let idx: [u16; 6] = [0, 1, 2, 0, 2, 3];
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("plane-vb"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("plane-ib"),
        contents: bytemuck::cast_slice(&idx),
        usage: wgpu::BufferUsages::INDEX,
    });
    (vb, ib, idx.len() as u32)
}

/// Unit cube with position/normal/uv for textured pipelines.
/// UVs are mapped per-face to a full 0..1 quad.
#[allow(dead_code)]
pub fn create_uv_cube(device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer, u32) {
    let p = 0.5f32;
    // Per-face vertices (quad) with simple planar UVs
    let faces: [([f32; 3], [f32; 3], [f32; 2]); 24] = [
        // +X
        ([p, -p, -p], [1.0, 0.0, 0.0], [0.0, 0.0]),
        ([p, p, -p], [1.0, 0.0, 0.0], [0.0, 1.0]),
        ([p, p, p], [1.0, 0.0, 0.0], [1.0, 1.0]),
        ([p, -p, p], [1.0, 0.0, 0.0], [1.0, 0.0]),
        // -X
        ([-p, -p, p], [-1.0, 0.0, 0.0], [0.0, 0.0]),
        ([-p, p, p], [-1.0, 0.0, 0.0], [0.0, 1.0]),
        ([-p, p, -p], [-1.0, 0.0, 0.0], [1.0, 1.0]),
        ([-p, -p, -p], [-1.0, 0.0, 0.0], [1.0, 0.0]),
        // +Y
        ([-p, p, -p], [0.0, 1.0, 0.0], [0.0, 0.0]),
        ([p, p, -p], [0.0, 1.0, 0.0], [1.0, 0.0]),
        ([p, p, p], [0.0, 1.0, 0.0], [1.0, 1.0]),
        ([-p, p, p], [0.0, 1.0, 0.0], [0.0, 1.0]),
        // -Y
        ([-p, -p, p], [0.0, -1.0, 0.0], [0.0, 0.0]),
        ([p, -p, p], [0.0, -1.0, 0.0], [1.0, 0.0]),
        ([p, -p, -p], [0.0, -1.0, 0.0], [1.0, 1.0]),
        ([-p, -p, -p], [0.0, -1.0, 0.0], [0.0, 1.0]),
        // +Z
        ([-p, -p, p], [0.0, 0.0, 1.0], [0.0, 0.0]),
        ([p, -p, p], [0.0, 0.0, 1.0], [1.0, 0.0]),
        ([p, p, p], [0.0, 0.0, 1.0], [1.0, 1.0]),
        ([-p, p, p], [0.0, 0.0, 1.0], [0.0, 1.0]),
        // -Z
        ([p, -p, -p], [0.0, 0.0, -1.0], [0.0, 0.0]),
        ([-p, -p, -p], [0.0, 0.0, -1.0], [1.0, 0.0]),
        ([-p, p, -p], [0.0, 0.0, -1.0], [1.0, 1.0]),
        ([p, p, -p], [0.0, 0.0, -1.0], [0.0, 1.0]),
    ];
    let verts: Vec<VertexPosNrmUv> = faces
        .iter()
        .map(|(p, n, uv)| VertexPosNrmUv {
            pos: *p,
            nrm: *n,
            uv: *uv,
        })
        .collect();
    let indices: [u16; 36] = [
        0, 1, 2, 0, 2, 3, // +X
        4, 5, 6, 4, 6, 7, // -X
        8, 9, 10, 8, 10, 11, // +Y
        12, 13, 14, 12, 14, 15, // -Y
        16, 17, 18, 16, 18, 19, // +Z
        20, 21, 22, 20, 22, 23, // -Z
    ];
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("cube-uv-vb"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("cube-uv-ib"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    (vb, ib, indices.len() as u32)
}
