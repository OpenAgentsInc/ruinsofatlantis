//! Skinned mesh and animation clip loading from glTF.

use anyhow::{Context, Result, bail};
use glam::{Mat4, Quat, Vec3};
use gltf::mesh::util::ReadIndices;
use std::collections::HashMap;
use std::path::Path;

use crate::draco::decode_draco_skinned_primitive;
use crate::types::{AnimClip, SkinnedMeshCPU, TextureCPU, TrackQuat, TrackVec3, VertexSkinCPU};
#[cfg(not(target_arch = "wasm32"))]
use crate::util::prepare_gltf_path;

pub fn load_gltf_skinned(path: &Path) -> Result<SkinnedMeshCPU> {
    // On wasm, avoid std::fs by importing embedded asset bytes for known paths.
    #[cfg(target_arch = "wasm32")]
    let (doc, buffers, images) = {
        let p = path.to_string_lossy();
        if p.contains("assets/models/wizard.gltf") {
            let bytes: &'static [u8] = include_bytes!("../../../assets/models/wizard.gltf");
            gltf::import_slice(bytes).context("import skinned glTF (wizard.gltf slice)")?
        } else if p.contains("assets/models/zombie.glb") {
            let bytes: &'static [u8] = include_bytes!("../../../assets/models/zombie.glb");
            gltf::import_slice(bytes).context("import skinned glTF (zombie.glb slice)")?
        } else if p.contains("assets/models/zombie-guy.glb") {
            let bytes: &'static [u8] = include_bytes!("../../../assets/models/zombie-guy.glb");
            gltf::import_slice(bytes).context("import skinned glTF (zombie-guy.glb slice)")?
        } else if p.contains("assets/anims/universal/AnimationLibrary.glb") {
            let bytes: &'static [u8] =
                include_bytes!("../../../assets/anims/universal/AnimationLibrary.glb");
            gltf::import_slice(bytes).context("import animations (AnimationLibrary.glb slice)")?
        } else if p.contains("assets/models/ubc/godot/Superhero_Male.gltf") {
            // Prefer a prepacked GLB to satisfy slice-only import on wasm
            let bytes: &'static [u8] =
                include_bytes!("../../../assets/models/ubc/godot/Superhero_Male_packed.glb");
            gltf::import_slice(bytes).context("import skinned GLB (Superhero_Male_packed.glb)")?
        } else {
            // Fallback: try slice import if the caller embedded bytes elsewhere.
            // As a last resort, this will fail early rather than attempting std::fs.
            anyhow::bail!("wasm: unsupported skinned glTF path: {}", p);
        }
    };
    #[cfg(not(target_arch = "wasm32"))]
    let (doc, buffers, images) = {
        let prepared = prepare_gltf_path(path)?;
        gltf::import(&prepared)
            .with_context(|| format!("import skinned glTF: {}", prepared.display()))?
    };

    // Parent map and base TRS
    let node_count = doc.nodes().len();
    let mut parent = vec![None; node_count];
    for n in doc.nodes() {
        for c in n.children() {
            parent[c.index()] = Some(n.index());
        }
    }
    let mut base_t = vec![Vec3::ZERO; node_count];
    let mut base_r = vec![Quat::IDENTITY; node_count];
    let mut base_s = vec![Vec3::ONE; node_count];
    for n in doc.nodes() {
        let (t, r, s) = decompose_node(&n);
        base_t[n.index()] = t;
        base_r[n.index()] = r;
        base_s[n.index()] = s;
    }
    let node_names: Vec<String> = doc
        .nodes()
        .map(|n| n.name().unwrap_or("").to_string())
        .collect();

    // Choose the dominant skin by vertex count (UBC splits geometry across nodes sharing one skin)
    let mut best_skin_index: Option<usize> = None;
    let mut best_skin_vertices: usize = 0;
    for node in doc.nodes() {
        if let (Some(skin), Some(mesh)) = (node.skin(), node.mesh()) {
            let mut vtx = 0usize;
            for prim in mesh.primitives() {
                let reader = prim.reader(|b| buffers.get(b.index()).map(|bb| bb.0.as_slice()));
                if let Some(pos) = reader.read_positions() {
                    vtx += pos.size_hint().0;
                }
            }
            if vtx > best_skin_vertices {
                best_skin_vertices = vtx;
                best_skin_index = Some(skin.index());
            }
        }
    }
    if let Some(idx) = best_skin_index {
        log::info!(
            "skinning: selected skin index {} ({} verts)",
            idx,
            best_skin_vertices
        );
    } else {
        log::warn!("skinning: no skins found; attempting rigid fallback");
    }

    // Gather ALL primitives from nodes that reference the chosen skin (UBC is multi‑material)
    let mut skin_opt: Option<gltf::Skin> = None;
    let mut verts: Vec<VertexSkinCPU> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();
    let mut submeshes: Vec<crate::types::SubmeshCPU> = Vec::new();
    // Track a plausible baseColor texture from the largest contributing primitive.
    let mut best_tex_pixels: Option<(Vec<u8>, u32, u32)> = None;
    let mut best_tex_srbg = true;
    let mut best_vert_count: usize = 0;

    for node in doc.nodes() {
        let Some(skin) = node.skin() else {
            continue;
        };
        if let Some(sel) = best_skin_index
            && skin.index() != sel
        {
            continue;
        }
        skin_opt = Some(skin);
        if let Some(mesh) = node.mesh() {
            for prim in mesh.primitives() {
                let reader = prim.reader(|b| buffers.get(b.index()).map(|bb| bb.0.as_slice()));
                let pos_it = reader.read_positions();
                let nrm_it = reader.read_normals();
                let joints_it = reader.read_joints(0);
                let weights_it = reader.read_weights(0);
                // Accumulate non-Draco primitives with skinning
                if let (Some(pos_it), Some(nrm_it), Some(joints_it), Some(weights_it)) =
                    (pos_it, nrm_it, joints_it, weights_it)
                {
                    let base = verts.len() as u32;
                    let pos: Vec<[f32; 3]> = pos_it.collect();
                    let nrm: Vec<[f32; 3]> = nrm_it.collect();
                    let uv_set = prim
                        .material()
                        .pbr_metallic_roughness()
                        .base_color_texture()
                        .map(|ti| ti.tex_coord())
                        .unwrap_or(0);
                    let uv_opt = reader.read_tex_coords(uv_set).map(|tc| tc.into_f32());
                    let uv: Vec<[f32; 2]> = if let Some(it) = uv_opt {
                        it.collect()
                    } else {
                        pos.iter()
                            .map(|p| [0.5 + 0.5 * p[0], 0.5 - 0.5 * p[2]])
                            .collect()
                    };
                    let joints: Vec<[u16; 4]> = match joints_it {
                        gltf::mesh::util::ReadJoints::U16(it) => {
                            it.map(|v| [v[0], v[1], v[2], v[3]]).collect()
                        }
                        gltf::mesh::util::ReadJoints::U8(it) => it
                            .map(|v| [v[0] as u16, v[1] as u16, v[2] as u16, v[3] as u16])
                            .collect(),
                    };
                    let weights: Vec<[f32; 4]> = match weights_it {
                        gltf::mesh::util::ReadWeights::F32(it) => it.collect(),
                        gltf::mesh::util::ReadWeights::U16(it) => it
                            .map(|v| {
                                [
                                    v[0] as f32 / 65535.0,
                                    v[1] as f32 / 65535.0,
                                    v[2] as f32 / 65535.0,
                                    v[3] as f32 / 65535.0,
                                ]
                            })
                            .collect(),
                        gltf::mesh::util::ReadWeights::U8(it) => it
                            .map(|v| {
                                [
                                    v[0] as f32 / 255.0,
                                    v[1] as f32 / 255.0,
                                    v[2] as f32 / 255.0,
                                    v[3] as f32 / 255.0,
                                ]
                            })
                            .collect(),
                    };
                    // Append vertices
                    for i in 0..pos.len() {
                        verts.push(VertexSkinCPU {
                            pos: pos[i],
                            nrm: nrm[i],
                            joints: joints[i],
                            weights: weights[i],
                            uv: uv[i],
                        });
                    }
                    // Append (rebased) indices or synthesize if absent
                    let idx_u32: Vec<u32> = match reader.read_indices() {
                        Some(ReadIndices::U16(it)) => it.map(|v| v as u32).collect(),
                        Some(ReadIndices::U32(it)) => it.collect(),
                        Some(ReadIndices::U8(it)) => it.map(|v| v as u32).collect(),
                        None => (0..pos.len() as u32).collect(),
                    };
                    let mut added = 0u32;
                    let start_index = indices.len() as u32;
                    for i in idx_u32 {
                        let v = i + base;
                        if v > u16::MAX as u32 {
                            bail!("indices exceed u16 after rebase: {}", v);
                        }
                        indices.push(v as u16);
                        added += 1;
                    }
                    log::info!(
                        "append prim: verts={} idx={} material={}",
                        pos.len(),
                        added,
                        prim.material().name().unwrap_or("")
                    );
                    // Record submesh range and per-primitive baseColor
                    let base_tex = if let Some(texinfo) = prim
                        .material()
                        .pbr_metallic_roughness()
                        .base_color_texture()
                    {
                        let tex = texinfo.texture();
                        let img_idx = tex.source().index();
                        images.get(img_idx).map(|img| {
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
                            TextureCPU {
                                pixels,
                                width: w,
                                height: h,
                                srgb: true,
                            }
                        })
                    } else {
                        None
                    };
                    submeshes.push(crate::types::SubmeshCPU {
                        start: start_index,
                        count: added,
                        base_color_texture: base_tex,
                    });
                    // Track a plausible base color texture from the largest contributing primitive
                    if pos.len() > best_vert_count
                        && let Some(texinfo) = prim
                            .material()
                            .pbr_metallic_roughness()
                            .base_color_texture()
                    {
                        let tex = texinfo.texture();
                        let img_idx = tex.source().index();
                        if let Some(img) = images.get(img_idx) {
                            // Convert to RGBA8
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
                            best_tex_pixels = Some((pixels, w, h));
                            best_tex_srbg = true;
                            best_vert_count = pos.len();
                        }
                    }
                } else if prim.extension_value("KHR_draco_mesh_compression").is_some() {
                    // Decode Draco skinned primitive, then rebase last indices by previous vertex count.
                    let idx_start = indices.len();
                    let vtx_start = verts.len();
                    decode_draco_skinned_primitive(
                        &doc,
                        &buffers,
                        &prim,
                        &mut verts,
                        &mut indices,
                    )?;
                    let added_idx = indices.len().saturating_sub(idx_start);
                    if added_idx > 0 {
                        let base = vtx_start as u32;
                        for item in indices.iter_mut().skip(idx_start) {
                            let v = *item as u32 + base;
                            *item = u16::try_from(v).map_err(|_| {
                                anyhow::anyhow!("rebased draco index {} exceeds u16", v)
                            })?;
                        }
                        // Record submesh for Draco primitive
                        let base_tex = if let Some(texinfo) = prim
                            .material()
                            .pbr_metallic_roughness()
                            .base_color_texture()
                        {
                            let tex = texinfo.texture();
                            let img_idx = tex.source().index();
                            images.get(img_idx).map(|img| {
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
                                TextureCPU {
                                    pixels,
                                    width: w,
                                    height: h,
                                    srgb: true,
                                }
                            })
                        } else {
                            None
                        };
                        submeshes.push(crate::types::SubmeshCPU {
                            start: idx_start as u32,
                            count: added_idx as u32,
                            base_color_texture: base_tex,
                        });
                    }
                }
            }
        }
    }

    // Fallback to rigid geometry with synthesized joints/weights
    if verts.is_empty() {
        'find_any: for mesh in doc.meshes() {
            for prim in mesh.primitives() {
                let reader = prim.reader(|b| buffers.get(b.index()).map(|bb| bb.0.as_slice()));
                let Some(pos_it) = reader.read_positions() else {
                    continue;
                };
                let nrm_it = reader.read_normals();
                let pos: Vec<[f32; 3]> = pos_it.collect();
                let nrm: Vec<[f32; 3]> = nrm_it
                    .map(|it| it.collect())
                    .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; pos.len()]);
                let uv: Vec<[f32; 2]> = pos
                    .iter()
                    .map(|p| [0.5 + 0.5 * p[0], 0.5 - 0.5 * p[2]])
                    .collect();
                for i in 0..pos.len() {
                    verts.push(VertexSkinCPU {
                        pos: pos[i],
                        nrm: nrm[i],
                        joints: [0, 0, 0, 0],
                        weights: [1.0, 0.0, 0.0, 0.0],
                        uv: uv[i],
                    });
                }
                let idx_u32: Vec<u32> = match reader.read_indices() {
                    Some(ReadIndices::U16(it)) => it.map(|v| v as u32).collect(),
                    Some(ReadIndices::U32(it)) => it.collect(),
                    Some(ReadIndices::U8(it)) => it.map(|v| v as u32).collect(),
                    None => (0..pos.len() as u32).collect(),
                };
                for i in idx_u32 {
                    if i > u16::MAX as u32 {
                        bail!("indices exceed u16");
                    }
                    indices.push(i as u16);
                }
                break 'find_any;
            }
        }
    }

    if verts.is_empty()
        && doc
            .extensions_required()
            .any(|e| e == "KHR_draco_mesh_compression")
    {
        bail!(
            "GLTF uses KHR_draco_mesh_compression; please provide a pre-decompressed copy (e.g., assets/models/<name>.decompressed.gltf) using the gltf_decompress tool"
        );
    }

    // Skin
    let synth_skin = verts.is_empty() || doc.skins().next().is_none();
    let skin = if synth_skin {
        None
    } else {
        Some(skin_opt.unwrap_or_else(|| doc.skins().next().unwrap()))
    };
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

    // Animations (all clips)
    let mut animations: HashMap<String, AnimClip> = HashMap::new();
    for anim in doc.animations() {
        let name = anim.name().unwrap_or("").to_string();
        let mut t_tracks: HashMap<usize, TrackVec3> = HashMap::new();
        let mut r_tracks: HashMap<usize, TrackQuat> = HashMap::new();
        let mut s_tracks: HashMap<usize, TrackVec3> = HashMap::new();
        let mut max_t = 0.0f32;
        for ch in anim.channels() {
            let target = ch.target();
            let node_idx = target.node().index();
            let rdr = ch.reader(|b| buffers.get(b.index()).map(|bb| bb.0.as_slice()));
            let Some(inputs) = rdr.read_inputs() else {
                continue;
            };
            let times: Vec<f32> = inputs.collect();
            if let Some(&last) = times.last()
                && last > max_t
            {
                max_t = last;
            }
            match target.property() {
                gltf::animation::Property::Translation => {
                    let Some(outs) = rdr.read_outputs() else {
                        continue;
                    };
                    let vals: Vec<Vec3> = match outs {
                        gltf::animation::util::ReadOutputs::Translations(it) => {
                            it.map(Vec3::from).collect()
                        }
                        _ => continue,
                    };
                    t_tracks.insert(
                        node_idx,
                        TrackVec3 {
                            times: times.clone(),
                            values: vals,
                        },
                    );
                }
                gltf::animation::Property::Rotation => {
                    let Some(outs) = rdr.read_outputs() else {
                        continue;
                    };
                    let vals: Vec<Quat> = match outs {
                        gltf::animation::util::ReadOutputs::Rotations(it) => it
                            .into_f32()
                            .map(|v| Quat::from_xyzw(v[0], v[1], v[2], v[3]).normalize())
                            .collect(),
                        _ => continue,
                    };
                    r_tracks.insert(
                        node_idx,
                        TrackQuat {
                            times: times.clone(),
                            values: vals,
                        },
                    );
                }
                gltf::animation::Property::Scale => {
                    let Some(outs) = rdr.read_outputs() else {
                        continue;
                    };
                    let vals: Vec<Vec3> = match outs {
                        gltf::animation::util::ReadOutputs::Scales(it) => {
                            it.map(Vec3::from).collect()
                        }
                        _ => continue,
                    };
                    s_tracks.insert(
                        node_idx,
                        TrackVec3 {
                            times: times.clone(),
                            values: vals,
                        },
                    );
                }
                _ => {}
            }
        }
        animations.insert(
            name.clone(),
            AnimClip {
                name,
                duration: max_t,
                t_tracks,
                r_tracks,
                s_tracks,
            },
        );
    }

    if animations.is_empty() {
        animations.insert(
            "__static".to_string(),
            AnimClip {
                name: "__static".to_string(),
                duration: 0.0,
                t_tracks: HashMap::new(),
                r_tracks: HashMap::new(),
                s_tracks: HashMap::new(),
            },
        );
    }

    // Base color texture (optional): choose the largest contributing primitive's texture if available
    let mut base_color_texture = None;
    if let Some((pixels, w, h)) = best_tex_pixels {
        base_color_texture = Some(TextureCPU {
            pixels,
            width: w,
            height: h,
            srgb: best_tex_srbg,
        });
    }

    // Identify useful nodes for VFX
    let hand_right_node = node_names.iter().position(|n| {
        let low = n.to_lowercase();
        low.contains("hand right")
            || low.contains("right hand")
            || low.contains("hand_r")
            || low.contains("r_hand")
    });
    let root_node = node_names.iter().position(|n| {
        let low = n.to_lowercase();
        low == "root" || low.contains("armature")
    });

    Ok(SkinnedMeshCPU {
        vertices: verts,
        indices,
        joints_nodes,
        inverse_bind,
        parent,
        base_t,
        base_r,
        base_s,
        animations,
        base_color_texture,
        submeshes,
        node_names,
        hand_right_node,
        root_node,
    })
}

