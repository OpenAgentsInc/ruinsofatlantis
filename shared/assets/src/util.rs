use anyhow::Result;
use std::path::{Path, PathBuf};

/// Prepare a glTF path for loading per policy: prefer `<name>.decompressed.gltf`
/// if present. Do not attempt runtime Draco decompression.
pub fn prepare_gltf_path(path: &Path) -> Result<PathBuf> {
    // Prefer a sibling `<name>.decompressed.glb` or `.decompressed.gltf` when present and importable.
    // This avoids Draco at runtime for both skinned and unskinned loads.
    let dec_glb = path.with_extension("decompressed.glb");
    // If a decompressed GLB exists, prefer it without an import probe (some tools
    // produce GLBs that `gltf::import` rejects under probe, yet load fine in full pass).
    if dec_glb.exists() {
        log::info!(
            target: "roa_assets::util",
            "prepare_gltf_path: using decompressed {}",
            dec_glb.display()
        );
        return Ok(dec_glb);
    }
    let decompressed = path.with_extension("decompressed.gltf");
    if decompressed.exists() && gltf::import(&decompressed).is_ok() {
        log::info!(
            target: "roa_assets::util",
            "prepare_gltf_path: using decompressed {}",
            decompressed.display()
        );
        return Ok(decompressed);
    }
    if gltf::import(path).is_ok() {
        log::info!(
            target: "roa_assets::util",
            "prepare_gltf_path: using original {}",
            path.display()
        );
        return Ok(path.to_path_buf());
    }
    log::warn!(
        target: "roa_assets::util",
        "prepare_gltf_path: import failed for {}, returning original",
        path.display()
    );
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for _ in 0..5 {
            if p.join("assets/models/wizard.gltf").exists() {
                return p;
            }
            p.pop();
        }
        panic!("could not locate repo root containing assets/models");
    }

    #[test]
    fn returns_importable_path() {
        let root = repo_root();
        let orig = root.join("assets/models/wizard.gltf");
        let out = prepare_gltf_path(&orig).expect("prepare path");
        assert!(out.exists(), "resolved file must exist: {}", out.display());
        assert!(
            gltf::import(&out).is_ok(),
            "resolved file must be importable: {}",
            out.display()
        );
    }
}
