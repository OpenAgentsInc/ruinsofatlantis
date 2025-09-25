//! Asset system (CPU-side) for loading meshes.
//!
//! Initial focus is on GLTF mesh loading. This module parses a `.gltf` file and
//! produces CPU-side mesh data (positions + normals + indices) that the renderer
//! can upload to GPU buffers. Materials/textures are intentionally ignored for
//! now to keep the prototype simple.
//!
//! Design notes
//! - We flatten all mesh primitives in the file into a single mesh by appending
//!   vertices and re-indexing; this keeps render wiring straightforward.
//! - Indices are converted to `u16`. If any index exceeds `u16::MAX`, loading
//!   fails with a clear error (the demo assets are expected to be small).
//! - If normals are missing in the source, we fall back to a constant up normal
//!   so the model renders without lighting artifacts. Proper normal generation
//!   can be added later.

use anyhow::{anyhow, bail, Context, Result};
use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use crate::gfx::Vertex;
use gltf::mesh::Semantic;
use gltf::buffer::Data;
use draco_decoder::{MeshDecodeConfig, AttributeDataType, decode_mesh};
use serde_json::Value;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;

/// CPU-side mesh ready to be uploaded to GPU.
pub struct CpuMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

// CPU-side skinned mesh data and animation clips (simplified)
use glam::{Mat4, Vec3, Quat};
use std::collections::HashMap;

#[derive(Clone)]
pub struct VertexSkinCPU {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
    pub joints: [u16; 4],
    pub weights: [f32; 4],
    pub uv: [f32; 2],
}

#[derive(Clone)]
pub struct TrackVec3 { pub times: Vec<f32>, pub values: Vec<Vec3> }
#[derive(Clone)]
pub struct TrackQuat { pub times: Vec<f32>, pub values: Vec<Quat> }

#[derive(Clone)]
pub struct AnimClip {
    pub name: String,
    pub duration: f32,
    pub t_tracks: HashMap<usize, TrackVec3>,
    pub r_tracks: HashMap<usize, TrackQuat>,
    pub s_tracks: HashMap<usize, TrackVec3>,
}

pub struct SkinnedMeshCPU {
    pub vertices: Vec<VertexSkinCPU>,
    pub indices: Vec<u16>,
    pub joints_nodes: Vec<usize>,
    pub inverse_bind: Vec<Mat4>,
    pub parent: Vec<Option<usize>>, // node parent map
    pub base_t: Vec<Vec3>,
    pub base_r: Vec<Quat>,
    pub base_s: Vec<Vec3>,
    pub animations: HashMap<String, AnimClip>,
    pub base_color_texture: Option<TextureCPU>,
}

pub struct TextureCPU { pub pixels: Vec<u8>, pub width: u32, pub height: u32, pub srgb: bool }

