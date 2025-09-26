//! Draco decode helpers (native path) for primitives.

use anyhow::{anyhow, bail, Context, Result};
use draco_decoder::{decode_mesh, AttributeDataType, MeshDecodeConfig};
use gltf::{buffer::Data, mesh::Semantic};

use crate::assets::types::{VertexSkinCPU};
use crate::gfx::Vertex;

/// Decode a Draco-compressed primitive into POSITION/NORMAL vertices and indices.
pub(crate) fn decode_draco_primitive(
    doc: &gltf::Document,
    buffers: &Vec<Data>,
    prim: &gltf::mesh::Primitive,
    out_vertices: &mut Vec<Vertex>,
    out_indices: &mut Vec<u16>,
) -> Result<()> {
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

    // Counts
    let pos_accessor = prim.get(&Semantic::Positions).context("POSITION accessor missing")?;
    let vertex_count = pos_accessor.count() as u32;
    let index_count = prim.indices().map(|a| a.count() as u32).unwrap_or(0);

    // Build decode config
    let mut cfg = MeshDecodeConfig::new(vertex_count, index_count);
    // Map semantics by attribute id, sorted
    let mut mapped: Vec<(u32, Semantic)> = Vec::new();
    for (k, v) in attr_map.iter() {
        let id = v.as_u64().unwrap_or(0) as u32;
        let sem = match k.as_str() {
            "POSITION" => Semantic::Positions,
            "NORMAL" => Semantic::Normals,
            s if s.starts_with("TEXCOORD_") => {
                let set: u32 = s[9..].parse().unwrap_or(0);
                Semantic::TexCoords(set)
            }
            _ => continue,
        };
        mapped.push((id, sem));
    }
    mapped.sort_by_key(|(id, _)| *id);
    for (_, sem) in &mapped {
        let acc = prim.get(sem).context("accessor for mapped semantic missing")?;
        let dim = match acc.dimensions() {
            gltf::accessor::Dimensions::Vec2 => 2,
            gltf::accessor::Dimensions::Vec3 => 3,
            gltf::accessor::Dimensions::Vec4 => 4,
            _ => 3,
        };
        let ty = match acc.data_type() {
            gltf::accessor::DataType::F32 => AttributeDataType::Float32,
            gltf::accessor::DataType::U16 => AttributeDataType::UInt16,
            gltf::accessor::DataType::U8 => AttributeDataType::UInt8,
            gltf::accessor::DataType::I16 => AttributeDataType::Int16,
            gltf::accessor::DataType::I8 => AttributeDataType::Int8,
            gltf::accessor::DataType::U32 => AttributeDataType::UInt32,
        };
        cfg.add_attribute(dim as u32, ty);
    }

    let decoded = pollster::block_on(decode_mesh(comp_bytes, &cfg)).context("draco native decode failed")?;

    // Parse decoded stream
    let mut off = 0usize;
    let idx_bytes = if index_count <= u16::MAX as u32 { (index_count as usize) * 2 } else { (index_count as usize) * 4 };
    if idx_bytes > 0 {
        let idx_slice = &decoded[off..off + idx_bytes];
        off += idx_bytes;
        if index_count <= u16::MAX as u32 {
            for c in idx_slice.chunks_exact(2) {
                out_indices.push(u16::from_le_bytes([c[0], c[1]]));
            }
        } else {
            for c in idx_slice.chunks_exact(4) {
                let v = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                out_indices.push(u16::try_from(v).map_err(|_| anyhow!("decoded index {} exceeds u16", v))?);
            }
        }
    }

    // Attributes
    let mut pos_opt: Option<Vec<[f32; 3]>> = None;
    let mut nrm_opt: Option<Vec<[f32; 3]>> = None;
    for (_, sem) in &mapped {
        let acc = prim.get(sem).unwrap();
        let dim = match acc.dimensions() {
            gltf::accessor::Dimensions::Vec2 => 2usize,
            gltf::accessor::Dimensions::Vec3 => 3usize,
            gltf::accessor::Dimensions::Vec4 => 4usize,
            _ => 3usize,
        };
        let ty = match acc.data_type() {
            gltf::accessor::DataType::F32 => AttributeDataType::Float32,
            gltf::accessor::DataType::U16 => AttributeDataType::UInt16,
            gltf::accessor::DataType::U8 => AttributeDataType::UInt8,
            gltf::accessor::DataType::I16 => AttributeDataType::Int16,
            gltf::accessor::DataType::I8 => AttributeDataType::Int8,
            gltf::accessor::DataType::U32 => AttributeDataType::UInt32,
        };
        let comp_size = ty.size_in_bytes();
        let bytes_len = dim * (vertex_count as usize) * comp_size;
        let slice = &decoded[off..off + bytes_len];
        off += bytes_len;

        match (sem, ty) {
            (Semantic::Positions, AttributeDataType::Float32) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(4 * dim) {
                    let x = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                    let y = f32::from_le_bytes([c[4], c[5], c[6], c[7]]);
                    let z = if dim > 2 { f32::from_le_bytes([c[8], c[9], c[10], c[11]]) } else { 0.0 };
                    v.push([x, y, z]);
                }
                pos_opt = Some(v);
            }
            (Semantic::Normals, AttributeDataType::Float32) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(4 * dim) {
                    let x = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                    let y = f32::from_le_bytes([c[4], c[5], c[6], c[7]]);
                    let z = if dim > 2 { f32::from_le_bytes([c[8], c[9], c[10], c[11]]) } else { 1.0 };
                    v.push([x, y, z]);
                }
                nrm_opt = Some(v);
            }
            _ => {}
        }
    }

    let start = out_vertices.len();
    let pos = pos_opt.context("decoded POSITION missing")?;
    let nrm = nrm_opt.unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; pos.len()]);
    for i in 0..pos.len() {
        out_vertices.push(Vertex { pos: pos[i], nrm: nrm[i] });
    }
    // Rebase indices for this primitive
    let start_u = start as u32;
    if index_count == 0 {
        for i in 0..(pos.len() as u32) {
            out_indices.push((start_u + i) as u16);
        }
    } else {
        let base = out_indices.len() - (index_count as usize);
        for i in base..out_indices.len() {
            let v = out_indices[i] as u32 + start_u;
            out_indices[i] = u16::try_from(v).map_err(|_| anyhow!("rebased index {} exceeds u16", v))?;
        }
    }
    Ok(())
}

