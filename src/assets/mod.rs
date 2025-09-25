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
use crate::gfx::Vertex;

/// CPU-side mesh ready to be uploaded to GPU.
pub struct CpuMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

/// Load a `.gltf` file from disk and merge all primitives into a single mesh.
pub fn load_gltf_mesh(path: &std::path::Path) -> Result<CpuMesh> {
    // Use the high-level importer which resolves external buffers/images.
    let (doc, buffers, _images) = gltf::import(path).with_context(|| format!(
        "failed to import glTF: {}",
        path.display()
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
