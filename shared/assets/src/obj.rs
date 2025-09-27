//! OBJ static mesh loader (positions + normals), no materials.
//!
//! Returns CpuMesh suitable for instanced drawing with the existing Vertex layout.

use anyhow::{Context, Result, bail};
use std::path::Path;

use crate::types::{CpuMesh, Vertex};

pub fn load_obj_mesh(path: &Path) -> Result<CpuMesh> {
    let input =
        std::fs::read_to_string(path).with_context(|| format!("read OBJ: {}", path.display()))?;
    let load_opts = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    };
    let (models, _materials) = tobj::load_obj_buf(&mut input.as_bytes(), &load_opts, |_| {
        Ok((Vec::new(), Default::default()))
    })
    .with_context(|| format!("parse OBJ: {}", path.display()))?;
    if models.is_empty() {
        bail!("no meshes in OBJ: {}", path.display());
    }
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();
    for m in models {
        let mesh = m.mesh;
        // Positions
        let pos = mesh.positions;
        let nrm = if !mesh.normals.is_empty() {
            mesh.normals
        } else {
            // Fallback: all up
            vec![0.0; pos.len()]
        };
        let vcount = pos.len() / 3;
        let start = vertices.len() as u32;
        for i in 0..vcount {
            let p = [pos[3 * i], pos[3 * i + 1], pos[3 * i + 2]];
            let nn = if nrm.len() >= 3 * (i + 1) {
                [nrm[3 * i], nrm[3 * i + 1], nrm[3 * i + 2]]
            } else {
                [0.0, 1.0, 0.0]
            };
            vertices.push(Vertex { pos: p, nrm: nn });
        }
        // Indices
        if mesh.indices.is_empty() {
            for i in 0..vcount as u32 {
                indices.push((start + i) as u16);
            }
        } else {
            for &idx in &mesh.indices {
                let vv = start + idx;
                indices.push(u16::try_from(vv).context("OBJ index exceeds u16")?);
            }
        }
    }
    Ok(CpuMesh { vertices, indices })
}
