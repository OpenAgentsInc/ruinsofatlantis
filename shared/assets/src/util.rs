use anyhow::Result;
use std::path::{Path, PathBuf};

/// Prepare a glTF path for loading per policy: prefer `<name>.decompressed.gltf`
/// if present. Do not attempt runtime Draco decompression.
pub fn prepare_gltf_path(path: &Path) -> Result<PathBuf> {
    // Prefer original if it imports successfully. Fall back to a sibling
    // `<name>.decompressed.gltf` if present and importable. Do not attempt
    // runtime decompression.
    if gltf::import(path).is_ok() {
        return Ok(path.to_path_buf());
    }
    let decompressed = path.with_extension("decompressed.gltf");
    if decompressed.exists() && gltf::import(&decompressed).is_ok() {
        return Ok(decompressed);
    }
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for _ in 0..5 {
            if p.join("assets/models/wizard.gltf").exists() { return p; }
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
        assert!(gltf::import(&out).is_ok(), "resolved file must be importable: {}", out.display());
    }
}