/// Merge animation clips from another GLTF/GLB into an existing skinned mesh by node-name mapping.
pub fn merge_gltf_animations(base: &mut SkinnedMeshCPU, anim_path: &Path) -> Result<usize> {
    let other = load_gltf_skinned(anim_path)?;
    let mut merged = 0usize;
    for (name, clip) in other.animations.iter() {
        let mut t_tracks = HashMap::new();
        let mut r_tracks = HashMap::new();
        let mut s_tracks = HashMap::new();
        let map_idx = |idx: &usize| -> Option<usize> {
            other.node_names.get(*idx).and_then(|n| {
                let nn = normalize_bone_name(n);
                base.node_names
                    .iter()
                    .position(|m| normalize_bone_name(m) == nn)
            })
        };
        for (i, tr) in &clip.t_tracks {
            if let Some(di) = map_idx(i) {
                t_tracks.insert(di, tr.clone());
            }
        }
        // Rotation retarget: bring source local rotations into target local space by
        // applying the delta from source rest onto target rest.
        for (i, rr) in &clip.r_tracks {
            if let Some(di) = map_idx(i) {
                let src_rest = other.base_r[*i];
                let tgt_rest = base.base_r[di];
                let mut new_rr = rr.clone();
                for q in &mut new_rr.values {
                    let delta = src_rest.inverse() * (*q);
                    let ret = (tgt_rest * delta).normalize();
                    *q = ret;
                }
                r_tracks.insert(di, new_rr);
            }
        }
        for (i, sr) in &clip.s_tracks {
            if let Some(di) = map_idx(i) {
                s_tracks.insert(di, sr.clone());
            }
        }
        base.animations.insert(
            name.clone(),
            AnimClip {
                name: name.clone(),
                duration: clip.duration,
                t_tracks,
                r_tracks,
                s_tracks,
            },
        );
        merged += 1;
    }
    Ok(merged)
}

