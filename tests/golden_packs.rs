use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

#[test]
fn golden_spellpack_matches_builder() {
    // Build pack bytes in-memory using the same format as xtask
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let spells_dir = root.join("data/spells");
    let mut entries: Vec<(String, serde_json::Value)> = Vec::new();
    for entry in fs::read_dir(spells_dir).expect("spells dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") { continue; }
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        // Validate it parses as a spec via data_runtime
        let rel = format!("spells/{}", path.file_name().unwrap().to_string_lossy());
        let _spec = data_runtime::loader::load_spell_spec(&rel).expect("spell spec");
        let txt = fs::read_to_string(&path).expect("read spell json");
        let val: serde_json::Value = serde_json::from_str(&txt).expect("parse json");
        entries.push((name, val));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut buf: Vec<u8> = Vec::new();
    buf.extend_from_slice(b"SPELLPK\0");
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (name, json) in &entries {
        let name_bytes = name.as_bytes();
        let json_bytes = serde_json::to_vec(json).expect("serde vec");
        assert!(name_bytes.len() <= u16::MAX as usize);
        buf.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(name_bytes);
        buf.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(&json_bytes);
    }
    let pack_path = root.join("packs/spellpack.v1.bin");
    if pack_path.exists() {
        let on_disk = fs::read(&pack_path).expect("read packs/spellpack.v1.bin");
        assert_eq!(buf, on_disk, "spellpack bytes differ from builder");
    } else {
        // Not built in this test run (expected when running `cargo test` directly). CI uses xtask to build packs first.
        eprintln!("golden skipped: {} not found", pack_path.display());
    }
}

#[test]
fn golden_zone_meta_sha256() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let meta = root.join("data/zones/wizard_woods/snapshot.v1/zone_meta.json");
    if !meta.exists() {
        eprintln!("golden skipped: {} not found", meta.display());
        return;
    }
    let bytes = fs::read(&meta).expect("read zone_meta.json");
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let got = format!("{:x}", hasher.finalize());
    let expected = "b3e10f4f21ac69674e40511b8af61a67bf864084749ef27e4ac328856da86350";
    assert_eq!(got, expected, "zone_meta.json sha256 mismatch");
}
