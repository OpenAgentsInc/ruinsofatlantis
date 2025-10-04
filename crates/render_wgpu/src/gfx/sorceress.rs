//! Sorceress NPC: UBC Female model, single idle instance.

use anyhow::{Context, Result};
use wgpu::util::DeviceExt;

use crate::gfx::types::{InstanceSkin, VertexSkinned};
use ra_assets::skinning::{load_gltf_skinned, merge_gltf_animations};

pub struct SorcAssets {
    pub cpu: ra_assets::types::SkinnedMeshCPU,
    pub vb: wgpu::Buffer,
    pub ib: wgpu::Buffer,
    pub index_count: u32,
}

pub fn load_assets(device: &wgpu::Device) -> Result<SorcAssets> {
    let model_path = "assets/models/ubc/godot/Superhero_Female.gltf";
    let mut cpu = load_gltf_skinned(&asset_path(model_path))
        .with_context(|| format!("load skinned {}", model_path))?;
    // Merge common animation library so Idle is present if missing
    let lib = asset_path("assets/anims/universal/AnimationLibrary.glb");
    if lib.exists() {
        let _ = merge_gltf_animations(&mut cpu, &lib);
    }
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
        label: Some("sorc-vb"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("sorc-ib"),
        contents: bytemuck::cast_slice(&cpu.indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    let index_count = cpu.indices.len() as u32;
    Ok(SorcAssets {
        cpu,
        vb,
        ib,
        index_count,
    })
}

pub fn build_instance_at(
    device: &wgpu::Device,
    pos: glam::Vec3,
) -> (wgpu::Buffer, Vec<InstanceSkin>, Vec<glam::Mat4>, u32) {
    let m = glam::Mat4::from_scale_rotation_translation(
        glam::Vec3::splat(1.0),
        glam::Quat::IDENTITY,
        pos,
    );
    let models = vec![m];
    let inst = InstanceSkin {
        model: m.to_cols_array_2d(),
        color: [1.0, 1.0, 1.0],
        selected: 0.0,
        palette_base: 0,
        _pad_inst: [0; 3],
    };
    let instances_cpu = vec![inst];
    let instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("sorc-instances"),
        contents: bytemuck::cast_slice(&instances_cpu),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });
    (instances, instances_cpu, models, 1)
}

fn asset_path(rel: &str) -> std::path::PathBuf {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../").join(rel);
    if ws.exists() { ws } else { here.join(rel) }
}
