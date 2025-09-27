//! zone_bake: Bake a Zone snapshot (terrain + tree transforms) to data/zones/<slug>/snapshot.v1
//!
//! Usage:
//!   cargo run --bin zone_bake -- <slug>
//! Example:
//!   cargo run --bin zone_bake -- wizard_woods

use anyhow::{Context, Result};
use ruinsofatlantis::core::data::zone::load_zone_manifest;
use ruinsofatlantis::gfx::terrain;

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
    // Deterministic tree scatter (count & seed can be tuned later or read from manifest)
    let trees = terrain::place_trees(&cpu, 350, 20250926)
        .into_iter()
        .map(|inst| inst.model)
        .collect::<Vec<[[f32; 4]; 4]>>();
    // Write snapshots
    terrain::write_terrain_snapshot(&slug, &cpu, z.terrain.seed)?;
    terrain::write_trees_snapshot(&slug, &trees)?;
    log::info!(
        "Wrote snapshot.v1 for '{}' (terrain {}x{}, trees={})",
        slug,
        cpu.size,
        cpu.size,
        trees.len()
    );
    Ok(())
}
