use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Parser)]
#[command(author, version, about = "Workspace automation tasks", long_about = None)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// fmt + clippy -D warnings + tests (workspace)
    Ci,
    /// Validate all WGSL shaders across the workspace
    Wgsl,
    /// Validate data against serde models (zone manifests, spells)
    SchemaCheck,
    /// Build all packs (spells, zones)
    BuildPacks,
    /// Build spell pack only (stub)
    BuildSpells,
    /// Bake a zone snapshot to packs
    BakeZone { slug: String },
}

fn run(cmd: &mut Command) -> Result<()> {
    let status = cmd.status().context("spawn")?;
    if !status.success() {
        bail!("command failed: {:?}", cmd);
    }
    Ok(())
}

fn cargo(args: &[&str]) -> Result<()> {
    let mut c = Command::new("cargo");
    c.args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    run(&mut c)
}

fn ci() -> Result<()> {
    cargo(&["fmt", "--all"])?;
    cargo(&["clippy", "--all-targets", "--", "-D", "warnings"])?;
    wgsl_validate()?;
    cargo_deny()?;
    cargo(&["test"])?;
    schema_check()?;
    Ok(())
}

fn wgsl_validate() -> Result<()> {
    // Find all .wgsl files in the workspace and parse them with Naga.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut count = 0usize;
    for entry in walkdir::WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wgsl") {
            continue;
        }
        let txt = std::fs::read_to_string(path)
            .with_context(|| format!("read WGSL: {}", path.display()))?;
        match naga::front::wgsl::parse_str(&txt) {
            Ok(_module) => {
                count += 1;
            }
            Err(err) => {
                bail!("WGSL validation failed for {}: {}", path.display(), err);
            }
        }
    }
    println!("xtask: WGSL validated ({} files)", count);
    Ok(())
}

fn cargo_deny() -> Result<()> {
    // Attempt to run `cargo deny check` if installed; otherwise warn and continue.
    let mut cmd = Command::new("cargo");
    cmd.args(["deny", "check"]).stdout(Stdio::inherit()).stderr(Stdio::inherit());
    match cmd.status() {
        Ok(status) => {
            if !status.success() {
                bail!("cargo deny check failed");
            }
        }
        Err(e) => {
            eprintln!("xtask: cargo-deny not found or failed to launch: {} (skipping)", e);
        }
    }
    Ok(())
}

fn schema_check() -> Result<()> {
    // Minimal schema check: ensure all zone manifests load and some spells parse via serde.
    // In addition, validate zone manifests against a JSON Schema.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let zones = root.join("data/zones");
    if zones.is_dir() {
        // Load JSON Schema for ZoneManifest
        let schema_path = root.join("crates/data_runtime/schemas/zone_manifest.schema.json");
        let schema_txt = std::fs::read_to_string(&schema_path)
            .with_context(|| format!("read schema: {}", schema_path.display()))?;
        let schema_json: serde_json::Value =
            serde_json::from_str(&schema_txt).with_context(|| "parse schema json")?;
        // Extend lifetime for validator by leaking the parsed schema for process lifetime.
        let schema_static: &'static serde_json::Value = Box::leak(Box::new(schema_json));
        let compiled = jsonschema::JSONSchema::compile(schema_static)
            .with_context(|| "compile JSON Schema")?;
        for entry in std::fs::read_dir(&zones)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let slug = entry.file_name().to_string_lossy().to_string();
            // Serde validation
            data_runtime::zone::load_zone_manifest(&slug)
                .with_context(|| format!("validate zone manifest: {}", slug))?;
            // Schema validation
            let path = zones.join(&slug).join("manifest.json");
            let txt = std::fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?;
            let json: serde_json::Value = serde_json::from_str(&txt)
                .with_context(|| format!("parse json: {}", path.display()))?;
            if let Err(errors) = compiled.validate(&json) {
                let mut msg = String::new();
                for err in errors {
                    msg.push_str(&format!("schema error: {err}\n"));
                }
                bail!("{}", msg);
            }
        }
    }
    // Validate a few spells via loader
    let spells_dir = root.join("data/spells");
    if spells_dir.is_dir() {
        for entry in std::fs::read_dir(&spells_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let rel = format!("spells/{}", path.file_name().unwrap().to_string_lossy());
                let _ = data_runtime::loader::load_spell_spec(&rel)
                    .with_context(|| format!("validate spell: {}", rel))?;
            }
        }
    }
    Ok(())
}

fn build_spells() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let spells_dir = root.join("data/spells");
    let out_dir = root.join("packs");
    std::fs::create_dir_all(&out_dir)?;
    let out_path = out_dir.join("spellpack.v1.bin");
    let mut entries: Vec<(String, serde_json::Value)> = Vec::new();
    if spells_dir.is_dir() {
        for entry in std::fs::read_dir(&spells_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let name = path.file_stem().unwrap().to_string_lossy().to_string();
            let rel = format!("spells/{}", path.file_name().unwrap().to_string_lossy());
            // Ensure it parses into our Spec via serde
            let _ = data_runtime::loader::load_spell_spec(&rel)
                .with_context(|| format!("validate spell: {}", rel))?;
            let txt = std::fs::read_to_string(&path)?;
            let val: serde_json::Value = serde_json::from_str(&txt)?;
            entries.push((name, val));
        }
    }
    // Sort entries for determinism
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    // Serialize a compact binary pack: [magic(8)][version(u32)][count(u32)][repeated: name_len(u16) name bytes json_len(u32) json bytes]
    let mut buf: Vec<u8> = Vec::new();
    buf.extend_from_slice(b"SPELLPK\0");
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (name, json) in &entries {
        let name_bytes = name.as_bytes();
        let json_bytes = serde_json::to_vec(json)?; // already validated, compact
        if name_bytes.len() > u16::MAX as usize {
            bail!("spell name too long: {}", name);
        }
        buf.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(name_bytes);
        buf.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(&json_bytes);
    }
    std::fs::write(&out_path, &buf)?;
    println!(
        "xtask: wrote {} ({} spells)",
        out_path.display(),
        entries.len()
    );
    Ok(())
}

fn bake_zone(slug: &str) -> Result<()> {
    // Delegate to tools/zone-bake with the requested slug
    let mut c = Command::new("cargo");
    c.args(["run", "-p", "zone-bake", "--", slug])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    run(&mut c)
}

fn build_packs() -> Result<()> {
    build_spells()?;
    // Bake default demo zone if present
    if PathBuf::from("tools/zone-bake").exists() {
        let _ = bake_zone("wizard_woods");
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Ci => ci(),
        Cmd::Wgsl => wgsl_validate(),
        Cmd::SchemaCheck => schema_check(),
        Cmd::BuildPacks => build_packs(),
        Cmd::BuildSpells => build_spells(),
        Cmd::BakeZone { slug } => bake_zone(&slug),
    }
}