pub fn load_gltf_skinned(path: &Path) -> Result<SkinnedMeshCPU> {
    // Robust prepare: prefer original glTF if it imports; fall back to a
    // decompressed copy (or auto-decompress) only if needed.
    let prepared = prepare_gltf_path(path)?;
    if prepared != path { log::warn!("anim diag: using prepared glTF: {}", prepared.display()); }
    let (doc, buffers, images) = gltf::import(&prepared).with_context(|| format!("import skinned glTF: {}", prepared.display()))?;

    // Build parent map and base TRS
    let node_count = doc.nodes().len();
    let mut parent = vec![None; node_count];
    for n in doc.nodes() {
        for c in n.children() { parent[c.index()] = Some(n.index()); }
    }
    let mut base_t = vec![Vec3::ZERO; node_count];
    let mut base_r = vec![Quat::IDENTITY; node_count];
    let mut base_s = vec![Vec3::ONE; node_count];
    for n in doc.nodes() {
        let (t, r, s) = decompose_node(&n);
        base_t[n.index()] = t; base_r[n.index()] = r; base_s[n.index()] = s;
    }

    // Find first mesh primitive with joints/weights and its skin via the node
    let mut skin_opt: Option<gltf::Skin> = None;
    let mut verts: Vec<VertexSkinCPU> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();
    // (deferred material loading uses `images` later)

    // Prefer a mesh attached to a skinned node; fall back later if none.
    let mut joints_all_zero = false;
    'outer: for node in doc.nodes() {
        if node.skin().is_none() { continue; }
        if let Some(mesh) = node.mesh() {
            if let Some(skin) = node.skin() { skin_opt = Some(skin); }
            for prim in mesh.primitives() {
                let reader = prim.reader(|b| buffers.get(b.index()).map(|bb| bb.0.as_slice()));
                let pos_it = reader.read_positions();
                let nrm_it = reader.read_normals();
                let joints_it = reader.read_joints(0);
                let weights_it = reader.read_weights(0);

                // If standard attributes are present, use them. Only fall back to Draco when
                // joints/weights are unavailable via the normal reader.
                if pos_it.is_some() && nrm_it.is_some() && joints_it.is_some() && weights_it.is_some() {
                    let pos: Vec<[f32;3]> = pos_it.unwrap().collect();
                    let nrm: Vec<[f32;3]> = nrm_it.unwrap().collect();
                    // Pick the UV set actually referenced by baseColorTexture (default 0)
                    let uv_set = prim.material()
                        .pbr_metallic_roughness()
                        .base_color_texture()
                        .map(|ti| ti.tex_coord())
                        .unwrap_or(0);
                    let uv_opt = reader.read_tex_coords(uv_set).map(|tc| tc.into_f32());
                    let joints: Vec<[u16;4]> = match joints_it.unwrap() {
                        gltf::mesh::util::ReadJoints::U16(it) => it.map(|v| [v[0],v[1],v[2],v[3]]).collect(),
                        gltf::mesh::util::ReadJoints::U8(it) => it.map(|v| [v[0] as u16,v[1] as u16,v[2] as u16,v[3] as u16]).collect(),
                    };
                    let weights: Vec<[f32;4]> = match weights_it.unwrap() {
                        gltf::mesh::util::ReadWeights::F32(it) => it.collect(),
                        gltf::mesh::util::ReadWeights::U16(it) => it.map(|v| [v[0] as f32/65535.0, v[1] as f32/65535.0, v[2] as f32/65535.0, v[3] as f32/65535.0]).collect(),
                        gltf::mesh::util::ReadWeights::U8(it) => it.map(|v| [v[0] as f32/255.0, v[1] as f32/255.0, v[2] as f32/255.0, v[3] as f32/255.0]).collect(),
                    };

                    let uv: Vec<[f32;2]> = if let Some(it) = uv_opt {
                        let collected: Vec<[f32;2]> = it.collect();
                        let all_zero = collected.iter().all(|u| u[0] == 0.0 && u[1] == 0.0);
                        if collected.len() == pos.len() && !all_zero { collected } else {
                            log::warn!("wizard: invalid TEXCOORD_{} (len {}, all_zero={}); using planar fallback", uv_set, collected.len(), all_zero);
                            pos.iter().map(|p| [0.5 + 0.5 * p[0], 0.5 - 0.5 * p[2]]).collect()
                        }
                    } else {
                        log::warn!("wizard: TEXCOORD_{} missing; using planar fallback UVs", uv_set);
                        pos.iter().map(|p| [0.5 + 0.5 * p[0], 0.5 - 0.5 * p[2]]).collect()
                    };

                    for i in 0..pos.len() {
                        verts.push(VertexSkinCPU { pos: pos[i], nrm: nrm[i], joints: joints[i], weights: weights[i], uv: uv[i] });
                    }
                    // Debug: log JOINTS/WEIGHTS ranges captured from standard attributes
                    if !verts.is_empty() {
                        let mut jmin = [u16::MAX;4];
                        let mut jmax = [0u16;4];
                        let mut wsum_min = f32::INFINITY; let mut wsum_max = f32::NEG_INFINITY;
                        for v in verts.iter().take(512) {
                            for k in 0..4 { jmin[k] = jmin[k].min(v.joints[k]); jmax[k] = jmax[k].max(v.joints[k]); }
                            let s = v.weights[0]+v.weights[1]+v.weights[2]+v.weights[3];
                            wsum_min = wsum_min.min(s); wsum_max = wsum_max.max(s);
                        }
                        log::warn!("assets: std attrs JOINTS_0=[{}..{}] WEIGHT_SUM=[{:.3}..{:.3}]", jmin[0], jmax[0], wsum_min, wsum_max);
                    }
                    let idx_u32: Vec<u32> = match reader.read_indices() {
                        Some(gltf::mesh::util::ReadIndices::U16(it)) => it.map(|v| v as u32).collect(),
                        Some(gltf::mesh::util::ReadIndices::U32(it)) => it.collect(),
                        Some(gltf::mesh::util::ReadIndices::U8(it)) => it.map(|v| v as u32).collect(),
                        None => (0..pos.len() as u32).collect(),
                    };
                    for i in idx_u32 { if i > u16::MAX as u32 { bail!("wizard indices exceed u16"); } indices.push(i as u16); }
                    break 'outer;
                } else if prim.extension_value("KHR_draco_mesh_compression").is_some() {
                    // Fallback: decode via Draco if standard attributes are unavailable
                    log::warn!("assets: falling back to Draco decode for skinned primitive");
                    decode_draco_skinned_primitive(&doc, &buffers, &prim, &mut verts, &mut indices)?;
                    break 'outer;
                } else {
                    continue;
                }
            }
        }
    }

    // If we did not capture skinned attributes, fall back to rigid geometry so we can still render the mesh.
    if verts.is_empty() {
        // Try first mesh primitive with positions/normals and synthesize joints/weights
        'find_any: for mesh in doc.meshes() {
            for prim in mesh.primitives() {
                let reader = prim.reader(|b| buffers.get(b.index()).map(|bb| bb.0.as_slice()));
                let Some(pos_it) = reader.read_positions() else { continue };
                let nrm_it = reader.read_normals();
                let pos: Vec<[f32;3]> = pos_it.collect();
                let nrm: Vec<[f32;3]> = nrm_it.map(|it| it.collect()).unwrap_or_else(|| vec![[0.0,1.0,0.0]; pos.len()]);
                let uv: Vec<[f32;2]> = pos.iter().map(|p| [0.5 + 0.5 * p[0], 0.5 - 0.5 * p[2]]).collect();
                for i in 0..pos.len() {
                    verts.push(VertexSkinCPU { pos: pos[i], nrm: nrm[i], joints: [0,0,0,0], weights: [1.0, 0.0, 0.0, 0.0], uv: uv[i] });
                }
                let idx_u32: Vec<u32> = match reader.read_indices() { Some(gltf::mesh::util::ReadIndices::U16(it)) => it.map(|v| v as u32).collect(), Some(gltf::mesh::util::ReadIndices::U32(it)) => it.collect(), Some(gltf::mesh::util::ReadIndices::U8(it)) => it.map(|v| v as u32).collect(), None => (0..pos.len() as u32).collect() };
                for i in idx_u32 { if i > u16::MAX as u32 { bail!("indices exceed u16"); } indices.push(i as u16); }
                break 'find_any;
            }
        }
        joints_all_zero = true;
    } else {
        // Check if all joints are zero -> indicates missing skin attributes
        joints_all_zero = verts.iter().all(|v| v.joints == [0,0,0,0]);
    }

    // Final attempt: if joints are all zero and the file uses Draco, try to decompress via gltf-transform CLI and re-import
    if joints_all_zero && doc.extensions_required().any(|e| e == "KHR_draco_mesh_compression") {
        let decompressed = path.with_extension("decompressed.gltf");
        if let Some(out_path) = try_gltf_transform_decompress(path, &decompressed) {
            log::warn!("anim diag: Draco-compressed skin; re-importing decompressed glTF: {}", out_path.display());
            let (doc2, buffers2, _images2) = gltf::import(&out_path).with_context(|| format!("import decompressed glTF: {}", out_path.display()))?;
            verts.clear(); indices.clear();
            'outer2: for scene in doc2.scenes() { for node in scene.nodes() {
                if node.skin().is_none() { continue; }
                if let Some(mesh) = node.mesh() {
                    for prim in mesh.primitives() {
                        let reader = prim.reader(|b| buffers2.get(b.index()).map(|bb| bb.0.as_slice()));
                        let Some(pos_it) = reader.read_positions() else { continue };
                        let nrm_it = reader.read_normals();
                        let uv_set = prim.material().pbr_metallic_roughness().base_color_texture().map(|ti| ti.tex_coord()).unwrap_or(0);
                        let uv_opt = reader.read_tex_coords(uv_set).map(|tc| tc.into_f32());
                        let joints = match reader.read_joints(0) { Some(gltf::mesh::util::ReadJoints::U16(it)) => it.map(|v| [v[0],v[1],v[2],v[3]]).collect::<Vec<[u16;4]>>(), Some(gltf::mesh::util::ReadJoints::U8(it)) => it.map(|v| [v[0] as u16,v[1] as u16,v[2] as u16,v[3] as u16]).collect(), _ => continue };
                        let weights = match reader.read_weights(0) { Some(gltf::mesh::util::ReadWeights::F32(it)) => it.collect::<Vec<[f32;4]>>(), Some(gltf::mesh::util::ReadWeights::U16(it)) => it.map(|v| [v[0] as f32/65535.0, v[1] as f32/65535.0, v[2] as f32/65535.0, v[3] as f32/65535.0]).collect(), Some(gltf::mesh::util::ReadWeights::U8(it)) => it.map(|v| [v[0] as f32/255.0, v[1] as f32/255.0, v[2] as f32/255.0, v[3] as f32/255.0]).collect(), None => continue };
                        let pos: Vec<[f32;3]> = pos_it.collect();
                        let nrm: Vec<[f32;3]> = nrm_it.map(|it| it.collect()).unwrap_or_else(|| vec![[0.0,1.0,0.0]; pos.len()]);
                        let uv: Vec<[f32;2]> = if let Some(it) = uv_opt { it.collect() } else { pos.iter().map(|p| [0.5 + 0.5 * p[0], 0.5 - 0.5 * p[2]]).collect() };
                        for i in 0..pos.len() { verts.push(VertexSkinCPU { pos: pos[i], nrm: nrm[i], joints: joints[i], weights: weights[i], uv: uv[i] }); }
                        let idx_u32: Vec<u32> = match reader.read_indices() { Some(gltf::mesh::util::ReadIndices::U16(it)) => it.map(|v| v as u32).collect(), Some(gltf::mesh::util::ReadIndices::U32(it)) => it.collect(), Some(gltf::mesh::util::ReadIndices::U8(it)) => it.map(|v| v as u32).collect(), None => (0..pos.len() as u32).collect() };
                        for i in idx_u32 { if i > u16::MAX as u32 { bail!("indices exceed u16"); } indices.push(i as u16); }
                        break 'outer2;
                    }
                }
            }}
            let all_zero = verts.iter().all(|v| v.joints == [0,0,0,0]);
            log::warn!("anim diag: after decompress, all joints zero = {} (verts={})", all_zero, verts.len());
        } else {
            log::warn!("anim diag: Draco decompression tool not available; skinning may be static");
        }
    }

    // Choose a skin if available; otherwise synthesize a 1-joint skin.
    let synth_skin = verts.is_empty() || doc.skins().next().is_none();
    let skin = if synth_skin { None } else { Some(skin_opt.unwrap_or_else(|| doc.skins().next().unwrap())) };
    let (joints_nodes, inverse_bind) = if let Some(skin) = skin {
        let joints_nodes: Vec<usize> = skin.joints().map(|j| j.index()).collect();
        let rdr = skin.reader(|b| buffers.get(b.index()).map(|bb| bb.0.as_slice()));
        let inverse_bind = match rdr.read_inverse_bind_matrices() {
            Some(iter) => iter.map(|m| Mat4::from_cols_array_2d(&m)).collect(),
            None => vec![Mat4::IDENTITY; joints_nodes.len()],
        };
        (joints_nodes, inverse_bind)
    } else {
        (vec![0usize], vec![Mat4::IDENTITY])
    };

    // Build animation clips (only named ones we care about)
    let mut animations: HashMap<String, AnimClip> = HashMap::new();
    let wanted = ["PortalOpen", "Still", "Waiting"];
    for anim in doc.animations() {
        let name = anim.name().unwrap_or("").to_string();
        if !wanted.contains(&name.as_str()) { continue; }
        let mut t_tracks: HashMap<usize, TrackVec3> = HashMap::new();
        let mut r_tracks: HashMap<usize, TrackQuat> = HashMap::new();
        let mut s_tracks: HashMap<usize, TrackVec3> = HashMap::new();
        let mut max_t = 0.0f32;
        for ch in anim.channels() {
            let target = ch.target();
            let node_idx = target.node().index();
            let rdr = ch.reader(|b| buffers.get(b.index()).map(|bb| bb.0.as_slice()));
            let Some(inputs) = rdr.read_inputs() else { continue };
            let times: Vec<f32> = inputs.collect();
            if let Some(&last) = times.last() { if last > max_t { max_t = last; } }
            match target.property() {
                gltf::animation::Property::Translation => {
                    let Some(outs) = rdr.read_outputs() else { continue };
                    let vals: Vec<Vec3> = match outs { gltf::animation::util::ReadOutputs::Translations(it) => it.map(|v| Vec3::from(v)).collect(), _ => continue };
                    t_tracks.insert(node_idx, TrackVec3 { times: times.clone(), values: vals });
                }
                gltf::animation::Property::Rotation => {
                    let Some(outs) = rdr.read_outputs() else { continue };
                    let vals: Vec<Quat> = match outs { gltf::animation::util::ReadOutputs::Rotations(it) => it.into_f32().map(|v| Quat::from_xyzw(v[0],v[1],v[2],v[3]).normalize()).collect(), _ => continue };
                    r_tracks.insert(node_idx, TrackQuat { times: times.clone(), values: vals });
                }
                gltf::animation::Property::Scale => {
                    let Some(outs) = rdr.read_outputs() else { continue };
                    let vals: Vec<Vec3> = match outs { gltf::animation::util::ReadOutputs::Scales(it) => it.map(|v| Vec3::from(v)).collect(), _ => continue };
                    s_tracks.insert(node_idx, TrackVec3 { times: times.clone(), values: vals });
                }
                _ => {}
            }
        }
        animations.insert(name.clone(), AnimClip { name, duration: max_t, t_tracks, r_tracks, s_tracks });
    }

    // Debug: list available clips
    if !animations.is_empty() {
        for (k,v) in &animations { log::info!("anim clip: '{}' dur={:.3}s T={} R={} S={}", k, v.duration, v.t_tracks.len(), v.r_tracks.len(), v.s_tracks.len()); }
    } else {
        log::warn!("no animations parsed from {}", path.display());
    }

    if animations.is_empty() {
        animations.insert("__static".to_string(), AnimClip { name: "__static".to_string(), duration: 0.0, t_tracks: HashMap::new(), r_tracks: HashMap::new(), s_tracks: HashMap::new() });
    }

    // Final guard: ensure we have non-empty geometry
    if verts.is_empty() || indices.is_empty() {
        bail!("no renderable geometry found in {}", path.display());
    }

    // Try to grab baseColor texture via images from import
    let mut base_color_texture = None;
    if let Some(material) = doc.meshes().next().and_then(|m| m.primitives().next()).map(|p| p.material()) {
        if let Some(texinfo) = material.pbr_metallic_roughness().base_color_texture() {
            let tex = texinfo.texture();
            let img_idx = tex.source().index();
            if let Some(img) = images.get(img_idx) {
                // Convert to RGBA8
                let (w,h) = (img.width, img.height);
                let pixels = match img.format {
                    gltf::image::Format::R8G8B8A8 => img.pixels.clone(),
                    gltf::image::Format::R8G8B8 => {
                        let mut out = Vec::with_capacity((w*h*4) as usize);
                        for c in img.pixels.chunks_exact(3) { out.extend_from_slice(&[c[0],c[1],c[2],255]); }
                        out
                    }
                    gltf::image::Format::R8 => {
                        let mut out = Vec::with_capacity((w*h*4) as usize);
                        for &r in &img.pixels { out.extend_from_slice(&[r,r,r,255]); }
                        out
                    }
                    _ => img.pixels.clone(),
                };
                base_color_texture = Some(TextureCPU { pixels, width: w, height: h, srgb: true });
            }
        }
    }

    // Log UV range for diagnostics
    if !verts.is_empty() {
        let mut umin = f32::INFINITY; let mut vmin = f32::INFINITY; let mut umax = f32::NEG_INFINITY; let mut vmax = f32::NEG_INFINITY;
        for v in &verts { umin = umin.min(v.uv[0]); umax = umax.max(v.uv[0]); vmin = vmin.min(v.uv[1]); vmax = vmax.max(v.uv[1]); }
        log::info!("loader: wizard UV range: u=[{:.3},{:.3}] v=[{:.3},{:.3}]", umin, umax, vmin, vmax);
    }

    Ok(SkinnedMeshCPU { vertices: verts, indices, joints_nodes, inverse_bind, parent, base_t, base_r, base_s, animations, base_color_texture })
}

