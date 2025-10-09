pub mod api {
    use anyhow::{Context, Result};
    use serde::Serialize;
    use std::fs;
    use std::path::PathBuf;
    use blake3::Hasher as Blake3;

    #[derive(Debug)]
    pub struct BakeInputs {
        pub manifest_json: String,
        pub scene_json: String,
        pub assets_root: PathBuf,
        pub out_dir: PathBuf, // packs/zones (parent of <slug>/snapshot.v1)
        pub slug: String,
    }

    #[derive(Serialize)]
    struct ZoneMeta<'a> {
        schema: &'a str,
        slug: &'a str,
        version: &'a str,
        bounds: Bounds,
        counts: Counts,
        hashes: Hashes,
    }

    #[derive(Serialize, Default)]
    struct Bounds { min: [f32;3], max: [f32;3] }
    #[derive(Serialize, Default)]
    struct Counts { instances: u32, clusters: u32, colliders: u32, logic_triggers: u32, logic_spawns: u32 }
    #[derive(Serialize, Default)]
    struct Hashes { instances: String, clusters: String, colliders: String, logic: String }

    pub fn bake_snapshot(inputs: &BakeInputs) -> Result<()> {
        #[derive(serde::Deserialize)]
        struct ManifestMin { slug: String, display_name: String, #[allow(dead_code)] terrain: serde_json::Value, #[serde(default)] version: Option<String> }
        let m: ManifestMin =
            serde_json::from_str(&inputs.manifest_json).context("parse manifest_json")?;

        let snap = inputs.out_dir.join(&inputs.slug).join("snapshot.v1");
        std::fs::create_dir_all(&snap).with_context(|| format!("mkdir {}", snap.display()))?;
        // Minimal binary snapshot files
        fs::write(snap.join("instances.bin"), 0u32.to_le_bytes())?;
        fs::write(snap.join("clusters.bin"), 0u32.to_le_bytes())?;
        fs::write(snap.join("colliders.bin"), &[] as &[u8])?;
        fs::write(snap.join("colliders_index.bin"), &[] as &[u8])?;
        fs::write(snap.join("logic.bin"), &[] as &[u8])?;

        let file_hash = |name: &str| -> Result<String> {
            let mut h = Blake3::new();
            let b = fs::read(snap.join(name)).with_context(|| format!("read {}", name))?;
            h.update(&b);
            Ok(format!("blake3:{}", h.finalize().to_hex()))
        };
        let meta = ZoneMeta {
            schema: "snapshot.v1",
            slug: &m.slug,
            version: m.version.as_deref().unwrap_or("1.0.0"),
            bounds: Bounds::default(),
            counts: Counts { instances: 0, clusters: 0, colliders: 0, logic_triggers: 0, logic_spawns: 0 },
            hashes: Hashes {
                instances: file_hash("instances.bin")?,
                clusters: file_hash("clusters.bin")?,
                colliders: file_hash("colliders.bin")?,
                logic: file_hash("logic.bin")?,
            },
        };
        let mj = serde_json::to_string_pretty(&meta)?;
        fs::write(snap.join("meta.json"), mj)?;
        Ok(())
    }
}
