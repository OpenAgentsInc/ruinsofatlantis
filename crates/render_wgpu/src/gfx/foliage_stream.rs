//! CPU-only foliage builder for background streaming.
//!
//! This module performs disk IO + GLTF parsing and prepares CPU buffers
//! for tree kinds. It must contain no `wgpu` usage so it can run off-thread
//! and avoid stalling the UI thread. The renderer uploads the resulting
//! batches incrementally on the main thread.

use anyhow::Result;

/// CPU payload for one tree-kind batch (no wgpu types here).
#[derive(Clone)]
pub struct TreeCpuBatch {
    pub kind: String,
    pub instances: Vec<crate::gfx::types::Instance>,
    pub verts_uv: Vec<crate::gfx::types::VertexPosNrmUv>,
    pub indices_u16: Vec<u16>,
    /// Optional base-color texture (rgba8, width, height)
    pub base_tex_rgba8: Option<(Vec<u8>, u32, u32)>,
}

/// Build foliage by kind on CPU (no GPU calls, safe to run on a worker).
pub fn build_foliage_cpu_by_kind(
    zone_slug: &str,
    terr: &crate::gfx::terrain::TerrainCPU,
) -> Result<Vec<TreeCpuBatch>> {
    // Load baked snapshot by kind; fall back to nothing
    let map = match crate::gfx::terrain::load_trees_snapshot_by_kind(zone_slug) {
        Some(m) => m,
        None => return Ok(Vec::new()),
    };

    let mut out = Vec::new();
    for (kind, models) in map {
        if should_skip_kind(&kind) || snapshot_is_collapsed(&models) {
            continue;
        }
        // Instances (snap Y to ground)
        let mut inst = crate::gfx::terrain::instances_from_models(&models);
        for m in &mut inst {
            let x = m.model[3][0];
            let z = m.model[3][2];
            let (y, _n) = crate::gfx::terrain::height_at(terr, x, z);
            m.model[3][1] = y;
            m.selected = 0.25;
        }

        // Mesh + optional base color texture — CPU only
        let mesh_path = crate::gfx::foliage::path_for_kind(&kind);
        let (verts_uv, indices_u16, base_tex_rgba8) = match gltf::import(&mesh_path) {
            Ok((doc, buffers, images)) => {
                use gltf::mesh::util::ReadIndices;
                let mut vtx: Vec<crate::gfx::types::VertexPosNrmUv> = Vec::new();
                let mut idx: Vec<u16> = Vec::new();
                let mut base_tex: Option<(Vec<u8>, u32, u32)> = None;

                for mesh in doc.meshes() {
                    for prim in mesh.primitives() {
                        let reader =
                            prim.reader(|b| buffers.get(b.index()).map(|bb| bb.0.as_slice()));
                        let pos = reader
                            .read_positions()
                            .map(|it| it.collect::<Vec<_>>())
                            .unwrap_or_default();
                        let nrm = reader
                            .read_normals()
                            .map(|it| it.collect::<Vec<_>>())
                            .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; pos.len()]);
                        let uv = reader
                            .read_tex_coords(0)
                            .map(|c| c.into_f32().collect::<Vec<_>>())
                            .unwrap_or_else(|| vec![[0.0, 0.0]; pos.len()]);
                        let base = vtx.len() as u32;
                        for i in 0..pos.len() {
                            vtx.push(crate::gfx::types::VertexPosNrmUv {
                                pos: pos[i],
                                nrm: nrm[i],
                                uv: uv[i],
                            });
                        }
                        let idx_u32: Vec<u32> = match reader.read_indices() {
                            Some(ReadIndices::U16(it)) => it.map(|v| v as u32).collect(),
                            Some(ReadIndices::U32(it)) => it.collect(),
                            Some(ReadIndices::U8(it)) => it.map(|v| v as u32).collect(),
                            None => (0..pos.len() as u32).collect(),
                        };
                        for v in idx_u32 {
                            let rb = v + base;
                            if rb <= u16::MAX as u32 {
                                idx.push(rb as u16);
                            }
                        }
                        // Try to capture a reasonable base color texture
                        if base_tex.is_none() {
                            if let Some(texinfo) = prim
                                .material()
                                .pbr_metallic_roughness()
                                .base_color_texture()
                            {
                                let img_idx = texinfo.texture().source().index();
                                if let Some(img) = images.get(img_idx) {
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
                                    base_tex = Some((pixels, w, h));
                                }
                            }
                        }
                    }
                }
                (vtx, idx, base_tex)
            }
            Err(_) => (Vec::new(), Vec::new(), None),
        };

        out.push(TreeCpuBatch {
            kind,
            instances: inst,
            verts_uv,
            indices_u16,
            base_tex_rgba8,
        });
    }
    Ok(out)
}

// Local copies of small helpers to avoid depending on private items in foliage.rs
fn should_skip_kind(kind: &str) -> bool {
    // Keys are lowercased in snapshot. Historically we skipped the pine family due to
    // missing assets. If the resolved mesh path exists locally, do NOT skip.
    let k = kind.to_ascii_lowercase();
    if k == "pine" || k.starts_with("quaternius.pine_") {
        let p = crate::gfx::foliage::path_for_kind(kind);
        return !p.exists();
    }
    false
}

/// Heuristic: detect a broken/degenerate bake where all instance transforms
/// share (nearly) the same translation, causing trees to stack into one.
fn snapshot_is_collapsed(models: &[[[f32; 4]; 4]]) -> bool {
    if models.is_empty() {
        return true;
    }
    if models.len() == 1 {
        return false;
    }
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for m in models {
        let x = m[3][0];
        let y = m[3][1];
        let z = m[3][2];
        if x < min[0] {
            min[0] = x;
        }
        if x > max[0] {
            max[0] = x;
        }
        if y < min[1] {
            min[1] = y;
        }
        if y > max[1] {
            max[1] = y;
        }
        if z < min[2] {
            min[2] = z;
        }
        if z > max[2] {
            max[2] = z;
        }
    }
    let dx = (max[0] - min[0]).abs();
    let dz = (max[2] - min[2]).abs();
    dx < 0.5 && dz < 0.5
}