fn decompose_node(n: &gltf::Node) -> (Vec3, Quat, Vec3) {
    use gltf::scene::Transform;
    match n.transform() {
        Transform::Matrix { matrix } => {
            let m = Mat4::from_cols_array_2d(&matrix);
            let (s, r, t) = m.to_scale_rotation_translation();
            (t, r, s)
        }
        Transform::Decomposed { translation, rotation, scale } => {
            (Vec3::from(translation), Quat::from_array(rotation).normalize(), Vec3::from(scale))
        }
    }
}


/// Load a `.gltf` file from disk and merge all primitives into a single mesh.
pub fn load_gltf_mesh(path: &Path) -> Result<CpuMesh> {
    let source_path: PathBuf = path.to_path_buf();
    let import_res = gltf::import(&source_path);
    let (doc, buffers, _images) = match import_res {
        Ok(ok) => ok,
        Err(e) => {
            // Try the JSON/Draco fallback path (handles extensionsRequired + no bufferViews)
            if let Ok(mesh) = try_load_gltf_draco_json(path) {
                return Ok(mesh);
            }
            return Err(anyhow!("failed to import glTF: {}", source_path.display()).context(e));
        }
    };

    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();

    for mesh in doc.meshes() {
        for prim in mesh.primitives() {
            // If this primitive uses Draco, decode it in Rust.
            let uses_draco = prim
                .extension_value("KHR_draco_mesh_compression")
                .is_some();
            if uses_draco {
                decode_draco_primitive(&doc, &buffers, &prim, &mut vertices, &mut indices)?;
                continue;
            }

            // Non-Draco path using standard accessors
            let reader = prim.reader(|buf| buffers.get(buf.index()).map(|b| b.0.as_slice()));
            let Some(pos_iter) = reader.read_positions() else { continue };
            let nrm_iter_opt = reader.read_normals();
            let start = vertices.len() as u32;
            for (i, p) in pos_iter.enumerate() {
                let n = nrm_iter_opt
                    .as_ref()
                    .and_then(|it| it.clone().nth(i))
                    .unwrap_or([0.0, 1.0, 0.0]);
                vertices.push(Vertex { pos: p, nrm: n });
            }
            let idx_iter = match reader.read_indices() {
                Some(gltf::mesh::util::ReadIndices::U16(it)) => it.map(|i| i as u32).collect::<Vec<u32>>(),
                Some(gltf::mesh::util::ReadIndices::U32(it)) => it.collect::<Vec<u32>>(),
                Some(gltf::mesh::util::ReadIndices::U8(it)) => it.map(|i| i as u32).collect::<Vec<u32>>(),
                None => {
                    let added = (vertices.len() as u32 - start) as usize;
                    if added % 3 != 0 {
                        bail!("primitive without indices has non-multiple-of-3 vertex count");
                    }
                    (0..added as u32).map(|i| i + start).collect::<Vec<u32>>()
                }
            };
            for i in idx_iter {
                let idx = i + start;
                if idx > u16::MAX as u32 { bail!("index {} exceeds u16::MAX", idx); }
                indices.push(idx as u16);
            }
        }
    }

    if vertices.is_empty() || indices.is_empty() { bail!("no renderable geometry found in {}", path.display()); }
    Ok(CpuMesh { vertices, indices })
}

