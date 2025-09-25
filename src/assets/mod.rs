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
                for i in 0..pos.len() {
                    verts.push(VertexSkinCPU { pos: pos[i], nrm: nrm[i], joints: [0,0,0,0], weights: [1.0, 0.0, 0.0, 0.0] });
                }
                let idx_u32: Vec<u32> = match reader.read_indices() { Some(gltf::mesh::util::ReadIndices::U16(it)) => it.map(|v| v as u32).collect(), Some(gltf::mesh::util::ReadIndices::U32(it)) => it.collect(), Some(gltf::mesh::util::ReadIndices::U8(it)) => it.map(|v| v as u32).collect(), None => (0..pos.len() as u32).collect() };
                for i in idx_u32 { if i > u16::MAX as u32 { bail!("indices exceed u16"); } indices.push(i as u16); }
                break 'find_any;
            }
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

    if animations.is_empty() {
        animations.insert("__static".to_string(), AnimClip { name: "__static".to_string(), duration: 0.0, t_tracks: HashMap::new(), r_tracks: HashMap::new(), s_tracks: HashMap::new() });
    }

    // Final guard: ensure we have non-empty geometry
    if verts.is_empty() || indices.is_empty() {
        bail!("no renderable geometry found in {}", path.display());
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
    let (doc, buffers, _images) = gltf::import(&source_path).with_context(|| format!(
        "failed to import glTF: {}",
        source_path.display()
    ))?;

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

/// Prepare a glTF for loading: prefer `<name>.decompressed.gltf` if present.
/// If import fails due to Draco compression, attempt to auto-decompress once
/// using glTF-Transform via `npx` or a globally installed `gltf-transform`.
/// Returns a path guaranteed to import (or an error if it cannot be prepared).
pub fn prepare_gltf_path(path: &Path) -> Result<PathBuf> {
    let decompressed = path.with_extension("decompressed.gltf");
    if decompressed.exists() {
        return Ok(decompressed);
    }
    // Try import quickly; if it works, return original path.
    if gltf::import(path).is_ok() {
        return Ok(path.to_path_buf());
    }
    // Import failed — try to auto-decompress assuming Draco.
    if let Some(out) = try_gltf_transform_decompress(path, &decompressed) {
        log::info!("auto-decompressed {} -> {}", path.display(), decompressed.display());
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
