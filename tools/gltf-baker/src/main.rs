use anyhow::{Context, Result, anyhow};
use clap::Parser;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "gltf-baker")]
#[command(about = "Minimal GLTF/GLB inspector & baker (MVP)")]
struct Cli {
    /// Input .gltf or .glb
    input: PathBuf,
    /// Output JSON summary (MVP)
    output: Option<PathBuf>,
}

#[derive(Serialize)]
struct Summary {
    file: String,
    scenes: usize,
    nodes: usize,
    meshes: usize,
    skins: usize,
    animations: usize,
    materials: usize,
    has_draco: bool,
}

fn scan_has_draco(doc: &gltf::Document) -> bool {
    doc.extensions_used()
        .any(|e| e == "KHR_draco_mesh_compression")
}

fn bake_summary(path: &std::path::Path) -> Result<Summary> {
    let (doc, _buffers, _images) =
        gltf::import(path).with_context(|| format!("import {}", path.display()))?;
    let sum = Summary {
        file: path.display().to_string(),
        scenes: doc.scenes().len(),
        nodes: doc.nodes().len(),
        meshes: doc.meshes().len(),
        skins: doc.skins().len(),
        animations: doc.animations().len(),
        materials: doc.materials().len(),
        has_draco: scan_has_draco(&doc),
    };
    if sum.has_draco {
        return Err(anyhow!(
            "KHR_draco_mesh_compression detected — please pre-decompress before baking"
        ));
    }
    Ok(sum)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if !cli.input.exists() {
        return Err(anyhow!("missing input: {}", cli.input.display()));
    }
    let sum = bake_summary(&cli.input)?;
    if let Some(out) = cli.output.as_ref() {
        let data = serde_json::to_vec_pretty(&sum)?;
        std::fs::write(out, data)?;
        println!("wrote {}", out.display());
    } else {
        println!("{}", serde_json::to_string_pretty(&sum)?);
    }
    Ok(())
}