fn try_load_gltf_draco_json(path: &Path) -> Result<CpuMesh> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read glTF json: {}", path.display()))?;
    let v: Value = serde_json::from_str(&text).context("parse glTF JSON")?;
    // Check required extension
    let empty = Vec::new();
    let ext_req = v.get("extensionsRequired").and_then(|x| x.as_array()).unwrap_or(&empty);
    let has_draco = ext_req.iter().any(|s| s.as_str() == Some("KHR_draco_mesh_compression"));
    if !has_draco {
        bail!("JSON fallback: no KHR_draco_mesh_compression present");
    }

    // Decode buffers (support only data: URIs here)
    let buffers = v.get("buffers").and_then(|b| b.as_array()).context("buffers missing")?;
    let mut bin_bytes: Vec<Vec<u8>> = Vec::new();
    for b in buffers {
        let uri = b.get("uri").and_then(|u| u.as_str()).context("buffer.uri missing")?;
        if let Some(idx) = uri.find(",") {
            let b64 = &uri[(idx+1)..];
            let data = BASE64.decode(b64.as_bytes()).context("base64 decode buffer")?;
            bin_bytes.push(data);
        } else {
            bail!("only data: URIs are supported in JSON fallback");
        }
    }

    // Convenience accessors
    let views = v.get("bufferViews").and_then(|x| x.as_array()).context("bufferViews missing")?;
    let accessors = v.get("accessors").and_then(|x| x.as_array()).context("accessors missing")?;
    let meshes = v.get("meshes").and_then(|x| x.as_array()).context("meshes missing")?;

    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();

    for mesh in meshes {
        let empty_p = Vec::new();
        let prims = mesh.get("primitives").and_then(|p| p.as_array()).unwrap_or(&empty_p);
        for prim in prims {
            let ext = prim.get("extensions").and_then(|e| e.get("KHR_draco_mesh_compression"));
            if ext.is_none() { continue; }
            let ext = ext.unwrap();
            let bv_index = ext.get("bufferView").and_then(|b| b.as_u64()).context("draco bufferView missing")? as usize;
            let attr_map = ext.get("attributes").and_then(|a| a.as_object()).context("draco attributes missing")?;

            // Byte range for compressed data
            let bv = &views[bv_index];
            let buf_index = bv.get("buffer").and_then(|b| b.as_u64()).unwrap_or(0) as usize;
            let byte_offset = bv.get("byteOffset").and_then(|b| b.as_u64()).unwrap_or(0) as usize;
            let byte_length = bv.get("byteLength").and_then(|b| b.as_u64()).context("byteLength missing")? as usize;
            let data = &bin_bytes[buf_index][byte_offset..byte_offset+byte_length];

            // Vertex/index counts & attribute dims/types from accessors referenced in primitive.attributes
            let attrs = prim.get("attributes").and_then(|a| a.as_object()).context("primitive.attributes missing")?;
            let pos_acc_idx = attrs.get("POSITION").and_then(|i| i.as_u64()).context("POSITION accessor missing")? as usize;
            let pos_acc = &accessors[pos_acc_idx];
            let vertex_count = pos_acc.get("count").and_then(|c| c.as_u64()).context("POSITION.count missing")? as u32;

            let index_count = prim.get("indices").and_then(|i| i.as_u64()).map(|idx| accessors[idx as usize].get("count").and_then(|c| c.as_u64()).unwrap_or(0) as u32).unwrap_or(0);

            let mut cfg = MeshDecodeConfig::new(vertex_count, index_count);
            // Build mapping sorted by attribute id
            let mut mapped: Vec<(u32, (&str, usize))> = Vec::new();
            for (k, idv) in attr_map.iter() {
                if let Some(id) = idv.as_u64() {
                    let acc_idx = attrs.get(k).and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                    mapped.push((id as u32, (k.as_str(), acc_idx)));
                }
            }
            mapped.sort_by_key(|(id, _)| *id);

            for (_, (sem_name, acc_idx)) in &mapped {
                let acc = &accessors[*acc_idx];
                let dims = acc.get("type").and_then(|t| t.as_str()).unwrap_or("VEC3");
                let dim = match dims { "SCALAR"=>1, "VEC2"=>2, "VEC3"=>3, "VEC4"=>4, _=>3 };
                let ctype = acc.get("componentType").and_then(|c| c.as_u64()).unwrap_or(5126);
                let ty = match ctype { 5126=>AttributeDataType::Float32, 5123=>AttributeDataType::UInt16, 5121=>AttributeDataType::UInt8, 5122=>AttributeDataType::Int16, 5120=>AttributeDataType::Int8, 5125=>AttributeDataType::UInt32, _=>AttributeDataType::Float32 };
                // Only configure for semantics we will parse (POSITION/NORMAL); extra attrs ok
                cfg.add_attribute(dim as u32, ty);
                let _ = sem_name; // keep for future if needed
            }

            let decoded = pollster::block_on(decode_mesh(data, &cfg)).context("draco native decode failed")?;

            let mut off = 0usize;
            let idx_bytes = if index_count <= u16::MAX as u32 { (index_count as usize) * 2 } else { (index_count as usize) * 4 };
            if idx_bytes > 0 {
                let idx_slice = &decoded[off..off+idx_bytes];
                off += idx_bytes;
                if index_count <= u16::MAX as u32 {
                    for c in idx_slice.chunks_exact(2) { indices.push(u16::from_le_bytes([c[0],c[1]])); }
                } else {
                    for c in idx_slice.chunks_exact(4) {
                        let v = u32::from_le_bytes([c[0],c[1],c[2],c[3]]);
                        indices.push(u16::try_from(v).map_err(|_| anyhow!("decoded index {} exceeds u16", v))?);
                    }
                }
            }

            // Now parse attributes in mapped order; grab POSITION/NORMAL only
            let mut pos_opt: Option<Vec<[f32;3]>> = None;
            let mut nrm_opt: Option<Vec<[f32;3]>> = None;
            for (_, (sem_name, acc_idx)) in &mapped {
                let acc = &accessors[*acc_idx];
                let dims = acc.get("type").and_then(|t| t.as_str()).unwrap_or("VEC3");
                let dim = match dims { "SCALAR"=>1usize, "VEC2"=>2usize, "VEC3"=>3usize, "VEC4"=>4usize, _=>3usize };
                let ctype = acc.get("componentType").and_then(|c| c.as_u64()).unwrap_or(5126);
                let comp_size = match ctype { 5126|5125|5124=>4usize, 5123|5122=>2usize, 5121|5120=>1usize, _=>4usize };
                let byte_len = dim * (vertex_count as usize) * comp_size;
                let slice = &decoded[off..off+byte_len];
                off += byte_len;

                match (*sem_name, ctype) {
                    ("POSITION", 5126) => {
                        let mut v = Vec::with_capacity(vertex_count as usize);
                        for c in slice.chunks_exact(4*dim) {
                            let x = f32::from_le_bytes([c[0],c[1],c[2],c[3]]);
                            let y = f32::from_le_bytes([c[4],c[5],c[6],c[7]]);
                            let z = if dim>2 { f32::from_le_bytes([c[8],c[9],c[10],c[11]]) } else { 0.0 };
                            v.push([x,y,z]);
                        }
                        pos_opt = Some(v);
                    }
                    ("NORMAL", 5126) => {
                        let mut v = Vec::with_capacity(vertex_count as usize);
                        for c in slice.chunks_exact(4*dim) {
                            let x = f32::from_le_bytes([c[0],c[1],c[2],c[3]]);
                            let y = f32::from_le_bytes([c[4],c[5],c[6],c[7]]);
                            let z = if dim>2 { f32::from_le_bytes([c[8],c[9],c[10],c[11]]) } else { 1.0 };
                            v.push([x,y,z]);
                        }
                        nrm_opt = Some(v);
                    }
                    _ => {}
                }
            }

            let start = vertices.len();
            let pos = pos_opt.context("decoded POSITION missing")?;
            let nrm = nrm_opt.unwrap_or_else(|| vec![[0.0,1.0,0.0]; pos.len()]);
            for i in 0..pos.len() { vertices.push(Vertex { pos: pos[i], nrm: nrm[i] }); }
            // Rebase indices for this primitive
            let start_u = start as u32;
            if index_count == 0 {
                for i in 0..(pos.len() as u32) { indices.push((start_u + i) as u16); }
            } else {
                let base = indices.len() - (index_count as usize);
                for i in base..indices.len() {
                    let v = indices[i] as u32 + start_u;
                    indices[i] = u16::try_from(v).map_err(|_| anyhow!("rebased index {} exceeds u16", v))?;
                }
            }
        }
    }

    if vertices.is_empty() || indices.is_empty() { bail!("JSON fallback: no geometry decoded in {}", path.display()); }
    Ok(CpuMesh { vertices, indices })
}
fn decode_draco_primitive(
    doc: &gltf::Document,
    buffers: &Vec<Data>,
    prim: &gltf::mesh::Primitive,
    out_vertices: &mut Vec<Vertex>,
    out_indices: &mut Vec<u16>,
) -> Result<()> {
    // Parse extension JSON for bufferView and attribute ID map
    let Some(ext_val) = prim.extension_value("KHR_draco_mesh_compression") else { return Ok(()); };
    let obj = ext_val.as_object().context("draco ext not an object")?;
    let bv_index = obj.get("bufferView").and_then(|v| v.as_u64()).context("draco bufferView missing")? as usize;
    let attr_map = obj.get("attributes").and_then(|v| v.as_object()).context("draco attributes missing")?;

    // Resolve compressed bytes from bufferView
    let bv = doc.views().nth(bv_index).context("bufferView index out of range")?;
    let buf = bv.buffer();
    let data = &buffers[buf.index()].0;
    let start = bv.offset();
    let end = start + bv.length();
    let comp_bytes = &data[start..end];

    // Gather counts and attribute dims/types from top-level accessors for this primitive
    let pos_accessor = prim.get(&Semantic::Positions).context("POSITION accessor missing")?;
    let vertex_count = pos_accessor.count() as u32;
    let index_count = prim.indices().map(|a| a.count() as u32).unwrap_or(0);

    // Build decode config so internal buffer size matches decoder output
    let mut cfg = MeshDecodeConfig::new(vertex_count, index_count);
    // Build a list of semantics with their Draco attribute IDs, sorted by ID
    let mut mapped: Vec<(u32, Semantic)> = Vec::new();
    for (k, v) in attr_map.iter() {
        let id = v.as_u64().unwrap_or(0) as u32;
        let sem = match k.as_str() {
            "POSITION" => Semantic::Positions,
            "NORMAL" => Semantic::Normals,
            s if s.starts_with("TEXCOORD_") => {
                let set: u32 = s[9..].parse().unwrap_or(0); Semantic::TexCoords(set)
            }
            _ => continue,
        };
        mapped.push((id, sem));
    }
    mapped.sort_by_key(|(id, _)| *id);

    for (_, sem) in &mapped {
        let acc = prim.get(sem).context("accessor for mapped semantic missing")?;
        let dim = match acc.dimensions() { gltf::accessor::Dimensions::Vec2 => 2, gltf::accessor::Dimensions::Vec3 => 3, gltf::accessor::Dimensions::Vec4 => 4, _ => 3 };
        let ty = match acc.data_type() { gltf::accessor::DataType::F32 => AttributeDataType::Float32, gltf::accessor::DataType::U16 => AttributeDataType::UInt16, gltf::accessor::DataType::U8 => AttributeDataType::UInt8, gltf::accessor::DataType::I16 => AttributeDataType::Int16, gltf::accessor::DataType::I8 => AttributeDataType::Int8, gltf::accessor::DataType::U32 => AttributeDataType::UInt32 };
        cfg.add_attribute(dim as u32, ty);
    }

    // Decode via native path (blocking)
    let decoded = pollster::block_on(decode_mesh(comp_bytes, &cfg)).context("draco native decode failed")?;

    // Parse decoded stream: [indices][attr0][attr1]...
    let mut off = 0usize;
    let idx_bytes = if index_count <= u16::MAX as u32 { (index_count as usize) * 2 } else { (index_count as usize) * 4 };
    if idx_bytes > 0 {
        let idx_slice = &decoded[off..off+idx_bytes];
        off += idx_bytes;
        if index_count <= u16::MAX as u32 {
            for chunk in idx_slice.chunks_exact(2) { out_indices.push(u16::from_le_bytes([chunk[0], chunk[1]])); }
        } else {
            for chunk in idx_slice.chunks_exact(4) {
                let v = u32::from_le_bytes([chunk[0],chunk[1],chunk[2],chunk[3]]);
                let vv = u16::try_from(v).map_err(|_| anyhow!("decoded index {} exceeds u16", v))?;
                out_indices.push(vv);
            }
        }
    }

    // Prepare per-attribute slices
    let mut pos_opt: Option<Vec<[f32;3]>> = None;
    let mut nrm_opt: Option<Vec<[f32;3]>> = None;
    for (_, sem) in &mapped {
        let acc = prim.get(sem).unwrap();
        let dim = match acc.dimensions() { gltf::accessor::Dimensions::Vec2 => 2usize, gltf::accessor::Dimensions::Vec3 => 3usize, gltf::accessor::Dimensions::Vec4 => 4usize, _ => 3usize };
        let ty = match acc.data_type() { gltf::accessor::DataType::F32 => AttributeDataType::Float32, gltf::accessor::DataType::U16 => AttributeDataType::UInt16, gltf::accessor::DataType::U8 => AttributeDataType::UInt8, gltf::accessor::DataType::I16 => AttributeDataType::Int16, gltf::accessor::DataType::I8 => AttributeDataType::Int8, gltf::accessor::DataType::U32 => AttributeDataType::UInt32 };
        let comp_size = ty.size_in_bytes();
        let bytes_len = dim * (vertex_count as usize) * comp_size;
        let slice = &decoded[off..off+bytes_len];
        off += bytes_len;

        match (sem, ty) {
            (Semantic::Positions, AttributeDataType::Float32) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(4*dim) {
                    let x = f32::from_le_bytes([c[0],c[1],c[2],c[3]]);
                    let y = f32::from_le_bytes([c[4],c[5],c[6],c[7]]);
                    let z = if dim>2 { f32::from_le_bytes([c[8],c[9],c[10],c[11]]) } else { 0.0 };
                    v.push([x,y,z]);
                }
                pos_opt = Some(v);
            }
            (Semantic::Normals, AttributeDataType::Float32) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(4*dim) {
                    let x = f32::from_le_bytes([c[0],c[1],c[2],c[3]]);
                    let y = f32::from_le_bytes([c[4],c[5],c[6],c[7]]);
                    let z = if dim>2 { f32::from_le_bytes([c[8],c[9],c[10],c[11]]) } else { 1.0 };
                    v.push([x,y,z]);
                }
                nrm_opt = Some(v);
            }
            _ => {}
        }
    }

    let start = out_vertices.len();
    let positions = pos_opt.context("decoded POSITION missing")?;
    let normals = nrm_opt.unwrap_or_else(|| vec![[0.0,1.0,0.0]; positions.len()]);
    for i in 0..positions.len() {
        out_vertices.push(Vertex { pos: positions[i], nrm: normals[i] });
    }
    // Rebase recently appended indices by start
    let start_u32 = start as u32;
    let added = out_vertices.len() as u32 - start_u32;
    if index_count == 0 {
        // Generate trivial indices if none provided
        for i in 0..added { out_indices.push((start_u32 + i) as u16); }
    } else {
        let base = out_indices.len() - (index_count as usize);
        for i in base..out_indices.len() {
            let v = out_indices[i] as u32 + start_u32;
            out_indices[i] = u16::try_from(v).map_err(|_| anyhow!("rebased index {} exceeds u16", v))?;
        }
    }
    Ok(())
}

