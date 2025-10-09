//! Zone snapshot types and loader.
//!
//! A baked Zone snapshot is emitted by the zone-bake tool under:
//!   packs/zones/<slug>/snapshot.v1/
//! with a small set of binary blobs (instances, clusters, colliders) and
//! optional `meta.json` describing bounds and authoring metadata.
//!
//! This module provides a tolerant loader: it reads any files that exist and
//! leaves missing sections as `None`, so the runtime can evolve without
//! breaking older snapshots.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ZoneMeta {
    pub zone_id: Option<u32>,
    pub slug: Option<String>,
    pub display_name: Option<String>,
    pub world_bounds_min: Option<[f32; 3]>,
    pub world_bounds_max: Option<[f32; 3]>,
}

#[derive(Clone, Debug, Default)]
pub struct ZoneSnapshot {
    pub slug: String,
    pub root: PathBuf,
    pub meta: Option<ZoneMeta>,
    pub instances_bin: Option<Vec<u8>>,        // instanced static meshes (CPU layout TBD)
    pub clusters_bin: Option<Vec<u8>>,         // cluster grid for culling
    pub colliders_bin: Option<Vec<u8>>,        // baked collider set
    pub colliders_index_bin: Option<Vec<u8>>,  // optional index for colliders
    pub logic_bin: Option<Vec<u8>>,            // baked logic
}

impl ZoneSnapshot {
    pub fn load(root: impl AsRef<Path>, slug: &str) -> Result<Self> {
        let snap = Path::new(&root.as_ref()).join(slug).join("snapshot.v1");
        let mut out = ZoneSnapshot {
            slug: slug.to_string(),
            root: snap.clone(),
            ..Default::default()
        };
        // meta.json (optional)
        let meta_path = snap.join("meta.json");
        if meta_path.exists() {
            let txt = fs::read_to_string(&meta_path).context("read meta.json")?;
            let m: ZoneMeta = serde_json::from_str(&txt).context("parse meta.json")?;
            out.meta = Some(m);
        }
        // known blobs (optional)
        out.instances_bin = read_opt(&snap.join("instances.bin"));
        out.clusters_bin = read_opt(&snap.join("clusters.bin"));
        out.colliders_bin = read_opt(&snap.join("colliders.bin"));
        out.colliders_index_bin = read_opt(&snap.join("colliders_index.bin"));
        out.logic_bin = read_opt(&snap.join("logic.bin"));
        Ok(out)
    }
}

fn read_opt(path: &Path) -> Option<Vec<u8>> {
    if path.exists() {
        match fs::read(path) {
            Ok(b) => Some(b),
            Err(e) => {
                log::warn!("zones: failed to read {:?}: {}", path, e);
                None
            }
        }
    } else {
        None
    }
}

/// Scan `packs/zones/*/snapshot.v1` and build a simple registry.
#[derive(Default)]
pub struct ZoneRegistry {
    pub root: PathBuf,
    pub slugs: Vec<String>,
}

impl ZoneRegistry {
    pub fn discover(root: impl AsRef<Path>) -> Result<Self> {
        let root = PathBuf::from(root.as_ref());
        let zones_root = root.join("zones");
        let mut slugs = Vec::new();
        if zones_root.exists() {
            for entry in fs::read_dir(&zones_root).context("read zones dir")? {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    continue;
                }
                let slug = entry.file_name().to_string_lossy().to_string();
                if zones_root.join(&slug).join("snapshot.v1").exists() {
                    slugs.push(slug);
                }
            }
            slugs.sort();
        }
        Ok(Self {
            root: zones_root,
            slugs,
        })
    }
    pub fn contains(&self, slug: &str) -> bool {
        self.slugs.iter().any(|s| s == slug)
    }
}
