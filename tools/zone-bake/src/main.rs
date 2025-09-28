//! zone-bake: Bake a Zone snapshot (terrain + tree transforms) to data/zones/<slug>/snapshot.v1
//!
//! Usage:
//!   cargo run -p zone-bake -- <slug>
//! Example:
//!   cargo run -p zone-bake -- wizard_woods

use anyhow::{Context, Result};
use data_runtime::zone::load_zone_manifest;
use render_wgpu::gfx::terrain;
use serde::Serialize;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

fn main() -> Result<()> {
    env_logger::init();
    let slug = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "wizard_woods".to_string());
    let z =
        load_zone_manifest(&slug).with_context(|| format!("load zone manifest for '{}'", slug))?;
    log::info!(
        "Baking zone '{}' (id={}, plane={:?})",
        z.display_name,
        z.zone_id,
        z.plane
    );
    // CPU-only generation
    let cpu = terrain::generate_cpu(z.terrain.size as usize, z.terrain.extent, z.terrain.seed);
    // Deterministic tree scatter from manifest (fallback defaults if omitted)
    let (tree_count, tree_seed) = z
        .vegetation
        .as_ref()
        .map(|v| (v.tree_count as usize, v.tree_seed))
        .unwrap_or((350usize, 20250926u32));
    let trees = terrain::place_trees(&cpu, tree_count, tree_seed)
        .into_iter()
        .map(|inst| inst.model)
        .collect::<Vec<[[f32; 4]; 4]>>();
    // Write snapshots
    terrain::write_terrain_snapshot(&slug, &cpu, z.terrain.seed)?;
    terrain::write_trees_snapshot(&slug, &trees)?;
    // Write simple colliders for trees: Y cylinders centered at tree position
    write_colliders_snapshot(&slug, &trees)?;
    // Write meta with simple fingerprints
    write_meta(&slug, &z, &cpu.heights, &trees)?;
    log::info!(
        "Wrote snapshot.v1 for '{}' (terrain {}x{}, trees={})",
        slug,
        cpu.size,
        cpu.size,
        trees.len()
    );
    Ok(())
}

#[derive(Serialize)]
struct MetaTerrain<'a> {
    size: usize,
    extent: f32,
    seed: u32,
    heights_count: usize,
    heights_fingerprint: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<&'a str>,
}

#[derive(Serialize)]
struct MetaTrees {
    count: usize,
    fingerprint: u64,
}

#[derive(Serialize)]
struct ZoneMeta<'a> {
    schema: &'a str,
    zone_id: u32,
    slug: &'a str,
    display_name: &'a str,
    plane: &'a str,
    terrain: MetaTerrain<'a>,
    trees: MetaTrees,
    content_fingerprint: u64,
}

fn write_meta(
    slug: &str,
    z: &data_runtime::zone::ZoneManifest,
    heights: &[f32],
    trees: &[[[f32; 4]; 4]],
) -> Result<()> {
    let hfp = fingerprint_heights(heights);
    let tfp = fingerprint_models(trees);
    let meta = ZoneMeta {
        schema: "snapshot.v1",
        zone_id: z.zone_id,
        slug: &z.slug,
        display_name: &z.display_name,
        plane: match &z.plane {
            data_runtime::zone::ZonePlane::Material => "Material",
            data_runtime::zone::ZonePlane::Feywild => "Feywild",
            data_runtime::zone::ZonePlane::Shadowfell => "Shadowfell",
            data_runtime::zone::ZonePlane::Other(s) => s.as_str(),
        },
        terrain: MetaTerrain {
            size: z.terrain.size as usize,
            extent: z.terrain.extent,
            seed: z.terrain.seed,
            heights_count: heights.len(),
            heights_fingerprint: hfp,
            note: None,
        },
        trees: MetaTrees {
            count: trees.len(),
            fingerprint: tfp,
        },
        content_fingerprint: hfp ^ (tfp.rotate_left(1)),
    };
    // Write under workspace root data/, not crate-local
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let data_root = {
        let ws = here.join("../../data");
        if ws.is_dir() { ws } else { here.join("data") }
    };
    let dir = data_root.join("zones").join(slug).join("snapshot.v1");
    fs::create_dir_all(&dir)?;
    let path = dir.join("zone_meta.json");
    let txt = serde_json::to_string_pretty(&meta)?;
    fs::write(path, txt)?;
    Ok(())
}

