//! Red Wyvern NPC: load configured model (GLTF/GLB), single instance.
//! Mirrors the Sorceress/DK pattern and reuses the wizard skinned pipeline.

use anyhow::{Context, Result};
use wgpu::util::DeviceExt;

use crate::gfx::types::{InstanceSkin, VertexSkinned};
use roa_assets::skinning::{load_gltf_skinned, merge_gltf_animations};

pub struct WyvernAssets {
    pub cpu: roa_assets::types::SkinnedMeshCPU,
    pub vb: wgpu::Buffer,
    pub ib: wgpu::Buffer,
    pub index_count: u32,
}

pub fn load_assets(device: &wgpu::Device) -> Result<WyvernAssets> {
    // Prefer packed textured GLB; fall back to original if present
    let candidates = [
        "assets/models/red_wyvern/RedDragon2021.textured.glb",
        "assets/models/red_wyvern/RedDragon2021.glb",
    ];
    let mut model_path = None;
    for c in candidates {
        let p = asset_path(c);
        if p.exists() {
            model_path = Some(p);
            break;
        }
    }
    let model_path = model_path
        .ok_or_else(|| anyhow::anyhow!("wyvern model not found in assets/models/red_wyvern"))?;

    let mut cpu = load_gltf_skinned(&model_path)
        .with_context(|| format!("load skinned {}", model_path.display()))?;
    // Optional: merge animation library if present
    let anim_libs = [
        asset_path("assets/anims/converted/RedDragon2021.glb"),
        asset_path("assets/anims/dragons/RedDragon2021.fbx"),
    ];
    for lib in anim_libs.iter() {
        if lib.exists() {
            let _ = merge_gltf_animations(&mut cpu, lib);
        }
    }
    let verts: Vec<VertexSkinned> = cpu
        .vertices
        .iter()
        .map(|v| VertexSkinned {
            pos: v.pos,
            nrm: v.nrm,
            joints: [
                v.joints[0] as u32,
                v.joints[1] as u32,
                v.joints[2] as u32,
                v.joints[3] as u32,
            ],
            weights: v.weights,
            uv: v.uv,
        })
        .collect();
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("wyvern-vb"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("wyvern-ib"),
        contents: bytemuck::cast_slice(&cpu.indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    let index_count = cpu.indices.len() as u32;
    Ok(WyvernAssets {
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
    // Apply -90° X rotation into the instance model matrix for viewer parity
    let rot_x = glam::Quat::from_rotation_x(-90f32.to_radians());
    let m = glam::Mat4::from_scale_rotation_translation(glam::Vec3::splat(1.0), rot_x, pos);
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
        label: Some("wyvern-instances"),
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