/// Draco decode for skinned primitive: fills VertexSkinCPU with JOINTS_0/WEIGHTS_0 and UVs.
fn decode_draco_skinned_primitive(
    doc: &gltf::Document,
    buffers: &Vec<Data>,
    prim: &gltf::mesh::Primitive,
    out_vertices: &mut Vec<VertexSkinCPU>,
    out_indices: &mut Vec<u16>,
) -> Result<()> {
    // Parse extension JSON for bufferView and attribute ID map
    let Some(ext_val) = prim.extension_value("KHR_draco_mesh_compression") else { return Ok(()); };
    let obj = ext_val.as_object().context("draco ext not an object")?;
    let bv_index = obj.get("bufferView").and_then(|v| v.as_u64()).context("draco bufferView missing")? as usize;
    let attr_map = obj.get("attributes").and_then(|v| v.as_object()).context("draco attributes missing")?;

    // Resolve compressed bytes from bufferView
    let bv = doc.views().nth(bv_index).context("bufferView index out of range")?;
    let buf = bv.buffer();
    let data = &buffers[buf.index()].0;
    let start = bv.offset();
    let end = start + bv.length();
    let bytes = &data[start..end];

    // Fetch index count from primitive accessor
    let index_count = prim.indices().map(|a| a.count()).unwrap_or(0) as u32;
    let vertex_count = prim.attributes().next().map(|(_, a)| a.count()).unwrap_or(0) as u32;
    if vertex_count == 0 { bail!("draco skinned: no vertices"); }

    // Build decode config in order of attribute ids
    let mut mapped: Vec<(u32, (&str, usize))> = vec![];
    for (k, v) in attr_map.iter() {
        if let Some(acc_idx) = v.as_u64() { mapped.push((acc_idx as u32, (k.as_str(), acc_idx as usize))); }
    }
    mapped.sort_by_key(|(id, _)| *id);

    let mut cfg = MeshDecodeConfig::new(vertex_count, index_count);
    for (_, (sem_name, acc_idx)) in &mapped {
        let acc = &doc.accessors().nth(*acc_idx).context("draco accessor missing")?;
        let dims = acc.dimensions();
        let dim = match dims { gltf::accessor::Dimensions::Scalar=>1, gltf::accessor::Dimensions::Vec2=>2, gltf::accessor::Dimensions::Vec3=>3, gltf::accessor::Dimensions::Vec4=>4, _=>3 };
        let cty = acc.data_type();
        let ty = match cty { gltf::accessor::DataType::F32=>AttributeDataType::Float32, gltf::accessor::DataType::U16=>AttributeDataType::UInt16, gltf::accessor::DataType::U8=>AttributeDataType::UInt8, gltf::accessor::DataType::I16=>AttributeDataType::Int16, gltf::accessor::DataType::I8=>AttributeDataType::Int8, gltf::accessor::DataType::U32=>AttributeDataType::UInt32 };
        cfg.add_attribute(dim as u32, ty);
        let _ = sem_name;
    }

    let decoded = pollster::block_on(decode_mesh(bytes, &cfg)).context("draco native decode failed")?;

    // Walk buffer: first indices, then attributes in mapped order
    let mut off = 0usize;
    let idx_bytes = if index_count <= u16::MAX as u32 { (index_count as usize) * 2 } else { (index_count as usize) * 4 };
    if idx_bytes > 0 {
        let idx_slice = &decoded[off..off+idx_bytes];
        off += idx_bytes;
        if index_count <= u16::MAX as u32 {
            for c in idx_slice.chunks_exact(2) { out_indices.push(u16::from_le_bytes([c[0],c[1]])); }
        } else {
            for c in idx_slice.chunks_exact(4) {
                let v = u32::from_le_bytes([c[0],c[1],c[2],c[3]]);
                out_indices.push(u16::try_from(v).map_err(|_| anyhow!("decoded index {} exceeds u16", v))?);
            }
        }
    }

    // Temp storage
    let mut pos_opt: Option<Vec<[f32;3]>> = None;
    let mut nrm_opt: Option<Vec<[f32;3]>> = None;
    let mut uv_opt: Option<Vec<[f32;2]>> = None;
    let mut joints_opt: Option<Vec<[u16;4]>> = None;
    let mut weights_opt: Option<Vec<[f32;4]>> = None;

    for (_, (sem_name, acc_idx)) in &mapped {
        let acc = &doc.accessors().nth(*acc_idx).context("draco accessor missing")?;
        let dims = acc.dimensions();
        let dim = match dims { gltf::accessor::Dimensions::Scalar=>1usize, gltf::accessor::Dimensions::Vec2=>2usize, gltf::accessor::Dimensions::Vec3=>3usize, gltf::accessor::Dimensions::Vec4=>4usize, _=>3usize };
        let ctype = acc.data_type();
        let comp_size = match ctype { gltf::accessor::DataType::F32|gltf::accessor::DataType::U32 => 4usize, gltf::accessor::DataType::U16|gltf::accessor::DataType::I16 => 2usize, _ => 1usize };
        let byte_len = dim * (vertex_count as usize) * comp_size;
        let slice = &decoded[off..off+byte_len];
        off += byte_len;

        match (*sem_name, ctype) {
            ("POSITION", gltf::accessor::DataType::F32) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(4*dim) {
                    let x = f32::from_le_bytes([c[0],c[1],c[2],c[3]]);
                    let y = f32::from_le_bytes([c[4],c[5],c[6],c[7]]);
                    let z = if dim>2 { f32::from_le_bytes([c[8],c[9],c[10],c[11]]) } else { 0.0 };
                    v.push([x,y,z]);
                }
                pos_opt = Some(v);
            }
            ("NORMAL", gltf::accessor::DataType::F32) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(4*dim) {
                    let x = f32::from_le_bytes([c[0],c[1],c[2],c[3]]);
                    let y = f32::from_le_bytes([c[4],c[5],c[6],c[7]]);
                    let z = if dim>2 { f32::from_le_bytes([c[8],c[9],c[10],c[11]]) } else { 1.0 };
                    v.push([x,y,z]);
                }
                nrm_opt = Some(v);
            }
            ("TEXCOORD_0", gltf::accessor::DataType::F32) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(4*dim) {
                    let u = f32::from_le_bytes([c[0],c[1],c[2],c[3]]);
                    let w = f32::from_le_bytes([c[4],c[5],c[6],c[7]]);
                    v.push([u,w]);
                }
                uv_opt = Some(v);
            }
            ("JOINTS_0", gltf::accessor::DataType::U8) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(dim) {
                    let a = c[0] as u16; let b = if dim>1 { c[1] as u16 } else { 0 }; let d = if dim>2 { c[2] as u16 } else { 0 }; let e = if dim>3 { c[3] as u16 } else { 0 };
                    v.push([a,b,d,e]);
                }
                joints_opt = Some(v);
            }
            ("JOINTS_0", gltf::accessor::DataType::U16) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(2*dim) {
                    let a = u16::from_le_bytes([c[0],c[1]]);
                    let b = if dim>1 { u16::from_le_bytes([c[2],c[3]]) } else { 0 };
                    let d = if dim>2 { u16::from_le_bytes([c[4],c[5]]) } else { 0 };
                    let e = if dim>3 { u16::from_le_bytes([c[6],c[7]]) } else { 0 };
                    v.push([a,b,d,e]);
                }
                joints_opt = Some(v);
            }
            ("WEIGHTS_0", gltf::accessor::DataType::F32) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(4*dim) {
                    let a = f32::from_le_bytes([c[0],c[1],c[2],c[3]]);
                    let b = if dim>1 { f32::from_le_bytes([c[4],c[5],c[6],c[7]]) } else { 0.0 };
                    let d = if dim>2 { f32::from_le_bytes([c[8],c[9],c[10],c[11]]) } else { 0.0 };
                    let e = if dim>3 { f32::from_le_bytes([c[12],c[13],c[14],c[15]]) } else { 0.0 };
                    v.push([a,b,d,e]);
                }
                weights_opt = Some(v);
            }
            ("WEIGHTS_0", gltf::accessor::DataType::U16) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(2*dim) {
                    let a = u16::from_le_bytes([c[0],c[1]]) as f32 / 65535.0;
                    let b = if dim>1 { u16::from_le_bytes([c[2],c[3]]) as f32 / 65535.0 } else { 0.0 };
                    let d = if dim>2 { u16::from_le_bytes([c[4],c[5]]) as f32 / 65535.0 } else { 0.0 };
                    let e = if dim>3 { u16::from_le_bytes([c[6],c[7]]) as f32 / 65535.0 } else { 0.0 };
                    v.push([a,b,d,e]);
                }
                weights_opt = Some(v);
            }
            ("WEIGHTS_0", gltf::accessor::DataType::U8) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(dim) {
                    let a = (c[0] as f32) / 255.0; let b = if dim>1 { c[1] as f32 / 255.0 } else { 0.0 }; let d = if dim>2 { c[2] as f32 / 255.0 } else { 0.0 }; let e = if dim>3 { c[3] as f32 / 255.0 } else { 0.0 };
                    v.push([a,b,d,e]);
                }
                weights_opt = Some(v);
            }
            _ => {}
        }
    }

    let pos = pos_opt.context("decoded POSITION missing")?;
    let nrm = nrm_opt.unwrap_or_else(|| vec![[0.0,1.0,0.0]; pos.len()]);
    let uv = uv_opt.unwrap_or_else(|| pos.iter().map(|p| [0.5 + 0.5*p[0], 0.5 - 0.5*p[2]]).collect());
    let joints = joints_opt.context("decoded JOINTS_0 missing")?;
    let weights = weights_opt.context("decoded WEIGHTS_0 missing")?;
    for i in 0..pos.len() {
        out_vertices.push(VertexSkinCPU { pos: pos[i], nrm: nrm[i], joints: joints[i], weights: weights[i], uv: uv[i] });
    }
    Ok(())
}