fn normalize_bone_name(s: &str) -> String {
    // Lowercase and strip common rig prefixes and separators, then normalize digits and synonyms.
    let mut out = s.to_lowercase();
    for pref in [
        "mixamorig:",
        "armature|",
        "armature/",
        "armature:",
        "skeleton|",
        "skeleton/",
        "skeleton:",
        "def-",
        "rig|",
        "rig/",
        "rig:",
    ] {
        if out.starts_with(pref) {
            out = out.trim_start_matches(pref).to_string();
        }
        out = out.replace(pref, "");
    }
    // Remove common separator characters entirely
    out = out.replace([' ', '_', '-', '.', '|'], "");
    // Synonyms between libraries (best‑effort)
    out = out.replace("hips", "pelvis");
    out = out.replace("forearm", "lowerarm");
    out = out.replace("shoulder", "clavicle");
    out = out.replace("shin", "calf");
    // Collapse numeric runs to remove leading zeros (e.g., spine.003 -> spine3, spine_01 -> spine1)
    let mut collapsed = String::with_capacity(out.len());
    let mut i = 0;
    let b = out.as_bytes();
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let num_str = &out[start..i];
            if let Ok(val) = num_str.parse::<u32>() {
                collapsed.push_str(&val.to_string());
            } else {
                collapsed.push_str(num_str);
            }
            continue;
        }
        collapsed.push(b[i] as char);
        i += 1;
    }
    collapsed
}

