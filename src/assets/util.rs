//! Asset utilities (paths, policy helpers).

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Prepare a glTF path for loading per policy: prefer `<name>.decompressed.gltf`
/// if present. Do not attempt runtime Draco decompression.
pub fn prepare_gltf_path(path: &Path) -> Result<PathBuf> {
    let decompressed = path.with_extension("decompressed.gltf");
    if decompressed.exists() {
        Ok(decompressed)
    } else {
        Ok(path.to_path_buf())
    }
}

