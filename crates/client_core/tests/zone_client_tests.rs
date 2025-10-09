use std::{fs, path::PathBuf};

#[test]
fn load_zone_presentation_cc_demo() {
    // Create a tiny fake snapshot in a temp dir and point loader to it via env
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp
        .path()
        .join("packs")
        .join("zones")
        .join("cc_demo")
        .join("snapshot.v1");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("meta.json"), "{\n  \"slug\": \"cc_demo\"\n}").unwrap();
    unsafe {
        std::env::set_var(
            "ROA_PACKS_ROOT_FOR_TESTS",
            tmp.path().join("packs").to_string_lossy().to_string(),
        )
    };

    let zp = client_core::zone_client::ZonePresentation::load("cc_demo").expect("load");
    assert_eq!(zp.slug, "cc_demo");
}
