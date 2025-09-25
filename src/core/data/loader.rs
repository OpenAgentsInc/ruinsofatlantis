//! Data loaders. For now, stubs that resolve paths under `data/`.
//! Implement JSON parsing later to avoid adding new deps mid-prototype.

use std::path::{Path, PathBuf};
use std::fs;
use anyhow::{Context, Result};
use super::spell::SpellSpec;

fn data_root() -> PathBuf {
    // Assume running from project root during development
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// Read a raw JSON file under `data/` and return its string.
pub fn read_json(rel: impl AsRef<Path>) -> Result<String> {
    let path = data_root().join(rel);
    let s = fs::read_to_string(&path).with_context(|| format!("read data: {}", path.display()))?;
    Ok(s)
}

/// Load and deserialize a spell JSON (from data/spells/*).
pub fn load_spell_spec(rel: impl AsRef<Path>) -> Result<SpellSpec> {
    let txt = read_json(rel)?;
    let spec: SpellSpec = serde_json::from_str(&txt).context("parse spell json")?;
    Ok(spec)
}
