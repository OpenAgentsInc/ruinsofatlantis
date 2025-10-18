//! Red Wyvern NPC: load configured model (GLTF/GLB), single instance.
//! Mirrors the Sorceress/DK pattern and reuses the wizard skinned pipeline.

use anyhow::{Context, Result};
use wgpu::util::DeviceExt;

use crate::gfx::types::Vertex as Vtx;
use crate::gfx::types::{InstanceSkin, VertexSkinned};
use roa_assets::gltf::load_gltf_mesh;
use roa_assets::skinning::{load_gltf_skinned, merge_gltf_animations};

pub struct WyvernAssets {
    pub cpu: roa_assets::types::SkinnedMeshCPU,
    pub vb: wgpu::Buffer,
    pub ib: wgpu::Buffer,
    pub index_count: u32,
}

pub fn load_assets(device: &wgpu::Device) -> Result<WyvernAssets> {
    // Prefer packed GLB; otherwise find any *.glb|*.gltf under red_wyvern
    let base_path = find_wyvern_model_path()
        .ok_or_else(|| anyhow::anyhow!("wyvern model not found under assets/models/red_wyvern"))?;
    // Mirror viewer: prepare path (pick decompressed alternates when applicable)
    let prepared = match roa_assets::util::prepare_gltf_path(&base_path) {
        Ok(p) => p,
        Err(_) => base_path.clone(),
    };
    log::info!(
        target: "wyvern",
        "wyvern: try skinned load from {} (exists={})",
        prepared.display(),
        prepared.exists()
    );
    let mut cpu = load_gltf_skinned(&prepared)
        .with_context(|| format!("load skinned wyvern: {}", prepared.display()))?;
    log::info!(
        target: "wyvern",
        "wyvern: skinned ok (verts={}, idx={}, joints={}, anims={})",
        cpu.vertices.len(),
        cpu.indices.len(),
        cpu.joints_nodes.len(),
        cpu.animations.len()
    );
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

/// Attempt to load an unskinned static mesh of the wyvern for fallback drawing.
/// Returns (vb, ib, index_count) with Vertex (pos,nrm) layout.
pub fn load_unskinned_static(device: &wgpu::Device) -> Option<(wgpu::Buffer, wgpu::Buffer, u32)> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    // Preferred names
    candidates.push(asset_path(
        "assets/models/red_wyvern/RedDragon2021.textured.glb",
    ));
    candidates.push(asset_path("assets/models/red_wyvern/RedDragon2021.glb"));
    // Decompressed alt
    candidates.push(asset_path(
        "assets/models/red_wyvern/RedDragon2021.decompressed.gltf",
    ));
    // Scan directory for any other *.glb|*.gltf under red_wyvern
    let root = asset_path("assets/models/red_wyvern");
    if root.exists() {
        if let Ok(rd) = std::fs::read_dir(&root) {
            for ent in rd.flatten() {
                let p = ent.path();
                if p.is_file() {
                    if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                        let e = ext.to_ascii_lowercase();
                        if e == "glb" || e == "gltf" {
                            candidates.push(p);
                        }
                    }
                }
            }
        }
    }
    for p in candidates.iter() {
        if !p.exists() {
            continue;
        }
        let prepared = roa_assets::util::prepare_gltf_path(p).unwrap_or_else(|_| p.clone());
        if let Ok(cpu) = load_gltf_mesh(&prepared) {
            let verts: Vec<Vtx> = cpu
                .vertices
                .into_iter()
                .map(|v| Vtx {
                    pos: v.pos,
                    nrm: v.nrm,
                })
                .collect();
            let ib_data: Vec<u16> = cpu.indices;
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("wyvern-static-vb"),
                contents: bytemuck::cast_slice(&verts),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("wyvern-static-ib"),
                contents: bytemuck::cast_slice(&ib_data),
                usage: wgpu::BufferUsages::INDEX,
            });
            log::info!(
                "wyvern: static fallback ok: {} (verts={}, idx={})",
                prepared.display(),
                verts.len(),
                ib_data.len()
            );
            return Some((vb, ib, ib_data.len() as u32));
        }
    }
    None
}

fn find_wyvern_model_path() -> Option<std::path::PathBuf> {
    // Preferred exact names
    let preferred = [
        asset_path("assets/models/red_wyvern/RedDragon2021.textured.glb"),
        asset_path("assets/models/red_wyvern/RedDragon2021.glb"),
        asset_path("assets/models/red_wyvern/RedDragon2021.decompressed.gltf"),
    ];
    for p in preferred.iter() {
        if p.exists() {
            return Some(p.clone());
        }
    }
    // Directory scan fallback
    let root = asset_path("assets/models/red_wyvern");
    if !root.exists() {
        return None;
    }
    let mut picks: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&root) {
        for ent in rd.flatten() {
            let p = ent.path();
            if !p.is_file() {
                continue;
            }
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                let e = ext.to_ascii_lowercase();
                if e == "glb" || e == "gltf" {
                    picks.push(p);
                }
            }
        }
    }
    picks.sort();
    picks.into_iter().next()
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
