//! voxel_upload: helpers to upload/remove chunk meshes to GPU buffers.

use crate::gfx::{VoxelChunkMesh, types::Vertex};
use anyhow::{Context, Result};
use ecs_core::components::MeshCpu;
use std::collections::HashMap;
use wgpu::util::DeviceExt;

#[inline]
#[allow(dead_code)]
pub fn chunk_key(
    did: crate::gfx::DestructibleId,
    c: (u32, u32, u32),
) -> (crate::gfx::DestructibleId, u32, u32, u32) {
    (did, c.0, c.1, c.2)
}

#[allow(dead_code)]
pub fn upload_chunk_mesh(
    device: &wgpu::Device,
    did: crate::gfx::DestructibleId,
    chunk: (u32, u32, u32),
    mesh: &MeshCpu,
    out_meshes: &mut HashMap<(crate::gfx::DestructibleId, u32, u32, u32), VoxelChunkMesh>,
    out_hashes: &mut HashMap<(crate::gfx::DestructibleId, u32, u32, u32), u64>,
) -> Result<()> {
    mesh.validate().context("mesh validate")?;
    if mesh.indices.is_empty() {
        remove_chunk_mesh(did, chunk, out_meshes, out_hashes);
        return Ok(());
    }
    let mut verts: Vec<Vertex> = Vec::with_capacity(mesh.positions.len());
    for (i, p) in mesh.positions.iter().enumerate() {
        let n = mesh.normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
        verts.push(Vertex { pos: *p, nrm: n });
    }
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("voxel-chunk-vb"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("voxel-chunk-ib"),
        contents: bytemuck::cast_slice(&mesh.indices),
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
    });
    let key = chunk_key(did, chunk);
    out_meshes.insert(
        key,
        VoxelChunkMesh {
            vb,
            ib,
            idx: mesh.indices.len() as u32,
        },
    );
    // Lightweight content hash: positions and indices lengths xor as u64
    let h = (mesh.positions.len() as u64) ^ ((mesh.indices.len() as u64) << 32);
    out_hashes.insert(key, h);
    Ok(())
}

/// Upload a chunk mesh directly from voxel_mesh::MeshBuffers, avoiding intermediate clones.
#[allow(dead_code)]
pub fn upload_chunk_mesh_raw(
    device: &wgpu::Device,
    did: crate::gfx::DestructibleId,
    chunk: (u32, u32, u32),
    mb: &voxel_mesh::MeshBuffers,
    out_meshes: &mut HashMap<(crate::gfx::DestructibleId, u32, u32, u32), VoxelChunkMesh>,
    out_hashes: &mut HashMap<(crate::gfx::DestructibleId, u32, u32, u32), u64>,
) -> Result<()> {
    if mb.indices.is_empty() {
        remove_chunk_mesh(did, chunk, out_meshes, out_hashes);
        return Ok(());
    }
    let mut verts: Vec<Vertex> = Vec::with_capacity(mb.positions.len());
    for (i, p) in mb.positions.iter().enumerate() {
        let n = mb.normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
        verts.push(Vertex { pos: *p, nrm: n });
    }
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("voxel-chunk-vb"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("voxel-chunk-ib"),
        contents: bytemuck::cast_slice(&mb.indices),
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
    });
    let key = chunk_key(did, chunk);
    out_meshes.insert(
        key,
        VoxelChunkMesh {
            vb,
            ib,
            idx: mb.indices.len() as u32,
        },
    );
    let h = (mb.positions.len() as u64) ^ ((mb.indices.len() as u64) << 32);
    out_hashes.insert(key, h);
    Ok(())
}

#[allow(dead_code)]
pub fn remove_chunk_mesh(
    did: crate::gfx::DestructibleId,
    chunk: (u32, u32, u32),
    out_meshes: &mut HashMap<(crate::gfx::DestructibleId, u32, u32, u32), VoxelChunkMesh>,
    out_hashes: &mut HashMap<(crate::gfx::DestructibleId, u32, u32, u32), u64>,
) {
    let key = chunk_key(did, chunk);
    out_meshes.remove(&key);
    out_hashes.remove(&key);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mesh_validate_catches_mismatch() {
        let m = MeshCpu {
            positions: vec![[0.0, 0.0, 0.0]],
            normals: vec![],
            indices: vec![0, 1, 2],
        };
        assert!(m.validate().is_err());
    }
}
