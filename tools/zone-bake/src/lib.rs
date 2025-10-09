pub mod api {
    use anyhow::{Context, Result};
    use render_wgpu::gfx::terrain;
    use serde::Serialize;
    use std::collections::hash_map::DefaultHasher;
    use std::fs;
    use std::hash::{Hash, Hasher};
    use std::path::PathBuf;

    #[derive(Debug)]
    pub struct BakeInputs {
        pub manifest_json: String,
        pub scene_json: String,
        pub assets_root: PathBuf,
        pub out_dir: PathBuf, // packs/zones (parent of <slug>/snapshot.v1)
        pub slug: String,
    }

    #[derive(Serialize)]
    struct MetaTerrain<'a> {
        size: usize,
        extent: f32,
        seed: u32,
        heights_count: usize,
        heights_fingerprint: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        note: Option<&'a str>,
    }

    #[derive(Serialize)]
    struct MetaTrees {
        count: usize,
        fingerprint: u64,
    }

    #[derive(Serialize)]
    struct ZoneMeta<'a> {
        schema: &'a str,
        slug: &'a str,
        display_name: &'a str,
        terrain: MetaTerrain<'a>,
        trees: MetaTrees,
    }

    pub fn bake_snapshot(inputs: &BakeInputs) -> Result<()> {
        #[derive(serde::Deserialize)]
        struct ManifestMin {
            slug: String,
            display_name: String,
            terrain: TerrainMin,
        }
        #[derive(serde::Deserialize)]
        struct TerrainMin {
            size: u32,
            extent: f32,
            seed: u32,
        }
        let m: ManifestMin =
            serde_json::from_str(&inputs.manifest_json).context("parse manifest_json")?;

        let cpu = terrain::generate_cpu(m.terrain.size as usize, m.terrain.extent, m.terrain.seed);
        let trees = terrain::place_trees(&cpu, 64, 20251007)
            .into_iter()
            .map(|inst| inst.model)
            .collect::<Vec<[[f32; 4]; 4]>>();

        let snap = inputs.out_dir.join(&inputs.slug).join("snapshot.v1");
        std::fs::create_dir_all(&snap).with_context(|| format!("mkdir {}", snap.display()))?;

        let tjson = serde_json::to_string_pretty(&serde_json::json!({
            "size": cpu.size,
            "extent": cpu.extent,
            "seed": m.terrain.seed,
            "heights": cpu.heights,
        }))?;
        fs::write(snap.join("terrain.json"), tjson)?;
        let th = serde_json::to_string_pretty(&serde_json::json!({ "models": trees }))?;
        fs::write(snap.join("trees.json"), th)?;
        fs::write(snap.join("colliders.bin"), &[0u8; 8])?;
        fs::write(snap.join("colliders_index.bin"), &[0u8; 4])?;

        let meta = ZoneMeta {
            schema: "snapshot.v1",
            slug: &m.slug,
            display_name: &m.display_name,
            terrain: MetaTerrain {
                size: cpu.size,
                extent: cpu.extent,
                seed: m.terrain.seed,
                heights_count: cpu.heights.len(),
                heights_fingerprint: fp_heights(&cpu.heights),
                note: None,
            },
            trees: MetaTrees {
                count: trees.len(),
                fingerprint: fp_models(&trees),
            },
        };
        let mj = serde_json::to_string_pretty(&meta)?;
        fs::write(snap.join("meta.json"), mj)?;
        Ok(())
    }

    fn fp_heights(h: &[f32]) -> u64 {
        let mut hasher = DefaultHasher::new();
        h.len().hash(&mut hasher);
        for &v in h {
            v.to_bits().hash(&mut hasher);
        }
        hasher.finish()
    }
    fn fp_models(mats: &[[[f32; 4]; 4]]) -> u64 {
        let mut hasher = DefaultHasher::new();
        mats.len().hash(&mut hasher);
        for m in mats {
            for row in m {
                for &v in row {
                    v.to_bits().hash(&mut hasher);
                }
            }
        }
        hasher.finish()
    }
}