/// Prepare a glTF for loading: prefer `<name>.decompressed.gltf` if present.
/// If import fails due to Draco compression, attempt to auto-decompress once
/// using glTF-Transform via `npx` or a globally installed `gltf-transform`.
/// Returns a path guaranteed to import (or an error if it cannot be prepared).
pub fn prepare_gltf_path(path: &Path) -> Result<PathBuf> {
    let decompressed = path.with_extension("decompressed.gltf");
    // Prefer original if it imports successfully.
    if gltf::import(path).is_ok() {
        return Ok(path.to_path_buf());
    }
    // If original fails but a decompressed copy exists and imports, use it.
    if decompressed.exists() {
        if gltf::import(&decompressed).is_ok() {
            log::warn!("anim diag: original failed; using decompressed copy: {}", decompressed.display());
            return Ok(decompressed);
        }
    }
    // Try to auto-decompress assuming Draco.
    if let Some(out) = try_gltf_transform_decompress(path, &decompressed) {
        log::info!("auto-decompressed {} -> {}", path.display(), out.display());
        Ok(out)
    } else {
        Err(anyhow!(
            "failed to prepare glTF: {} (Draco decompress failed). Install Node and run:\n  npx -y @gltf-transform/cli draco {} {} --decode",
            path.display(), path.display(), decompressed.display()
        ))
    }
}

