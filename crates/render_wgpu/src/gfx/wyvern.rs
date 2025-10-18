//! Red Wyvern NPC: load configured model (GLTF/GLB), single instance.
//! Mirrors the Sorceress/DK pattern and reuses the wizard skinned pipeline.

use anyhow::{Context, Result};
use wgpu::util::DeviceExt;

use crate::gfx::types::Vertex as Vtx;
use crate::gfx::types::{InstanceSkin, VertexPosNrmUv, VertexSkinned};
use gltf as gltf_rs;
use roa_assets::gltf::load_gltf_mesh;
use roa_assets::skinning::{load_gltf_skinned, merge_gltf_animations};
use std::path::{Path, PathBuf};

pub struct WyvernAssets {
    pub cpu: roa_assets::types::SkinnedMeshCPU,
    pub vb: wgpu::Buffer,
    pub ib: wgpu::Buffer,
    pub index_count: u32,
}

pub fn load_assets(device: &wgpu::Device) -> Result<WyvernAssets> {
    // Prefer a model that actually contains a skin. Probe common candidates and pick the first
    // that yields joints_nodes > 0, mirroring the viewer's success criteria.
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    candidates.push(asset_path(
        "assets/models/red_wyvern/RedDragon2021.textured.glb",
    ));
    candidates.push(asset_path("assets/models/red_wyvern/RedDragon2021.glb"));
    candidates.push(asset_path(
        "assets/models/red_wyvern/RedDragon2021.decompressed.glb",
    ));
    candidates.push(asset_path(
        "assets/models/red_wyvern/RedDragon2021.decompressed.gltf",
    ));
    // Fallback: directory scan for any *.glb|*.gltf
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

    let mut chosen: Option<std::path::PathBuf> = None;
    let mut cpu_probe: Option<roa_assets::types::SkinnedMeshCPU> = None;
    for cand in candidates.iter() {
        if !cand.exists() {
            continue;
        }
        let prepared = roa_assets::util::prepare_gltf_path(cand).unwrap_or_else(|_| cand.clone());
        if !prepared.exists() {
            continue;
        }
        if let Ok(test) = load_gltf_skinned(&prepared) {
            if !test.joints_nodes.is_empty() && !test.indices.is_empty() {
                chosen = Some(prepared);
                cpu_probe = Some(test);
                break;
            }
        }
    }
    let prepared = chosen.ok_or_else(|| anyhow::anyhow!(
        "wyvern: no skinned model with joints found under assets/models/red_wyvern (tried textured/original/decompressed)"
    ))?;
    let mut cpu = cpu_probe.expect("probe must have loaded cpu");
    log::info!(
        target: "wyvern",
        "wyvern: skinned ok: {} (verts={}, idx={}, joints={}, anims={})",
        prepared.display(),
        cpu.vertices.len(),
        cpu.indices.len(),
        cpu.joints_nodes.len(),
        cpu.animations.len()
    );
    // Optional: merge animation libraries exactly like the model viewer.
    // Strategy: find <stem>.{glb,gltf,fbx} under converted/, dragons/, and anims/.
    let mut merged = 0usize;
    let mut stem = prepared
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    // Heuristic: strip common suffixes like ".textured" or ".decompressed"
    for suf in [".textured", ".decompressed"] {
        if let Some(pos) = stem.find(suf) {
            stem = stem[..pos].to_string();
            break;
        }
    }
    let search_dirs = [
        asset_path("assets/anims/converted"),
        asset_path("assets/anims/dragons"),
        asset_path("assets/anims"),
    ];
    let exts = ["glb", "gltf", "fbx"];
    for dir in search_dirs.iter() {
        for ext in exts.iter() {
            let cand = dir.join(format!("{}.{}", stem, ext));
            if !cand.exists() {
                continue;
            }
            let ok = if *ext == "fbx" {
                if let Some(conv) = try_convert_fbx_to_gltf(&cand) {
                    merge_gltf_animations(&mut cpu, &conv).ok()
                } else {
                    None
                }
            } else {
                merge_gltf_animations(&mut cpu, &cand).ok()
            };
            if let Some(k) = ok {
                merged += k;
            }
        }
    }
    if merged > 0 {
        log::info!(target: "wyvern", "merged {} animation clips", merged);
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

/// Try to convert an FBX file to GLB in assets/anims/converted, mirroring the viewer.
fn try_convert_fbx_to_gltf(src: &Path) -> Option<PathBuf> {
    let out_dir = asset_path("assets/anims/converted");
    let _ = std::fs::create_dir_all(&out_dir);
    let stem = src.file_stem()?.to_str()?;
    let out_path = out_dir.join(format!("{}.glb", stem));
    if out_path.exists() {
        return Some(out_path);
    }
    // Prefer fbx2gltf if available
    let try_cmd = |prog: &str, args: &[&str]| -> bool {
        std::process::Command::new(prog)
            .args(args)
            .status()
            .ok()
            .map(|s| s.success())
            .unwrap_or(false)
    };
    if try_cmd(
        "fbx2gltf",
        &[
            "-b",
            "-o",
            out_dir.to_str().unwrap_or("."),
            src.to_str().unwrap_or(""),
        ],
    ) && out_path.exists()
    {
        return Some(out_path);
    }
    // Fallback: assimp export
    if try_cmd(
        "assimp",
        &[
            "export",
            src.to_str().unwrap_or(""),
            out_path.to_str().unwrap_or(""),
        ],
    ) && out_path.exists()
    {
        return Some(out_path);
    }
    None
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

/// Unskinned textured loader: builds a VertexPosNrmUv VB from all primitives and returns an
/// optional baseColor texture (RGBA8, SRGB) extracted from the largest contributing primitive.
#[allow(dead_code)]
pub fn load_unskinned_textured(
    device: &wgpu::Device,
) -> Option<(wgpu::Buffer, wgpu::Buffer, u32, Option<(Vec<u8>, u32, u32)>)> {
    let base = super::wyvern::find_wyvern_model_path()?;
    let prepared = roa_assets::util::prepare_gltf_path(&base)
        .ok()
        .unwrap_or(base);
    let (doc, bufs, images) = gltf_rs::import(&prepared).ok()?;

    let mut verts: Vec<VertexPosNrmUv> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();
    let mut best_tex: Option<(Vec<u8>, u32, u32)> = None;
    let mut best_vert_count = 0usize;

    for mesh in doc.meshes() {
        for prim in mesh.primitives() {
            let reader = prim.reader(|b| bufs.get(b.index()).map(|bb| bb.0.as_slice()));
            let Some(pos_it) = reader.read_positions() else {
                continue;
            };
            let nrm_it = reader.read_normals();
            let uv_it = reader.read_tex_coords(0).map(|t| t.into_f32());

            let pos: Vec<[f32; 3]> = pos_it.collect();
            let nrm: Vec<[f32; 3]> = nrm_it
                .map(|it| it.collect())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; pos.len()]);
            let uv: Vec<[f32; 2]> = uv_it
                .map(|it| it.collect())
                .unwrap_or_else(|| vec![[0.5, 0.5]; pos.len()]);

            let base = verts.len() as u32;
            for i in 0..pos.len() {
                verts.push(VertexPosNrmUv {
                    pos: pos[i],
                    nrm: nrm[i],
                    uv: uv[i],
                });
            }
            let idx_u32: Vec<u32> = match reader.read_indices() {
                Some(gltf_rs::mesh::util::ReadIndices::U16(it)) => it.map(|v| v as u32).collect(),
                Some(gltf_rs::mesh::util::ReadIndices::U32(it)) => it.collect(),
                Some(gltf_rs::mesh::util::ReadIndices::U8(it)) => it.map(|v| v as u32).collect(),
                None => (0..pos.len() as u32).collect(),
            };
            for i in idx_u32 {
                let v = i + base;
                if let Ok(u) = u16::try_from(v) {
                    indices.push(u);
                }
            }

            // Track a plausible baseColor texture from the largest contributing primitive
            if pos.len() > best_vert_count {
                best_vert_count = pos.len();
                if let Some(texinfo) = prim
                    .material()
                    .pbr_metallic_roughness()
                    .base_color_texture()
                {
                    let tex = texinfo.texture();
                    let img_idx = tex.source().index();
                    if let Some(img) = images.get(img_idx) {
                        let (w, h) = (img.width, img.height);
                        let pixels = match img.format {
                            gltf::image::Format::R8G8B8A8 => img.pixels.clone(),
                            gltf::image::Format::R8G8B8 => {
                                let mut out = Vec::with_capacity((w * h * 4) as usize);
                                for c in img.pixels.chunks_exact(3) {
                                    out.extend_from_slice(&[c[0], c[1], c[2], 255]);
                                }
                                out
                            }
                            gltf::image::Format::R8 => {
                                let mut out = Vec::with_capacity((w * h * 4) as usize);
                                for &r in &img.pixels {
                                    out.extend_from_slice(&[r, r, r, 255]);
                                }
                                out
                            }
                            _ => img.pixels.clone(),
                        };
                        best_tex = Some((pixels, w, h));
                    }
                }
            }
        }
    }

    if indices.is_empty() || verts.is_empty() {
        return None;
    }

    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("wyvern-static-uv-vb"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("wyvern-static-uv-ib"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    log::info!(
        target: "wyvern",
        "wyvern: static textured ok: {} (verts={}, idx={}, tex={})",
        prepared.display(),
        verts.len(),
        indices.len(),
        best_tex.as_ref().map(|(_,w,h)| format!("{}x{}", w, h)).unwrap_or_else(|| "none".into())
    );
    Some((vb, ib, indices.len() as u32, best_tex))
}

/// Minimal reader: load only the first mesh primitive (positions/normals/indices),
/// clamped to u16 index range so we can draw something even for very large meshes.
pub fn load_unskinned_first_primitive(
    device: &wgpu::Device,
) -> Option<(wgpu::Buffer, wgpu::Buffer, u32)> {
    let base = super::wyvern::find_wyvern_model_path()?;
    let prepared = roa_assets::util::prepare_gltf_path(&base)
        .ok()
        .unwrap_or(base);
    let (doc, bufs, _imgs) = gltf_rs::import(&prepared).ok()?;
    let mesh = doc.meshes().next()?;
    let prim = mesh.primitives().next()?;
    let reader = prim.reader(|b| bufs.get(b.index()).map(|bb| bb.0.as_slice()));
    let pos: Vec<[f32; 3]> = reader.read_positions()?.collect();
    let nrm: Vec<[f32; 3]> = reader
        .read_normals()
        .map(|it| it.collect())
        .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; pos.len()]);
    let mut idx_u32: Vec<u32> = match reader.read_indices() {
        Some(gltf_rs::mesh::util::ReadIndices::U32(it)) => it.collect(),
        Some(gltf_rs::mesh::util::ReadIndices::U16(it)) => it.map(|v| v as u32).collect(),
        Some(gltf_rs::mesh::util::ReadIndices::U8(it)) => it.map(|v| v as u32).collect(),
        None => (0..pos.len() as u32).collect(),
    };
    // Clamp to u16 capacity
    let max_index = *idx_u32.iter().max().unwrap_or(&0);
    if max_index > u16::MAX as u32 {
        // Best effort: remap by slicing to first 65535 vertices
        let cap = (u16::MAX as usize).min(pos.len());
        idx_u32.retain(|&v| (v as usize) < cap);
    }
    if idx_u32.is_empty() || pos.is_empty() {
        return None;
    }
    let verts: Vec<Vtx> = pos
        .iter()
        .zip(nrm.iter())
        .map(|(p, n)| Vtx { pos: *p, nrm: *n })
        .collect();
    let ib_u16: Vec<u16> = idx_u32.into_iter().map(|v| v as u16).collect();
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("wyvern-first-prim-vb"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("wyvern-first-prim-ib"),
        contents: bytemuck::cast_slice(&ib_u16),
        usage: wgpu::BufferUsages::INDEX,
    });
    log::info!(
        target: "wyvern",
        "wyvern: first-primitive fallback ok: {} (verts={}, idx={})",
        prepared.display(),
        verts.len(),
        ib_u16.len()
    );
    Some((vb, ib, ib_u16.len() as u32))
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
