//! Foliage (trees) setup: builds instance transforms, loads a GLTF tree mesh,
//! and uploads GPU buffers. Procedurally scatters trees if no baked snapshot
//! exists for the zone.
//!
//! Extending
//! - Swap the hardcoded GLTF path for a prototypes schema (multiple tree types).
//! - Add per‑instance variation (palette, scale, wind params) and LODs.
//! - Share a material path for textured instancing if desired.

use anyhow::Result;

use roa_assets::gltf::load_gltf_mesh;
use wgpu::util::DeviceExt;

use super::terrain;

/// GPU resources for instanced trees.
pub struct TreesGpu {
    pub instances: wgpu::Buffer,
    pub count: u32,
    pub vb: wgpu::Buffer,
    pub ib: wgpu::Buffer,
    pub index_count: u32,
}

/// Build trees for a zone, using a baked snapshot when available, otherwise
/// scattering procedurally using vegetation params.
pub fn build_trees(
    device: &wgpu::Device,
    terrain_cpu: &terrain::TerrainCPU,
    zone_slug: &str,
    vegetation: Option<(usize, u32)>,
) -> Result<TreesGpu> {
    // Prefer baked instances if available and sane, else scatter using vegetation params.
    let mut trees_models_opt = if std::env::var("RA_TREES_PROCEDURAL")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        log::info!("RA_TREES_PROCEDURAL=1 => ignoring baked trees snapshot");
        None
    } else {
        terrain::load_trees_snapshot(zone_slug)
    };
    if let Some(models) = &trees_models_opt
        && snapshot_is_collapsed(models)
    {
        log::warn!(
            "baked trees snapshot for '{}' appears collapsed ({} models at ~one spot); using procedural scatter",
            zone_slug,
            models.len()
        );
        trees_models_opt = None;
    }
    // If no baked snapshot is present and vegetation explicitly sets tree_count=0,
    // disable trees. Otherwise, use baked snapshot (if any), falling back to procedural.
    if trees_models_opt.is_none()
        && let Some((count, _)) = vegetation
        && count == 0
    {
        log::debug!("trees disabled by manifest (count=0) and no baked snapshot present");
        let instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("trees-instances"),
            contents: &[],
            usage: wgpu::BufferUsages::VERTEX,
        });
        // Create a small dummy mesh (cube) that will never be drawn since count=0.
        let (vb, ib, index_count) = super::mesh::create_cube(device);
        return Ok(TreesGpu {
            instances,
            count: 0,
            vb,
            ib,
            index_count,
        });
    }
    let mut trees_instances_cpu: Vec<super::types::Instance> =
        if let Some(models) = &trees_models_opt {
            // Use baked transforms but snap Y to terrain so instances sit on the ground.
            let mut inst = terrain::instances_from_models(models);
            for m in &mut inst {
                let x = m.model[3][0];
                let z = m.model[3][2];
                let (y, _n) = terrain::height_at(terrain_cpu, x, z);
                m.model[3][1] = y;
            }
            inst
        } else {
            let (tree_count, tree_seed) = vegetation.unwrap_or((350usize, 20250926u32));
            terrain::place_trees(terrain_cpu, tree_count, tree_seed)
        };
    // Mark trees with a non-highlight selection value to enable wind sway in the shader.
    for inst in &mut trees_instances_cpu {
        inst.selected = 0.25; // below 0.5 so it won't render as highlighted
    }
    let count = trees_instances_cpu.len() as u32;
    log::info!("trees: building {} instances (zone='{}')", count, zone_slug);
    let instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("trees-instances"),
        contents: bytemuck::cast_slice(&trees_instances_cpu),
        usage: wgpu::BufferUsages::VERTEX,
    });

    // Load a static tree mesh and upload. Resolution order:
    // 1) RA_TREE_PATH (absolute or workspace-relative) if set
    // 2) assets/trees/Birch_4GLB.glb (embedded textures, convenient for tests)
    // 3) assets/models/trees/CommonTree_3/CommonTree_3.gltf (repo default)
    let tree_mesh_path = if let Ok(p) = std::env::var("RA_TREE_PATH") {
        let pb = std::path::PathBuf::from(p);
        if pb.exists() {
            pb
        } else {
            asset_path("assets/models/trees/CommonTree_3/CommonTree_3.gltf")
        }
    } else {
        let birch = asset_path("assets/trees/Birch_4GLB.glb");
        if birch.exists() {
            birch
        } else {
            asset_path("assets/models/trees/CommonTree_3/CommonTree_3.gltf")
        }
    };
    // Default to cube fallback if GLTF fails for any reason.
    let (vb, ib, index_count) = match load_gltf_mesh(&tree_mesh_path) {
        Ok(cpu) => {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("trees-vb"),
                contents: bytemuck::cast_slice(&cpu.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("trees-ib"),
                contents: bytemuck::cast_slice(&cpu.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            (vb, ib, cpu.indices.len() as u32)
        }
        Err(e) => {
            log::warn!(
                "failed to load GLTF tree mesh ({}): {}; falling back to cube",
                tree_mesh_path.display(),
                e
            );
            super::mesh::create_cube(device)
        }
    };

    // If meta exists, verify fingerprints
    if let Some(models) = &trees_models_opt
        && let Some(ok) =
            terrain::verify_snapshot_fingerprints(zone_slug, terrain_cpu, Some(models))
    {
        log::info!(
            "zone snapshot meta verification: {}",
            if ok { "ok" } else { "MISMATCH" }
        );
    }

    Ok(TreesGpu {
        instances,
        count,
        vb,
        ib,
        index_count,
    })
}

fn asset_path(rel: &str) -> std::path::PathBuf {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../").join(rel);
    if ws.exists() { ws } else { here.join(rel) }
}

/// Heuristic: detect a broken/degenerate bake where all instance transforms
/// share (nearly) the same translation, causing trees to stack into one.
fn snapshot_is_collapsed(models: &[[[f32; 4]; 4]]) -> bool {
    // One instance is valid; consider collapsed only when there are zero instances
    // or when multiple instances occupy nearly the same spot.
    if models.is_empty() {
        return true;
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
    // Very small spread implies a collapsed pile. Be generous (0.5m) to catch near-identical bakes.
    dx < 0.5 && dz < 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_collapsed_snapshot() {
        let m = [[
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [10.0, 2.0, 10.0, 1.0],
        ]];
        // Repeated at exactly same spot (many)
        let models: Vec<[[f32; 4]; 4]> = vec![m[0]; 50];
        assert!(snapshot_is_collapsed(&models));
    }

    #[test]
    fn non_collapsed_snapshot() {
        let base = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        // Single instance should be considered valid (not collapsed)
        let models_one: Vec<[[f32; 4]; 4]> = vec![base];
        assert!(!snapshot_is_collapsed(&models_one));
        // Multiple spread-out instances are not collapsed
        let mut models_many: Vec<[[f32; 4]; 4]> = Vec::new();
        for i in 0..20 {
            let mut m = base;
            m[3][0] = i as f32 * 2.0; // spread out in X
            models_many.push(m);
        }
        assert!(!snapshot_is_collapsed(&models_many));
    }
}
