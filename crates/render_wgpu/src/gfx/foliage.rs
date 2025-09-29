//! Foliage (trees) setup: builds instance transforms, loads a GLTF tree mesh,
//! and uploads GPU buffers. Procedurally scatters trees if no baked snapshot
//! exists for the zone.
//!
//! Extending
//! - Swap the hardcoded GLTF path for a prototypes schema (multiple tree types).
//! - Add per‑instance variation (palette, scale, wind params) and LODs.
//! - Share a material path for textured instancing if desired.

use anyhow::Result;

use ra_assets::gltf::load_gltf_mesh;
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
    // Prefer baked instances if available, else scatter using vegetation params.
    let trees_models_opt = terrain::load_trees_snapshot(zone_slug);
    let mut trees_instances_cpu: Vec<super::types::Instance> =
        if let Some(models) = &trees_models_opt {
            terrain::instances_from_models(models)
        } else {
            let (tree_count, tree_seed) = vegetation.unwrap_or((350usize, 20250926u32));
            terrain::place_trees(terrain_cpu, tree_count, tree_seed)
        };
    // Mark trees with a non-highlight selection value to enable wind sway in the shader.
    for inst in &mut trees_instances_cpu {
        inst.selected = 0.25; // below 0.5 so it won't render as highlighted
    }
    let count = trees_instances_cpu.len() as u32;
    let instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("trees-instances"),
        contents: bytemuck::cast_slice(&trees_instances_cpu),
        usage: wgpu::BufferUsages::VERTEX,
    });

    // Load a static tree mesh (GLTF) and upload. We vendor a specific tree asset
    // under assets/models so referenced images/buffers resolve via relative paths.
    let tree_mesh_path = asset_path("assets/models/trees/CommonTree_3/CommonTree_3.gltf");
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
