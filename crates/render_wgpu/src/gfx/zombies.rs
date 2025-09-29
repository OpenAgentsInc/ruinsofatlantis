//! Zombie assets and instance building.
//! Loads a skinned zombie GLB, builds vertex/index buffers, and constructs
//! instances from server NPC positions (snapped to terrain).

use anyhow::{Context, Result};
use wgpu::util::DeviceExt;

use crate::gfx::types::{InstanceSkin, VertexSkinned};
use ra_assets::skinning::{load_gltf_skinned, merge_gltf_animations};

pub struct ZombieAssets {
    pub cpu: ra_assets::types::SkinnedMeshCPU,
    pub vb: wgpu::Buffer,
    pub ib: wgpu::Buffer,
    pub index_count: u32,
}

pub fn load_assets(device: &wgpu::Device) -> Result<ZombieAssets> {
    let zombie_model_path = "assets/models/zombie.glb";
    let mut cpu = load_gltf_skinned(&asset_path(zombie_model_path))
        .with_context(|| format!("load skinned {}", zombie_model_path))?;

    // Optional external clips in assets/models/zombie_clips/*.glb
    for (_alias, file) in [
        ("Idle", "idle.glb"),
        ("Walk", "walk.glb"),
        ("Run", "run.glb"),
        ("Attack", "attack.glb"),
    ] {
        let p = asset_path(&format!("assets/models/zombie_clips/{}", file));
        if p.exists() {
            let _ = merge_gltf_animations(&mut cpu, &p);
        }
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
        label: Some("zombie-vb"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("zombie-ib"),
        contents: bytemuck::cast_slice(&cpu.indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    let index_count = cpu.indices.len() as u32;
    Ok(ZombieAssets {
        cpu,
        vb,
        ib,
        index_count,
    })
}

pub fn build_instances(
    device: &wgpu::Device,
    terrain_cpu: &crate::gfx::terrain::TerrainCPU,
    server: &server_core::ServerState,
    zombie_joints: u32,
) -> (
    wgpu::Buffer,
    Vec<InstanceSkin>,
    Vec<glam::Mat4>,
    Vec<server_core::NpcId>,
    u32,
) {
    let mut instances_cpu: Vec<InstanceSkin> = Vec::new();
    let mut models: Vec<glam::Mat4> = Vec::new();
    let mut ids: Vec<server_core::NpcId> = Vec::new();
    for (idx, npc) in server.npcs.iter().enumerate() {
        // Snap initial zombie spawn to terrain height
        let (h, _n) = crate::gfx::terrain::height_at(terrain_cpu, npc.pos.x, npc.pos.z);
        let pos = glam::vec3(npc.pos.x, h, npc.pos.z);
        let m = glam::Mat4::from_scale_rotation_translation(
            glam::Vec3::splat(1.0),
            glam::Quat::IDENTITY,
            pos,
        );
        models.push(m);
        ids.push(npc.id);
        instances_cpu.push(InstanceSkin {
            model: m.to_cols_array_2d(),
            color: [1.0, 1.0, 1.0],
            selected: 0.0,
            palette_base: (idx as u32) * zombie_joints,
            _pad_inst: [0; 3],
        });
    }
    let instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("zombie-instances"),
        contents: bytemuck::cast_slice(&instances_cpu),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });
    let count = instances_cpu.len() as u32;
    (instances, instances_cpu, models, ids, count)
}

pub fn forward_offset(cpu: &ra_assets::types::SkinnedMeshCPU) -> f32 {
    if let Some(root_ix) = cpu.root_node {
        let r = cpu
            .base_r
            .get(root_ix)
            .copied()
            .unwrap_or(glam::Quat::IDENTITY);
        let f = r * glam::Vec3::Z; // authoring forward
        f32::atan2(f.x, f.z) + std::f32::consts::PI
    } else {
        0.0
    }
}

fn asset_path(rel: &str) -> std::path::PathBuf {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../").join(rel);
    if ws.exists() { ws } else { here.join(rel) }
}
