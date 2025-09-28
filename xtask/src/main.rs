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
    cargo(&["test"])?;
    schema_check()?;
    Ok(())
}

fn schema_check() -> Result<()> {
    // Minimal schema check: ensure all zone manifests load and some spells parse.
    // This piggybacks on serde models in data_runtime.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let zones = root.join("data/zones");
    if zones.is_dir() {
        for entry in std::fs::read_dir(&zones)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let slug = entry.file_name().to_string_lossy().to_string();
            data_runtime::zone::load_zone_manifest(&slug)
                .with_context(|| format!("validate zone manifest: {}", slug))?;
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
    // Stub: pack pipeline not implemented yet; this is a placeholder to reserve the CLI.
    println!("xtask: build-spells (stub) — no-op");
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
        Cmd::SchemaCheck => schema_check(),
        Cmd::BuildPacks => build_packs(),
        Cmd::BuildSpells => build_spells(),
        Cmd::BakeZone { slug } => bake_zone(&slug),
    }
}
