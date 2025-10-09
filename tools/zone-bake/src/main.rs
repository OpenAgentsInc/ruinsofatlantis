//! zone-bake: Bake a Zone snapshot (terrain + tree transforms) to data/zones/<slug>/snapshot.v1
//!
//! Usage:
//!   cargo run -p zone-bake -- <slug>
//! Example:
//!   cargo run -p zone-bake -- wizard_woods

use anyhow::{Context, Result};
use std::fs;

fn main() -> Result<()> {
    env_logger::init();
    let slug = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "wizard_woods".to_string());
    // Read manifest + scene from workspace data dir, then call library API
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let data_root = {
        let ws = here.join("../../data");
        if ws.is_dir() { ws } else { here.join("data") }
    };
    let packs_root = here.join("../../packs");
    let manifest = fs::read_to_string(data_root.join("zones").join(&slug).join("manifest.json"))
        .with_context(|| format!("read manifest: {}", slug))?;
    let scene = fs::read_to_string(data_root.join("zones").join(&slug).join("scene.json"))
        .unwrap_or_else(|_| "{\"version\":\"1.0.0\",\"seed\":0,\"layers\":[],\"instances\":[],\"logic\":{\"triggers\":[],\"spawns\":[],\"waypoints\":[],\"links\":[]}}".to_string());

    let inp = zone_bake::api::BakeInputs {
        manifest_json: manifest,
        scene_json: scene,
        assets_root: here.join("../../assets"),
        out_dir: packs_root.join("zones"),
        slug: slug.clone(),
    };
    zone_bake::api::bake_snapshot(&inp)?;
    log::info!("Wrote snapshot.v1 for '{}'", slug);
    Ok(())
}