fn fingerprint_heights(h: &[f32]) -> u64 {
    let mut hasher = DefaultHasher::new();
    h.len().hash(&mut hasher);
    for &v in h {
        v.to_bits().hash(&mut hasher);
    }
    hasher.finish()
}

fn fingerprint_models(mats: &[[[f32; 4]; 4]]) -> u64 {
    let mut hasher = DefaultHasher::new();
    mats.len().hash(&mut hasher);
    for m in mats {
        for row in m {
            for &v in row {
                v.to_bits().hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

#[repr(C)]
#[derive(Copy, Clone)]
struct ColliderBin {
    // proto_id (1 for tree placeholder), shape kind 0=CylinderY
    proto_id: u16,
    shape: u16,
    // params: [cx, cy, cz, radius, half_height]
    cx: f32,
    cy: f32,
    cz: f32,
    radius: f32,
    half_height: f32,
    aabb_min: [f32; 3],
    aabb_max: [f32; 3],
    chunk_id: u32,
}

fn write_colliders_snapshot(slug: &str, trees: &[[[f32; 4]; 4]]) -> Result<()> {
    // Simple uniform grid chunking
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let data_root = {
        let ws = here.join("../../data");
        if ws.is_dir() { ws } else { here.join("data") }
    };
    let packs_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packs/zones")
        .join(slug)
        .join("snapshot.v1");
    fs::create_dir_all(&packs_root)?;
    let mut out: Vec<ColliderBin> = Vec::new();
    let extent = 150.0f32; // default used in manifest
    let chunk = 30.0f32;
    for m in trees {
        let c = m;
        let tx = c[3][0];
        let ty = c[3][1];
        let tz = c[3][2];
        let radius = 0.5f32;
        let half_h = 2.5f32;
        let cx = tx;
        let cy = ty + half_h;
        let cz = tz;
        let min = [cx - radius, cy - half_h, cz - radius];
        let max = [cx + radius, cy + half_h, cz + radius];
        let gx = ((cx + extent) / chunk).floor() as i32;
        let gz = ((cz + extent) / chunk).floor() as i32;
        let chunk_id = ((gx & 0xFFFF) as u32) << 16 | ((gz & 0xFFFF) as u32);
        out.push(ColliderBin {
            proto_id: 1,
            shape: 0,
            cx,
            cy,
            cz,
            radius,
            half_height: half_h,
            aabb_min: min,
            aabb_max: max,
            chunk_id,
        });
    }
    // Write colliders.bin (raw packed)
    let path = packs_root.join("colliders.bin");
    let mut bytes: Vec<u8> = Vec::with_capacity(out.len() * std::mem::size_of::<ColliderBin>());
    for b in &out {
        let p = unsafe {
            std::slice::from_raw_parts(
                (b as *const ColliderBin) as *const u8,
                std::mem::size_of::<ColliderBin>(),
            )
        };
        bytes.extend_from_slice(p);
    }
    fs::write(&path, &bytes)?;
    // Build a simple index: sorted by chunk_id with begin/end
    let mut ids: Vec<u32> = out.iter().map(|c| c.chunk_id).collect();
    ids.sort_unstable();
    ids.dedup();
    // Create mapping by scanning
    let mut index: Vec<(u32, u32, u32)> = Vec::new();
    for id in ids {
        let mut begin = u32::MAX;
        let mut end = 0u32;
        for (i, c) in out.iter().enumerate() {
            if c.chunk_id == id {
                if begin == u32::MAX {
                    begin = i as u32;
                }
                end = i as u32 + 1;
            }
        }
        if begin != u32::MAX {
            index.push((id, begin, end));
        }
    }
    let mut idx_bytes: Vec<u8> = Vec::with_capacity(index.len() * 12 + 4);
    idx_bytes.extend_from_slice(&(index.len() as u32).to_le_bytes());
    for (id, b, e) in index {
        idx_bytes.extend_from_slice(&id.to_le_bytes());
        idx_bytes.extend_from_slice(&b.to_le_bytes());
        idx_bytes.extend_from_slice(&e.to_le_bytes());
    }
    fs::write(packs_root.join("colliders_index.bin"), &idx_bytes)?;
    Ok(())
}
