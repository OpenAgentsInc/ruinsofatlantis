//! FBX animation import (feature placeholder).
//!
//! This module provides the public hook for merging external FBX animations
//! into an existing `SkinnedMeshCPU`. The actual parser is planned behind a
//! Cargo feature (`fbx`) using `fbxcel`/`fbxcel-dom`. Until then, this stub
//! returns an informative error so callers can handle the absence gracefully.

use anyhow::{Result, bail};
use std::path::Path;

use crate::types::SkinnedMeshCPU;

/// Merge animation clips from an FBX file into an existing skinned mesh by node-name mapping.
///
/// Note: Real parsing lives behind the `fbx` feature. This default build just provides
/// a friendly error so tools can compile without FBX support.
pub fn merge_fbx_animations(_base: &mut SkinnedMeshCPU, _fbx_path: &Path) -> Result<usize> {
    bail!("FBX animation import requires building ra-assets with the `fbx` feature")
}
