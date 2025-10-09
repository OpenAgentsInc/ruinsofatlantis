use std::{fs, path::PathBuf};
use tempfile::TempDir;

fn fx(p: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/zones")
        .join(p)
}

#[test]
fn bake_minimal_cc_demo_produces_snapshot() {
    let tmp = TempDir::new().unwrap();
    let out = tmp.path().join("packs").join("zones");
    fs::create_dir_all(&out).unwrap();

    let manifest = fs::read_to_string(fx("cc_demo/manifest.json")).unwrap();
    let scene = fs::read_to_string(fx("cc_demo/scene.min.json")).unwrap();

    let inp = zone_bake::api::BakeInputs {
        manifest_json: manifest,
        scene_json: scene,
        assets_root: PathBuf::from("assets"),
        out_dir: out.clone(),
        slug: "cc_demo".into(),
    };

    zone_bake::api::bake_snapshot(&inp).expect("bake should succeed");

    let snap_dir = out.join("cc_demo").join("snapshot.v1");
    for f in [
        "instances.bin",
        "clusters.bin",
        "colliders.bin",
        "colliders_index.bin",
        "logic.bin",
        "meta.json",
    ] {
        assert!(snap_dir.join(f).exists(), "missing {}", f);
    }

    let meta_txt = fs::read_to_string(snap_dir.join("meta.json")).unwrap();
    assert!(meta_txt.contains("\"schema\""));
    assert!(meta_txt.contains("\"counts\""));
}

#[test]
#[ignore]
fn bake_is_deterministic_for_same_seed_and_inputs() {
    use blake3::Hasher;
    let td1 = TempDir::new().unwrap();
    let td2 = TempDir::new().unwrap();

    let manifest = fs::read_to_string(fx("forest_grove/manifest.json")).unwrap();
    let scene = fs::read_to_string(fx("forest_grove/scene.json")).unwrap();

    let run = |root: &std::path::Path| {
        let out = root.join("packs").join("zones");
        fs::create_dir_all(&out).unwrap();
        let inputs = zone_bake::api::BakeInputs {
            manifest_json: manifest.clone(),
            scene_json: scene.clone(),
            assets_root: PathBuf::from("assets"),
            out_dir: out.clone(),
            slug: "forest_grove".into(),
        };
        zone_bake::api::bake_snapshot(&inputs).unwrap();
        let snap = out.join("forest_grove").join("snapshot.v1");
        let mut hasher = Hasher::new();
        for f in [
            "instances.bin",
            "clusters.bin",
            "colliders.bin",
            "colliders_index.bin",
            "logic.bin",
            "meta.json",
        ] {
            let b = fs::read(snap.join(f)).unwrap();
            hasher.update(&b);
        }
        hasher.finalize().to_hex().to_string()
    };

    let h1 = run(td1.path());
    let h2 = run(td2.path());
    assert_eq!(h1, h2, "bake must be deterministic for identical inputs");
}