/// Merge animation clips from an FBX file into an existing skinned mesh by node-name mapping.
///
/// See also: `crate::fbx::merge_fbx_animations`. This entry point is stable and available
/// in all builds; without the `fbx` feature it returns an error explaining how to enable it.
pub fn merge_fbx_animations(base: &mut SkinnedMeshCPU, fbx_path: &Path) -> Result<usize> {
    crate::fbx::merge_fbx_animations(base, fbx_path)
}

fn decompose_node(n: &gltf::Node) -> (Vec3, Quat, Vec3) {
    use gltf::scene::Transform;
    match n.transform() {
        Transform::Matrix { matrix } => {
            let m = Mat4::from_cols_array_2d(&matrix);
            let (s, r, t) = m.to_scale_rotation_translation();
            (t, r, s)
        }
        Transform::Decomposed {
            translation,
            rotation,
            scale,
        } => (
            Vec3::from(translation),
            Quat::from_array(rotation).normalize(),
            Vec3::from(scale),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> std::path::PathBuf {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for _ in 0..5 {
            if p.join("assets/models/wizard.gltf").exists() {
                return p;
            }
            p.pop();
        }
        panic!("could not locate repo root containing assets/models");
    }

    #[test]
    fn load_gltf_skinned_wizard() {
        let root = repo_root();
        let path = root.join("assets/models/wizard.gltf");
        let skinned = load_gltf_skinned(&path).expect("load skinned wizard");
        assert!(!skinned.vertices.is_empty(), "vertices should not be empty");
        assert!(!skinned.indices.is_empty(), "indices should not be empty");
        assert!(
            !skinned.joints_nodes.is_empty(),
            "joints_nodes should not be empty"
        );
        assert!(
            !skinned.animations.is_empty(),
            "animations should not be empty"
        );
    }
}