/// Draco decode for skinned primitive: fills VertexSkinCPU with JOINTS_0/WEIGHTS_0 and UVs.
pub(crate) fn decode_draco_skinned_primitive(
    doc: &gltf::Document,
    buffers: &Vec<Data>,
    prim: &gltf::mesh::Primitive,
    out_vertices: &mut Vec<VertexSkinCPU>,
    out_indices: &mut Vec<u16>,
) -> Result<()> {
    let Some(ext_val) = prim.extension_value("KHR_draco_mesh_compression") else { return Ok(()); };
    let obj = ext_val.as_object().context("draco ext not an object")?;
    let bv_index = obj.get("bufferView").and_then(|v| v.as_u64()).context("draco bufferView missing")? as usize;
    let attr_map = obj.get("attributes").and_then(|v| v.as_object()).context("draco attributes missing")?;

    // Resolve compressed bytes
    let bv = doc.views().nth(bv_index).context("bufferView index out of range")?;
    let buf = bv.buffer();
    let data = &buffers[buf.index()].0;
    let start = bv.offset();
    let end = start + bv.length();
    let bytes = &data[start..end];

    let index_count = prim.indices().map(|a| a.count()).unwrap_or(0) as u32;
    let vertex_count = prim.attributes().next().map(|(_, a)| a.count()).unwrap_or(0) as u32;
    if vertex_count == 0 { bail!("draco skinned: no vertices"); }

    // Build decode config in mapped ID order
    let mut mapped: Vec<(u32, (&str, usize))> = vec![];
    for (k, v) in attr_map.iter() {
        if let Some(acc_idx) = v.as_u64() {
            mapped.push((acc_idx as u32, (k.as_str(), acc_idx as usize)));
        }
    }
    mapped.sort_by_key(|(id, _)| *id);

    let mut cfg = MeshDecodeConfig::new(vertex_count, index_count);
    for (_, (sem_name, acc_idx)) in &mapped {
        let acc = &doc.accessors().nth(*acc_idx).context("draco accessor missing")?;
        let dims = acc.dimensions();
        let dim = match dims {
            gltf::accessor::Dimensions::Scalar => 1,
            gltf::accessor::Dimensions::Vec2 => 2,
            gltf::accessor::Dimensions::Vec3 => 3,
            gltf::accessor::Dimensions::Vec4 => 4,
            _ => 3,
        };
        let cty = acc.data_type();
        let ty = match cty {
            gltf::accessor::DataType::F32 => AttributeDataType::Float32,
            gltf::accessor::DataType::U16 => AttributeDataType::UInt16,
            gltf::accessor::DataType::U8 => AttributeDataType::UInt8,
            gltf::accessor::DataType::I16 => AttributeDataType::Int16,
            gltf::accessor::DataType::I8 => AttributeDataType::Int8,
            gltf::accessor::DataType::U32 => AttributeDataType::UInt32,
        };
        cfg.add_attribute(dim as u32, ty);
        let _ = sem_name;
    }

    let decoded = pollster::block_on(decode_mesh(bytes, &cfg)).context("draco native decode failed")?;

    // Walk buffer: first indices, then attributes in mapped order
    let mut off = 0usize;
    let idx_bytes = if index_count <= u16::MAX as u32 { (index_count as usize) * 2 } else { (index_count as usize) * 4 };
    if idx_bytes > 0 {
        let idx_slice = &decoded[off..off + idx_bytes];
        off += idx_bytes;
        if index_count <= u16::MAX as u32 {
            for c in idx_slice.chunks_exact(2) { out_indices.push(u16::from_le_bytes([c[0], c[1]])); }
        } else {
            for c in idx_slice.chunks_exact(4) {
                let v = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                out_indices.push(u16::try_from(v).map_err(|_| anyhow!("decoded index {} exceeds u16", v))?);
            }
        }
    }

    // Temp storage
    let mut pos_opt: Option<Vec<[f32; 3]>> = None;
    let mut nrm_opt: Option<Vec<[f32; 3]>> = None;
    let mut uv_opt: Option<Vec<[f32; 2]>> = None;
    let mut joints_opt: Option<Vec<[u16; 4]>> = None;
    let mut weights_opt: Option<Vec<[f32; 4]>> = None;

    for (_, (sem_name, acc_idx)) in &mapped {
        let acc = &doc.accessors().nth(*acc_idx).context("draco accessor missing")?;
        let dims = acc.dimensions();
        let dim = match dims {
            gltf::accessor::Dimensions::Scalar => 1usize,
            gltf::accessor::Dimensions::Vec2 => 2usize,
            gltf::accessor::Dimensions::Vec3 => 3usize,
            gltf::accessor::Dimensions::Vec4 => 4usize,
            _ => 3usize,
        };
        let ctype = acc.data_type();
        let comp_size = match ctype {
            gltf::accessor::DataType::F32 | gltf::accessor::DataType::U32 => 4usize,
            gltf::accessor::DataType::U16 | gltf::accessor::DataType::I16 => 2usize,
            _ => 1usize,
        };
        let byte_len = dim * (vertex_count as usize) * comp_size;
        let slice = &decoded[off..off + byte_len];
        off += byte_len;

        match (*sem_name, ctype) {
            ("POSITION", gltf::accessor::DataType::F32) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(4 * dim) {
                    let x = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                    let y = f32::from_le_bytes([c[4], c[5], c[6], c[7]]);
                    let z = if dim > 2 { f32::from_le_bytes([c[8], c[9], c[10], c[11]]) } else { 0.0 };
                    v.push([x, y, z]);
                }
                pos_opt = Some(v);
            }
            ("NORMAL", gltf::accessor::DataType::F32) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(4 * dim) {
                    let x = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                    let y = f32::from_le_bytes([c[4], c[5], c[6], c[7]]);
                    let z = if dim > 2 { f32::from_le_bytes([c[8], c[9], c[10], c[11]]) } else { 1.0 };
                    v.push([x, y, z]);
                }
                nrm_opt = Some(v);
            }
            ("TEXCOORD_0", gltf::accessor::DataType::F32) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(4 * dim) {
                    let u = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                    let w = f32::from_le_bytes([c[4], c[5], c[6], c[7]]);
                    v.push([u, w]);
                }
                uv_opt = Some(v);
            }
            ("JOINTS_0", gltf::accessor::DataType::U8) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(dim) {
                    let a = c[0] as u16;
                    let b = if dim > 1 { c[1] as u16 } else { 0 };
                    let d = if dim > 2 { c[2] as u16 } else { 0 };
                    let e = if dim > 3 { c[3] as u16 } else { 0 };
                    v.push([a, b, d, e]);
                }
                joints_opt = Some(v);
            }
            ("JOINTS_0", gltf::accessor::DataType::U16) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(2 * dim) {
                    let a = u16::from_le_bytes([c[0], c[1]]);
                    let b = if dim > 1 { u16::from_le_bytes([c[2], c[3]]) } else { 0 };
                    let d = if dim > 2 { u16::from_le_bytes([c[4], c[5]]) } else { 0 };
                    let e = if dim > 3 { u16::from_le_bytes([c[6], c[7]]) } else { 0 };
                    v.push([a, b, d, e]);
                }
                joints_opt = Some(v);
            }
            ("WEIGHTS_0", gltf::accessor::DataType::F32) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(4 * dim) {
                    let a = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                    let b = if dim > 1 { f32::from_le_bytes([c[4], c[5], c[6], c[7]]) } else { 0.0 };
                    let d = if dim > 2 { f32::from_le_bytes([c[8], c[9], c[10], c[11]]) } else { 0.0 };
                    let e = if dim > 3 { f32::from_le_bytes([c[12], c[13], c[14], c[15]]) } else { 0.0 };
                    v.push([a, b, d, e]);
                }
                weights_opt = Some(v);
            }
            ("WEIGHTS_0", gltf::accessor::DataType::U16) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(2 * dim) {
                    let a = u16::from_le_bytes([c[0], c[1]]) as f32 / 65535.0;
                    let b = if dim > 1 { u16::from_le_bytes([c[2], c[3]]) as f32 / 65535.0 } else { 0.0 };
                    let d = if dim > 2 { u16::from_le_bytes([c[4], c[5]]) as f32 / 65535.0 } else { 0.0 };
                    let e = if dim > 3 { u16::from_le_bytes([c[6], c[7]]) as f32 / 65535.0 } else { 0.0 };
                    v.push([a, b, d, e]);
                }
                weights_opt = Some(v);
            }
            ("WEIGHTS_0", gltf::accessor::DataType::U8) => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for c in slice.chunks_exact(dim) {
                    let a = (c[0] as f32) / 255.0;
                    let b = if dim > 1 { c[1] as f32 / 255.0 } else { 0.0 };
                    let d = if dim > 2 { c[2] as f32 / 255.0 } else { 0.0 };
                    let e = if dim > 3 { c[3] as f32 / 255.0 } else { 0.0 };
                    v.push([a, b, d, e]);
                }
                weights_opt = Some(v);
            }
            _ => {}
        }
    }

    let pos = pos_opt.context("decoded POSITION missing")?;
    let nrm = nrm_opt.unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; pos.len()]);
    let uv = uv_opt.unwrap_or_else(|| pos.iter().map(|p| [0.5 + 0.5 * p[0], 0.5 - 0.5 * p[2]]).collect());
    let joints = joints_opt.context("decoded JOINTS_0 missing")?;
    let weights = weights_opt.context("decoded WEIGHTS_0 missing")?;
    for i in 0..pos.len() {
        out_vertices.push(VertexSkinCPU { pos: pos[i], nrm: nrm[i], joints: joints[i], weights: weights[i], uv: uv[i] });
    }
    Ok(())
}

