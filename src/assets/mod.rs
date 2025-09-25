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
use std::{fs, path::{Path, PathBuf}, process::Command};
use crate::gfx::Vertex;

/// CPU-side mesh ready to be uploaded to GPU.
pub struct CpuMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

/// Load a `.gltf` file from disk and merge all primitives into a single mesh.
pub fn load_gltf_mesh(path: &Path) -> Result<CpuMesh> {
    // Try import first; if we hit Draco, fall back to best-effort decompression.
    let mut source_path: PathBuf = path.to_path_buf();
    let import_result = gltf::import(&source_path);
    let (doc, buffers, _images) = match import_result {
        Ok(ok) => ok,
        Err(e) => {
            let err_str = format!("{e}");
            // If the error directly mentions Draco or the JSON header says Draco is required, attempt to decompress.
            let draco_flag = err_str.contains("KHR_draco_mesh_compression")
                || is_draco_compressed(&source_path).unwrap_or(false)
                || file_contains(&source_path, "KHR_draco_mesh_compression").unwrap_or(false);
            if draco_flag {
                if let Some(out) = try_decompress_with_gltf_transform(&source_path) {
                    log::warn!(
                        "Detected KHR_draco_mesh_compression; using decompressed copy: {}",
                        out.display()
                    );
                    source_path = out;
                    gltf::import(&source_path).with_context(|| format!(
                        "failed to import glTF after decompress: {}",
                        source_path.display()
                    ))?
                } else {
                    bail!(
                        "Model uses KHR_draco_mesh_compression and could not be auto-decompressed. \
                         Please install Node and run: \n  npx -y @gltf-transform/cli decompress {0} {0}.decompressed.gltf\nThen re-run.",
                        path.display()
                    );
                }
            } else {
                return Err(anyhow!("failed to import glTF: {}", source_path.display()).context(e));
            }
        }
    };

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

/// Returns true if the `.gltf` JSON declares `KHR_draco_mesh_compression` as required.
fn is_draco_compressed(path: &Path) -> Result<bool> {
    if path.extension().and_then(|e| e.to_str()).unwrap_or("") != "gltf" {
        return Ok(false);
    }
    let bytes = fs::read(path).with_context(|| format!("read glTF json: {}", path.display()))?;
    let gltf = gltf::Gltf::from_slice(&bytes).context("parse glTF JSON header")?;
    Ok(gltf
        .extensions_required()
        .any(|s| s == "KHR_draco_mesh_compression"))
}

/// Best-effort Draco decompression using `gltf-transform` CLI via `npx`.
/// Returns the output path if successful.
fn try_decompress_with_gltf_transform(input: &Path) -> Option<PathBuf> {
    let npx = which::which("npx").ok()?;
    let out = input.with_extension("decompressed.gltf");
    let status = Command::new(npx)
        .arg("-y")
        .arg("@gltf-transform/cli")
        .arg("decompress")
        .arg(input.as_os_str())
        .arg(out.as_os_str())
        .status()
        .ok()?;
    if status.success() && out.exists() {
        Some(out)
    } else {
        None
    }
}

fn file_contains(path: &Path, needle: &str) -> Result<bool> {
    // Only for small JSON .gltf files; skips .glb.
    if path.extension().and_then(|e| e.to_str()) != Some("gltf") {
        return Ok(false);
    }
    let data = fs::read(path).with_context(|| format!("read file: {}", path.display()))?;
    let nb = needle.as_bytes();
    Ok(data.windows(nb.len()).any(|w| w == nb))
}
