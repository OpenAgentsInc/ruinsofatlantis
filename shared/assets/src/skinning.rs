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
        if skin_opt.is_none() {
            skin_opt = Some(skin);
        }
        if let Some(mesh) = node.mesh() {
            // Concatenate all primitives (materials) into one vertex/index list
            let base = verts.len() as u32;
            for prim in mesh.primitives() {
                let reader = prim.reader(|b| buffers.get(b.index()).map(|bb| bb.0.as_slice()));
                // Skip Draco-compressed prims here; we handle them separately
                if prim.extension_value("KHR_draco_mesh_compression").is_some() {
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
                    continue;
                }

                let Some(pos_it) = reader.read_positions() else {
                    continue;
                };
                let nrm_it = reader.read_normals();
                let joints_it = reader
                    .read_joints(0)
                    .or_else(|| reader.read_joints(1))
                    .or_else(|| reader.read_joints(2));
                let weights_it = reader
                    .read_weights(0)
                    .or_else(|| reader.read_weights(1))
                    .or_else(|| reader.read_weights(2));
                let uv_opt = reader.read_tex_coords(0).map(|t| t.into_f32());
                // Mandatory: joints + weights for skinning path
                let (Some(joints_it), Some(weights_it)) = (joints_it, weights_it) else {
                    continue;
                };

                let pos: Vec<[f32; 3]> = pos_it.collect();
                let nrm: Vec<[f32; 3]> = if let Some(it) = nrm_it {
                    it.collect()
                } else {
                    vec![[0.0, 1.0, 0.0]; pos.len()]
                };
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
            }
        }
    }

    let base_color_texture = best_tex_pixels.map(|(pixels, width, height)| TextureCPU {
        pixels,
        width,
        height,
        srgb: best_tex_srbg,
    });

    // Resolve chosen skin
    let Some(skin) = skin_opt else {
        // Try rigid-mesh fallback: still build vertex/index buffers and textures.
        return Ok(SkinnedMeshCPU {
            vertices: verts,
            indices,
            joints_nodes: Vec::new(),
            inverse_bind: Vec::new(),
            parent,
            base_t,
            base_r,
            base_s,
            animations: HashMap::new(),
            base_color_texture,
            submeshes,
            node_names,
            hand_right_node: None,
            root_node: None,
        });
    };

    // Inverse bind matrices
    let inverse_bind: Vec<Mat4> = {
        let acc = skin
            .inverse_bind_matrices()
            .context("skin has no inv bind")?;
        read_accessor_mats4_f32(&acc, &buffers).context("read inverse bind mats")?
    };
    let joints_nodes: Vec<usize> = skin.joints().map(|n| n.index()).collect();

    // Read animations (by local node index)
    let mut animations: HashMap<String, AnimClip> = HashMap::new();
    for anim in doc.animations() {
        let name = anim.name().unwrap_or("(clip)").to_string();
        let mut t_tracks: HashMap<usize, TrackVec3> = HashMap::new();
        let mut r_tracks: HashMap<usize, TrackQuat> = HashMap::new();
        let mut s_tracks: HashMap<usize, TrackVec3> = HashMap::new();
        let mut duration = 0.0f32;
        for ch in anim.channels() {
            let sampler = ch.sampler();
            let input = sampler.input();
            let output = sampler.output();
            let target = ch.target();
            let node_ix = target.node().index();
            let times: Vec<f32> = read_accessor_scalar_f32(&input, &buffers).unwrap_or_default();
            duration = duration.max(times.last().copied().unwrap_or(0.0));
            match target.property() {
                gltf::animation::Property::Translation => {
                    let values: Vec<Vec3> = read_accessor_vec_f32(&output, &buffers, 3)
                        .chunks(3)
                        .map(|c| Vec3::new(c[0], c[1], c[2]))
                        .collect();
                    t_tracks.insert(
                        node_ix,
                        TrackVec3 {
                            times: times.clone(),
                            values,
                        },
                    );
                }
                gltf::animation::Property::Rotation => {
                    let values: Vec<Quat> = read_accessor_vec_f32(&output, &buffers, 4)
                        .chunks(4)
                        .map(|c| Quat::from_array([c[0], c[1], c[2], c[3]]).normalize())
                        .collect();
                    r_tracks.insert(
                        node_ix,
                        TrackQuat {
                            times: times.clone(),
                            values,
                        },
                    );
                }
                gltf::animation::Property::Scale => {
                    let values: Vec<Vec3> = read_accessor_vec_f32(&output, &buffers, 3)
                        .chunks(3)
                        .map(|c| Vec3::new(c[0], c[1], c[2]))
                        .collect();
                    s_tracks.insert(
                        node_ix,
                        TrackVec3 {
                            times: times.clone(),
                            values,
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
                duration,
                t_tracks,
                r_tracks,
                s_tracks,
            },
        );
    }

    // Identify right hand node and root
    let hand_right_node = node_names.iter().position(|n| {
        let low = n.to_lowercase();
        low.contains("hand.r")
            || low.contains("hand_r")
            || low.contains("rhand")
            || low.contains("right_hand")
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
        // Only merge clips that actually mapped at least one track; log mapping counts
        let mapped_t = t_tracks.len();
        let mapped_r = r_tracks.len();
        let mapped_s = s_tracks.len();
        let mapped_count = mapped_t + mapped_r + mapped_s;
        if mapped_count > 0 {
            log::info!(
                "merge: '{}' mapped tracks → T:{} R:{} S:{} (dur {:.3}s)",
                name,
                mapped_t,
                mapped_r,
                mapped_s,
                clip.duration
            );
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
        } else {
            log::warn!(
                "skinning: skipped animation '{}' from {} (no retargeted tracks)",
                name,
                anim_path.display()
            );
        }
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

// ---- Accessor reading helpers (minimal, f32 only) ----
fn read_accessor_scalar_f32(
    acc: &gltf::Accessor,
    buffers: &[gltf::buffer::Data],
) -> Option<Vec<f32>> {
    let view = acc.view()?;
    let buf = &buffers[view.buffer().index()].0;
    let off = view.offset() + acc.offset();
    let stride = view.stride().unwrap_or(4);
    let count = acc.count();
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let idx = off + i * stride;
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&buf[idx..idx + 4]);
        out.push(f32::from_le_bytes(bytes));
    }
    Some(out)
}

fn read_accessor_vec_f32(
    acc: &gltf::Accessor,
    buffers: &[gltf::buffer::Data],
    dims: usize,
) -> Vec<f32> {
    // Best-effort: assume tightly-packed f32 vectors
    let view = if let Some(v) = acc.view() {
        v
    } else {
        return Vec::new();
    };
    let buf = &buffers[view.buffer().index()].0;
    let off = view.offset() + acc.offset();
    let comp = 4usize;
    let width = dims * comp;
    let stride = view.stride().unwrap_or(width);
    let count = acc.count();
    let mut out = Vec::with_capacity(count * dims);
    for i in 0..count {
        let idx = off + i * stride;
        for k in 0..dims {
            let mut bytes = [0u8; 4];
            let start = idx + k * 4;
            bytes.copy_from_slice(&buf[start..start + 4]);
            out.push(f32::from_le_bytes(bytes));
        }
    }
    out
}

fn read_accessor_mats4_f32(
    acc: &gltf::Accessor,
    buffers: &[gltf::buffer::Data],
) -> Option<Vec<Mat4>> {
    let view = acc.view()?;
    let buf = &buffers[view.buffer().index()].0;
    let off = view.offset() + acc.offset();
    let stride = view.stride().unwrap_or(16 * 4);
    let count = acc.count();
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let base = off + i * stride;
        let mut m = [0.0f32; 16];
        for (k, mk) in m.iter_mut().enumerate() {
            let mut bytes = [0u8; 4];
            let idx = base + k * 4;
            bytes.copy_from_slice(&buf[idx..idx + 4]);
            *mk = f32::from_le_bytes(bytes);
        }
        out.push(Mat4::from_cols_array(&m));
    }
    Some(out)
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
