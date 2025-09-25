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
use crate::gfx::Vertex;

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
}

pub fn load_gltf_skinned(path: &Path) -> Result<SkinnedMeshCPU> {
    let (doc, buffers, _images) = gltf::import(path).with_context(|| format!("import skinned glTF: {}", path.display()))?;

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

    'outer: for scene in doc.scenes() { for node in scene.nodes() {
        if let Some(mesh) = node.mesh() {
            if let Some(skin) = node.skin() { skin_opt = Some(skin); }
            for prim in mesh.primitives() {
                let reader = prim.reader(|b| buffers.get(b.index()).map(|bb| bb.0.as_slice()));
                let Some(pos_it) = reader.read_positions() else { continue };
                let Some(nrm_it) = reader.read_normals() else { continue };
                let joints = match reader.read_joints(0) { Some(gltf::mesh::util::ReadJoints::U16(it)) => it.map(|v| [v[0],v[1],v[2],v[3]]).collect::<Vec<[u16;4]>>(), Some(gltf::mesh::util::ReadJoints::U8(it)) => it.map(|v| [v[0] as u16,v[1] as u16,v[2] as u16,v[3] as u16]).collect(), _ => continue };
                let weights = match reader.read_weights(0) { Some(gltf::mesh::util::ReadWeights::F32(it)) => it.collect::<Vec<[f32;4]>>(), Some(gltf::mesh::util::ReadWeights::U16(it)) => it.map(|v| [v[0] as f32/65535.0, v[1] as f32/65535.0, v[2] as f32/65535.0, v[3] as f32/65535.0]).collect(), Some(gltf::mesh::util::ReadWeights::U8(it)) => it.map(|v| [v[0] as f32/255.0, v[1] as f32/255.0, v[2] as f32/255.0, v[3] as f32/255.0]).collect(), None => continue };

                let pos: Vec<[f32;3]> = pos_it.collect();
                let nrm: Vec<[f32;3]> = nrm_it.collect();
                for i in 0..pos.len() {
                    verts.push(VertexSkinCPU { pos: pos[i], nrm: nrm[i], joints: joints[i], weights: weights[i] });
                }
                let idx_u32: Vec<u32> = match reader.read_indices() { Some(gltf::mesh::util::ReadIndices::U16(it)) => it.map(|v| v as u32).collect(), Some(gltf::mesh::util::ReadIndices::U32(it)) => it.collect(), Some(gltf::mesh::util::ReadIndices::U8(it)) => it.map(|v| v as u32).collect(), None => (0..pos.len() as u32).collect() };
                for i in idx_u32 { if i > u16::MAX as u32 { bail!("wizard indices exceed u16"); } indices.push(i as u16); }
                break 'outer;
            }
        }
    }}

    let skin = skin_opt.context("skinned mesh must be under a node with a Skin")?;
    let joints_nodes: Vec<usize> = skin.joints().map(|j| j.index()).collect();
    let inverse_bind = {
        let rdr = skin.reader(|b| buffers.get(b.index()).map(|bb| bb.0.as_slice()));
        match rdr.read_inverse_bind_matrices() {
            Some(iter) => iter.map(|m| Mat4::from_cols_array_2d(&m)).collect(),
            None => vec![Mat4::IDENTITY; joints_nodes.len()],
        }
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

    Ok(SkinnedMeshCPU { vertices: verts, indices, joints_nodes, inverse_bind, parent, base_t, base_r, base_s, animations })
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
    // Use the high-level importer which resolves external buffers/images.
    let (doc, buffers, _images) = gltf::import(&source_path).with_context(|| format!(
        "failed to import glTF: {}",
        source_path.display()
    ))?;

    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();

    for mesh in doc.meshes() {
        for prim in mesh.primitives() {
            // Reader to access attribute/index data resolved by importer.
            let reader = prim.reader(|buf| buffers.get(buf.index()).map(|b| b.0.as_slice()));

            // Positions are required for our purposes; skip primitive if missing.
            let Some(pos_iter) = reader.read_positions() else { continue };

            // Normals may be absent; use a fallback.
            let nrm_iter_opt = reader.read_normals();

            // Collect into temporary arrays so we can reindex.
            let start = vertices.len() as u32;
            for (i, p) in pos_iter.enumerate() {
                let n = nrm_iter_opt
                    .as_ref()
                    .and_then(|it| it.clone().nth(i))
                    .unwrap_or([0.0, 1.0, 0.0]);
                vertices.push(Vertex { pos: p, nrm: n });
            }

            // Indices are optional in glTF (triangles can be implicit). We require them.
            let idx_iter = match reader.read_indices() {
                Some(gltf::mesh::util::ReadIndices::U16(it)) => it.map(|i| i as u32).collect::<Vec<u32>>(),
                Some(gltf::mesh::util::ReadIndices::U32(it)) => it.collect::<Vec<u32>>(),
                Some(gltf::mesh::util::ReadIndices::U8(it)) => it.map(|i| i as u32).collect::<Vec<u32>>(),
                None => {
                    // If indices are not present, generate them assuming triangles.
                    // glTF primitive mode default is triangles. Use a simple 0..N fan.
                    // We can’t know original topology without mode; keep it simple.
                    let added = (vertices.len() as u32 - start) as usize;
                    if added % 3 != 0 {
                        bail!("primitive without indices has non-multiple-of-3 vertex count");
                    }
                    (0..added as u32).map(|i| i + start).collect::<Vec<u32>>()
                }
            };

            for i in idx_iter {
                let idx = i + start;
                if idx > u16::MAX as u32 {
                    return Err(anyhow!(
                        "index {} exceeds u16::MAX ({}). Consider splitting/decimating the mesh.",
                        idx,
                        u16::MAX
                    ));
                }
                indices.push(idx as u16);
            }
        }
    }

    if vertices.is_empty() || indices.is_empty() {
        bail!("no vertices/indices found in glTF: {}", path.display());
    }

    Ok(CpuMesh { vertices, indices })
}
