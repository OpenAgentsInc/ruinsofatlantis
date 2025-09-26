use std::path::{Path, PathBuf};

/// Prefer a pre-decompressed copy of a glTF if present, falling back to the
/// provided path.
pub fn prepare_gltf_path(path: &Path) -> PathBuf {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("gltf"))
        .unwrap_or(false)
    {
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            let mut alt = path.to_path_buf();
            alt.set_file_name(format!("{}.decompressed.gltf", stem));
            if alt.exists() {
                return alt;
            }
        }
    }
    path.to_path_buf()
}

