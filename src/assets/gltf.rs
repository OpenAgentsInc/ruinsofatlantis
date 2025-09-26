//! GLTF loading: unskinned CPU mesh and Draco JSON fallback.

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use gltf::mesh::util::ReadIndices;
use std::path::{Path, PathBuf};

use crate::assets::draco::decode_draco_primitive;
use crate::assets::types::CpuMesh;
use crate::gfx::Vertex;

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
            let uses_draco = prim.extension_value("KHR_draco_mesh_compression").is_some();
            if uses_draco {
                decode_draco_primitive(&doc, &buffers, &prim, &mut vertices, &mut indices)?;
                continue;
            }

            let reader = prim.reader(|b| buffers.get(b.index()).map(|bb| bb.0.as_slice()));
            let pos = match reader.read_positions() {
                Some(it) => it.collect::<Vec<[f32; 3]>>(),
                None => continue,
            };
            let nrm: Vec<[f32; 3]> = match reader.read_normals() {
                Some(it) => it.collect(),
                None => vec![[0.0, 1.0, 0.0]; pos.len()],
            };
            let start = vertices.len();
            for i in 0..pos.len() {
                vertices.push(Vertex {
                    pos: pos[i],
                    nrm: nrm[i],
                });
            }
            let start_u = start as u32;
            let indices_read: Vec<u32> = match reader.read_indices() {
                Some(ReadIndices::U16(it)) => it.map(|v| v as u32).collect(),
                Some(ReadIndices::U32(it)) => it.collect(),
                Some(ReadIndices::U8(it)) => it.map(|v| v as u32).collect(),
                None => (0..pos.len() as u32).collect(),
            };
            if indices_read.is_empty() {
                for i in 0..(pos.len() as u32) {
                    indices.push((start_u + i) as u16);
                }
            } else {
                for v in indices_read {
                    let vv = start_u + v;
                    indices.push(
                        u16::try_from(vv)
                            .map_err(|_| anyhow!("rebased index {} exceeds u16", vv))?,
                    );
                }
            }
        }
    }

    if vertices.is_empty() || indices.is_empty() {
        bail!("no geometry found in {}", path.display());
    }
    Ok(CpuMesh { vertices, indices })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_gltf_mesh_wizard() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let path = root.join("assets/models/wizard.gltf");
        let mesh = load_gltf_mesh(&path).expect("load wizard.gltf");
        assert!(
            !mesh.vertices.is_empty(),
            "wizard vertices should not be empty"
        );
        assert!(
            !mesh.indices.is_empty(),
            "wizard indices should not be empty"
        );
    }

    #[test]
    fn load_gltf_mesh_ruins_draco() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let path = root.join("assets/models/ruins.gltf");
        let mesh = load_gltf_mesh(&path).expect("load ruins.gltf (Draco)");
        assert!(
            !mesh.vertices.is_empty(),
            "ruins vertices should not be empty"
        );
        assert!(
            !mesh.indices.is_empty(),
            "ruins indices should not be empty"
        );
    }
}