fn try_gltf_transform_decompress(input: &Path, output: &Path) -> Option<PathBuf> {
    // Candidate invocations: prefer '@gltf-transform/cli', then 'gltf-transform'.
    // Correct argument order for v4+: `draco <in> <out> --decode`
    let variants: Vec<Vec<&OsStr>> = vec![
        vec![OsStr::new("@gltf-transform/cli"), OsStr::new("draco"), input.as_os_str(), output.as_os_str(), OsStr::new("--decode")],
        vec![OsStr::new("gltf-transform"), OsStr::new("draco"), input.as_os_str(), output.as_os_str(), OsStr::new("--decode")],
        // Older CLI fallback
        vec![OsStr::new("@gltf-transform/cli"), OsStr::new("decompress"), input.as_os_str(), output.as_os_str()],
        vec![OsStr::new("gltf-transform"), OsStr::new("decompress"), input.as_os_str(), output.as_os_str()],
    ];

    // Try via npx
    if let Ok(npx) = which::which("npx") {
        for args in &variants {
            let status = std::process::Command::new(npx.as_os_str()).arg("-y").args(args).status();
            if let Ok(s) = status {
                if s.success() && output.exists() { return Some(output.to_path_buf()); }
            }
        }
    }
    // Try global binary directly
    for args in &variants {
        let Some(cmd) = args.first() else { continue };
        if which::which(cmd).is_ok() {
            let status = std::process::Command::new(cmd).args(&args[1..]).status();
            if let Ok(s) = status {
                if s.success() && output.exists() { return Some(output.to_path_buf()); }
            }
        }
    }
    None
}
