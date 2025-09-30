//! Death Knight assets and instance building.
//! Loads a skinned GLB (zombie-guy.glb), builds vertex/index buffers,
//! and constructs a single oversized instance placed in-scene.
//!
//! Scope
//! - Keep this minimal and data‑parallel to the existing `zombies` module so
//!   we can later generalize both into a common skinned group helper.
//! - This module intentionally mirrors function names used by `zombies.rs` so
//!   renderer init stays straightforward.

use anyhow::{Context, Result};
use wgpu::util::DeviceExt;

use crate::gfx::types::{InstanceSkin, VertexSkinned};
use ra_assets::skinning::load_gltf_skinned;

pub struct DeathKnightAssets {
    pub cpu: ra_assets::types::SkinnedMeshCPU,
    pub vb: wgpu::Buffer,
    pub ib: wgpu::Buffer,
    pub index_count: u32,
}

pub fn load_assets(device: &wgpu::Device) -> Result<DeathKnightAssets> {
    let model_path = "assets/models/zombie-guy.glb";
    let cpu = load_gltf_skinned(&asset_path(model_path))
        .with_context(|| format!("load skinned {}", model_path))?;

    let verts: Vec<VertexSkinned> = cpu
        .vertices
        .iter()
        .map(|v| VertexSkinned {
            pos: v.pos,
            nrm: v.nrm,
            joints: v.joints,
            weights: v.weights,
            uv: v.uv,
        })
        .collect();
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("deathknight-vb"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("deathknight-ib"),
        contents: bytemuck::cast_slice(&cpu.indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    let index_count = cpu.indices.len() as u32;
    Ok(DeathKnightAssets {
        cpu,
        vb,
        ib,
        index_count,
    })
}

/// Build a single instance for the Death Knight.
/// Placement: in front of the PC/wizard circle along +Z at a comfortable distance.
pub fn build_instances(
    device: &wgpu::Device,
    terrain_cpu: &crate::gfx::terrain::TerrainCPU,
    _joints: u32,
) -> (wgpu::Buffer, Vec<InstanceSkin>, Vec<glam::Mat4>, u32) {
    let radius = 114.75f32; // 15% closer than 135m
    // Sample terrain height under desired spot
    let (h, _n) = crate::gfx::terrain::height_at(terrain_cpu, 0.0, radius);
    let pos = glam::vec3(0.0, h, radius);
    let scale = glam::Vec3::splat(2.5); // 2.5x wizard size (50% smaller than before)
    let m = glam::Mat4::from_scale_rotation_translation(scale, glam::Quat::IDENTITY, pos);
    let mut instances_cpu: Vec<InstanceSkin> = Vec::new();
    let models: Vec<glam::Mat4> = vec![m];
    instances_cpu.push(InstanceSkin {
        model: m.to_cols_array_2d(),
        color: [1.0, 1.0, 1.0],
        selected: 0.0,
        palette_base: 0, // single instance at base 0
        _pad_inst: [0; 3],
    });
    let instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("deathknight-instances"),
        contents: bytemuck::cast_slice(&instances_cpu),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });
    (instances, instances_cpu, models, 1)
}

// If forward offset becomes necessary (e.g., for facing alignment),
// we can add a helper similar to zombies::forward_offset.

fn asset_path(rel: &str) -> std::path::PathBuf {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../").join(rel);
    if ws.exists() { ws } else { here.join(rel) }
}
