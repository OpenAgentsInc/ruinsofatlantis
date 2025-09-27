//! zone_bake: Bake a Zone snapshot (terrain + tree transforms) to data/zones/<slug>/snapshot.v1
//!
//! Usage:
//!   cargo run --bin zone_bake -- <slug>
//! Example:
//!   cargo run --bin zone_bake -- wizard_woods

use anyhow::{Context, Result};
use ruinsofatlantis::core::data::zone::load_zone_manifest;
use ruinsofatlantis::gfx::terrain;
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
    z: &ruinsofatlantis::core::data::zone::ZoneManifest,
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
            ruinsofatlantis::core::data::zone::ZonePlane::Material => "Material",
            ruinsofatlantis::core::data::zone::ZonePlane::Feywild => "Feywild",
            ruinsofatlantis::core::data::zone::ZonePlane::Shadowfell => "Shadowfell",
            ruinsofatlantis::core::data::zone::ZonePlane::Other(s) => s.as_str(),
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
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("zones")
        .join(slug)
        .join("snapshot.v1");
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