fn try_load_gltf_draco_json(path: &Path) -> Result<CpuMesh> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read glTF json: {}", path.display()))?;
    let v: serde_json::Value = serde_json::from_str(&text).context("parse glTF JSON")?;
    let empty = Vec::new();
    let ext_req = v
        .get("extensionsRequired")
        .and_then(|x| x.as_array())
        .unwrap_or(&empty);
    let has_draco = ext_req
        .iter()
        .any(|s| s.as_str() == Some("KHR_draco_mesh_compression"));
    if !has_draco {
        bail!("JSON fallback: no KHR_draco_mesh_compression present");
    }

    // Decode buffers (support only data: URIs here)
    let buffers = v
        .get("buffers")
        .and_then(|b| b.as_array())
        .context("buffers missing")?;
    let mut bin_bytes: Vec<Vec<u8>> = Vec::new();
    for b in buffers {
        let uri = b
            .get("uri")
            .and_then(|u| u.as_str())
            .context("buffer.uri missing")?;
        if let Some(idx) = uri.find(',') {
            let b64 = &uri[(idx + 1)..];
            let data = BASE64
                .decode(b64.as_bytes())
                .context("base64 decode buffer")?;
            bin_bytes.push(data);
        } else {
            bail!("only data: URIs are supported in JSON fallback");
        }
    }

    let views = v
        .get("bufferViews")
        .and_then(|x| x.as_array())
        .context("bufferViews missing")?;
    let accessors = v
        .get("accessors")
        .and_then(|x| x.as_array())
        .context("accessors missing")?;
    let meshes = v
        .get("meshes")
        .and_then(|x| x.as_array())
        .context("meshes missing")?;

    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();

    for mesh in meshes {
        let empty_p = Vec::new();
        let prims = mesh
            .get("primitives")
            .and_then(|p| p.as_array())
            .unwrap_or(&empty_p);
        for prim in prims {
            let ext = prim
                .get("extensions")
                .and_then(|e| e.get("KHR_draco_mesh_compression"));
            if ext.is_none() {
                continue;
            }
            let ext = ext.unwrap();
            let bv_index = ext
                .get("bufferView")
                .and_then(|b| b.as_u64())
                .context("draco bufferView missing")? as usize;
            let attr_map = ext
                .get("attributes")
                .and_then(|a| a.as_object())
                .context("draco attributes missing")?;

            let bv = &views[bv_index];
            let buf_index = bv.get("buffer").and_then(|b| b.as_u64()).unwrap_or(0) as usize;
            let byte_offset = bv.get("byteOffset").and_then(|b| b.as_u64()).unwrap_or(0) as usize;
            let byte_length = bv
                .get("byteLength")
                .and_then(|b| b.as_u64())
                .context("byteLength missing")? as usize;
            let data = &bin_bytes[buf_index][byte_offset..byte_offset + byte_length];

            // Vertex/index counts & attribute dims/types
            let attrs = prim
                .get("attributes")
                .and_then(|a| a.as_object())
                .context("primitive.attributes missing")?;
            let pos_acc_idx = attrs
                .get("POSITION")
                .and_then(|i| i.as_u64())
                .context("POSITION accessor missing")? as usize;
            let pos_acc = &accessors[pos_acc_idx];
            let vertex_count = pos_acc
                .get("count")
                .and_then(|c| c.as_u64())
                .context("POSITION.count missing")? as u32;
            let index_count = prim
                .get("indices")
                .and_then(|i| i.as_u64())
                .map(|idx| {
                    accessors[idx as usize]
                        .get("count")
                        .and_then(|c| c.as_u64())
                        .unwrap_or(0) as u32
                })
                .unwrap_or(0);

            let mut cfg = draco_decoder::MeshDecodeConfig::new(vertex_count, index_count);
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
                let dim = match dims {
                    "SCALAR" => 1,
                    "VEC2" => 2,
                    "VEC3" => 3,
                    "VEC4" => 4,
                    _ => 3,
                };
                let ctype = acc
                    .get("componentType")
                    .and_then(|c| c.as_u64())
                    .unwrap_or(5126);
                let ty = match ctype {
                    5126 => draco_decoder::AttributeDataType::Float32,
                    5123 => draco_decoder::AttributeDataType::UInt16,
                    5121 => draco_decoder::AttributeDataType::UInt8,
                    5122 => draco_decoder::AttributeDataType::Int16,
                    5120 => draco_decoder::AttributeDataType::Int8,
                    5125 => draco_decoder::AttributeDataType::UInt32,
                    _ => draco_decoder::AttributeDataType::Float32,
                };
                cfg.add_attribute(dim as u32, ty);
                let _ = sem_name;
            }

            let decoded = pollster::block_on(draco_decoder::decode_mesh(data, &cfg))
                .context("draco native decode failed")?;

            let mut off = 0usize;
            let idx_bytes = if index_count <= u16::MAX as u32 {
                (index_count as usize) * 2
            } else {
                (index_count as usize) * 4
            };
            if idx_bytes > 0 {
                let idx_slice = &decoded[off..off + idx_bytes];
                off += idx_bytes;
                if index_count <= u16::MAX as u32 {
                    for c in idx_slice.chunks_exact(2) {
                        indices.push(u16::from_le_bytes([c[0], c[1]]));
                    }
                } else {
                    for c in idx_slice.chunks_exact(4) {
                        let v = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                        indices.push(
                            u16::try_from(v)
                                .map_err(|_| anyhow!("decoded index {} exceeds u16", v))?,
                        );
                    }
                }
            }

            // Now parse attributes in mapped order; grab POSITION/NORMAL only
            let mut pos_opt: Option<Vec<[f32; 3]>> = None;
            let mut nrm_opt: Option<Vec<[f32; 3]>> = None;
            for (_, (sem_name, acc_idx)) in &mapped {
                let acc = &accessors[*acc_idx];
                let dims = acc.get("type").and_then(|t| t.as_str()).unwrap_or("VEC3");
                let dim = match dims {
                    "SCALAR" => 1usize,
                    "VEC2" => 2usize,
                    "VEC3" => 3usize,
                    "VEC4" => 4usize,
                    _ => 3usize,
                };
                let ctype = acc
                    .get("componentType")
                    .and_then(|c| c.as_u64())
                    .unwrap_or(5126);
                let comp_size = match ctype {
                    5126 | 5125 | 5124 => 4usize,
                    5123 | 5122 => 2usize,
                    5121 | 5120 => 1usize,
                    _ => 4usize,
                };
                let byte_len = dim * (vertex_count as usize) * comp_size;
                let slice = &decoded[off..off + byte_len];
                off += byte_len;

                match (*sem_name, ctype) {
                    ("POSITION", 5126) => {
                        let mut v = Vec::with_capacity(vertex_count as usize);
                        for c in slice.chunks_exact(4 * dim) {
                            let x = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                            let y = f32::from_le_bytes([c[4], c[5], c[6], c[7]]);
                            let z = if dim > 2 {
                                f32::from_le_bytes([c[8], c[9], c[10], c[11]])
                            } else {
                                0.0
                            };
                            v.push([x, y, z]);
                        }
                        pos_opt = Some(v);
                    }
                    ("NORMAL", 5126) => {
                        let mut v = Vec::with_capacity(vertex_count as usize);
                        for c in slice.chunks_exact(4 * dim) {
                            let x = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                            let y = f32::from_le_bytes([c[4], c[5], c[6], c[7]]);
                            let z = if dim > 2 {
                                f32::from_le_bytes([c[8], c[9], c[10], c[11]])
                            } else {
                                1.0
                            };
                            v.push([x, y, z]);
                        }
                        nrm_opt = Some(v);
                    }
                    _ => {}
                }
            }

            let start = vertices.len();
            let pos = pos_opt.context("decoded POSITION missing")?;
            let nrm = nrm_opt.unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; pos.len()]);
            for i in 0..pos.len() {
                vertices.push(Vertex {
                    pos: pos[i],
                    nrm: nrm[i],
                });
            }
            // Rebase indices for this primitive
            let start_u = start as u32;
            if index_count == 0 {
                for i in 0..(pos.len() as u32) {
                    indices.push((start_u + i) as u16);
                }
            } else {
                let base = indices.len() - (index_count as usize);
                for i in base..indices.len() {
                    let v = indices[i] as u32 + start_u;
                    indices[i] =
                        u16::try_from(v).map_err(|_| anyhow!("rebased index {} exceeds u16", v))?;
                }
            }
        }
    }

    if vertices.is_empty() || indices.is_empty() {
        bail!("JSON fallback: no geometry decoded in {}", path.display());
    }
    Ok(CpuMesh { vertices, indices })
}
