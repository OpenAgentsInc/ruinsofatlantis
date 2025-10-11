pub mod api {
    use anyhow::{Context, Result};
    use blake3::Hasher as Blake3;
    use serde::Serialize;
    use std::fs;
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
    struct ZoneMeta<'a> {
        schema: &'a str,
        slug: &'a str,
        version: &'a str,
        bounds: Bounds,
        counts: Counts,
        hashes: Hashes,
    }

    #[derive(Serialize, Default)]
    struct Bounds {
        min: [f32; 3],
        max: [f32; 3],
    }
    #[derive(Serialize, Default)]
    struct Counts {
        instances: u32,
        clusters: u32,
        colliders: u32,
        logic_triggers: u32,
        logic_spawns: u32,
    }
    #[derive(Serialize, Default)]
    struct Hashes {
        instances: String,
        clusters: String,
        colliders: String,
        logic: String,
    }

    pub fn bake_snapshot(inputs: &BakeInputs) -> Result<()> {
        #[derive(serde::Deserialize)]
        struct ManifestMin {
            slug: String,
            display_name: String,
            #[allow(dead_code)]
            terrain: serde_json::Value,
            #[serde(default)]
            version: Option<String>,
        }
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

        // Parse scene.json spawns → optional baked trees snapshot (trees.json)
        #[derive(serde::Deserialize)]
        struct SceneDoc {
            #[allow(dead_code)]
            version: Option<String>,
            #[allow(dead_code)]
            seed: Option<i64>,
            #[allow(dead_code)]
            layers: Option<serde_json::Value>,
            #[allow(dead_code)]
            instances: Option<serde_json::Value>,
            logic: Option<SceneLogic>,
        }
        #[derive(serde::Deserialize)]
        struct SceneLogic {
            spawns: Option<Vec<SpawnMarker>>,
        }
        #[derive(serde::Deserialize)]
        struct SpawnMarker {
            kind: String,
            pos: [f32; 3],
            yaw_deg: f32,
        }
        use std::collections::HashMap;
        #[derive(serde::Serialize)]
        struct TreesSnapshotJson {
            models: Vec<[[f32; 4]; 4]>,
            #[serde(skip_serializing_if = "HashMap::is_empty")]
            by_kind: HashMap<String, Vec<[[f32; 4]; 4]>>,
        }
        let mut trees_models: Vec<[[f32; 4]; 4]> = Vec::new();
        let mut by_kind: HashMap<String, Vec<[[f32; 4]; 4]>> = HashMap::new();
        if !inputs.scene_json.is_empty() {
            if let Ok(doc) = serde_json::from_str::<SceneDoc>(&inputs.scene_json) {
                if let Some(logic) = doc.logic {
                    if let Some(spawns) = logic.spawns {
                        for s in spawns.into_iter() {
                            if s.kind.starts_with("tree.") {
                                let kind_slug = s
                                    .kind
                                    .strip_prefix("tree.")
                                    .unwrap_or("default")
                                    .to_lowercase();
                                let yaw = s.yaw_deg.to_radians();
                                let (c, snt) = (yaw.cos(), yaw.sin());
                                let tx = s.pos[0];
                                let ty = s.pos[1];
                                let tz = s.pos[2];
                                // Column-major 4x4 with translation in last column
                                let model = [
                                    [c, 0.0, snt, 0.0],
                                    [0.0, 1.0, 0.0, 0.0],
                                    [-snt, 0.0, c, 0.0],
                                    [tx, ty, tz, 1.0],
                                ];
                                trees_models.push(model);
                                by_kind.entry(kind_slug).or_default().push(model);
                            }
                        }
                    }
                }
            }
        }
        if !trees_models.is_empty() {
            let tj = TreesSnapshotJson {
                models: trees_models,
                by_kind,
            };
            let txt = serde_json::to_string_pretty(&tj)?;
            fs::write(snap.join("trees.json"), txt)?;
        }

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
            counts: Counts {
                instances: 0,
                clusters: 0,
                colliders: 0,
                logic_triggers: 0,
                logic_spawns: 0,
            },
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

#[cfg(test)]
mod tests {
    use super::api::*;
    use std::fs;

    #[test]
    fn bake_writes_trees_from_scene_spawns() {
        let tmp = tempfile::TempDir::new().unwrap();
        let packs = tmp.path().join("packs");
        fs::create_dir_all(&packs).unwrap();
        let manifest =
            r#"{ "slug":"campaign_builder", "display_name":"Campaign Builder", "terrain": {} }"#;
        let scene = r#"{
            "version":"1.0.0",
            "seed":0,
            "layers":[],
            "instances":[],
            "logic":{
                "triggers":[],
                "spawns":[
                    {"id":"m0001","kind":"tree.default","pos":[1.0,0.0,-2.0],"yaw_deg":270.0}
                ],
                "waypoints":[],
                "links":[]
            }
        }"#;
        let inp = BakeInputs {
            manifest_json: manifest.into(),
            scene_json: scene.into(),
            assets_root: tmp.path().to_path_buf(),
            out_dir: packs.join("zones"),
            slug: "campaign_builder".into(),
        };
        bake_snapshot(&inp).expect("bake");
        let trees = packs
            .join("zones/campaign_builder/snapshot.v1/trees.json")
            .to_string_lossy()
            .to_string();
        let txt = fs::read_to_string(&trees).expect("read trees.json");
        assert!(txt.contains("\"models\""), "trees.json missing models");
        assert!(txt.contains("-2.0"), "translation Z not present");
    }

    #[test]
    fn non_tree_kinds_are_ignored() {
        let tmp = tempfile::TempDir::new().unwrap();
        let packs = tmp.path().join("packs");
        fs::create_dir_all(&packs).unwrap();
        let manifest =
            r#"{ "slug":"campaign_builder", "display_name":"Campaign Builder", "terrain": {} }"#;
        let scene = r#"{
            "version":"1.0.0",
            "seed":0,
            "layers":[],
            "instances":[],
            "logic":{
                "triggers":[],
                "spawns":[
                    {"id":"m0001","kind":"npc.wizard","pos":[0.0,0.0,0.0],"yaw_deg":0.0}
                ],
                "waypoints":[],
                "links":[]
            }
        }"#;
        let inp = BakeInputs {
            manifest_json: manifest.into(),
            scene_json: scene.into(),
            assets_root: tmp.path().to_path_buf(),
            out_dir: packs.join("zones"),
            slug: "campaign_builder".into(),
        };
        bake_snapshot(&inp).expect("bake");
        let trees_path = packs.join("zones/campaign_builder/snapshot.v1/trees.json");
        assert!(
            !trees_path.exists(),
            "should not emit trees.json when no tree.* spawns exist"
        );
    }
}
