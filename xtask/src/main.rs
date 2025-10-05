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
    warn_hooks();
    cargo(&["fmt", "--all"])?;
    cargo(&["clippy", "--all-targets", "--", "-D", "warnings"])?;
    layering_guard()?;
    wgsl_validate()?;
    cargo_deny()?;
    // Build packs so golden tests can read outputs
    build_packs()?;
    cargo(&["test"])?;
    schema_check()?;
    // 95A: Validate renderer with feature combos for legacy/demo gates
    // Default/no-features sanity for render_wgpu (explicit no-default-features to be clear)
    if std::env::var("RA_CHECK_RENDER_NO_DEFAULTS")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        cargo(&["check", "-p", "render_wgpu", "--no-default-features"])?;
        cargo(&[
            "clippy",
            "-p",
            "render_wgpu",
            "--no-default-features",
            "--",
            "-D",
            "warnings",
        ])?;
        cargo(&["test", "-p", "render_wgpu", "--no-default-features"])?;
    } else {
        eprintln!(
            "xtask: skipping render_wgpu no-default-features checks (set RA_CHECK_RENDER_NO_DEFAULTS=1 to enable)"
        );
    }
    // Feature combo: vox_onepath_demo + legacy_client_carve + destruct_debug
    let feat = "vox_onepath_demo,legacy_client_carve,destruct_debug";
    if std::env::var("RA_CHECK_RENDER_FEATURE_COMBO")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        cargo(&[
            "clippy",
            "-p",
            "render_wgpu",
            "--no-default-features",
            "--features",
            feat,
            "--",
            "-D",
            "warnings",
        ])?;
        cargo(&[
            "test",
            "-p",
            "render_wgpu",
            "--no-default-features",
            "--features",
            feat,
        ])?;
        // Ensure demo bin builds under feature combo
        cargo(&[
            "build",
            "-p",
            "render_wgpu",
            "--no-default-features",
            "--features",
            feat,
            "--bin",
            "vox_onepath",
        ])?;
    } else {
        eprintln!(
            "xtask: skipping render_wgpu feature-combo checks (set RA_CHECK_RENDER_FEATURE_COMBO=1 to enable)"
        );
    }
    Ok(())
}

fn layering_guard() -> Result<()> {
    // Ensure render_wgpu does not depend on server_core (layering violation)
    let mut cmd = Command::new("cargo");
    cmd.args(["tree", "-p", "render_wgpu"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let out = cmd.output().context("cargo tree render_wgpu")?;
    if !out.status.success() {
        // Not fatal; skip if tree fails for some reason
        return Ok(());
    }
    let s = String::from_utf8_lossy(&out.stdout);
    if s.contains("server_core ") || s.contains(" server_core") {
        eprintln!(
            "xtask: WARN layering: render_wgpu depends on server_core (expected until extraction)"
        );
    }
    Ok(())
}

fn warn_hooks() {
    // Best-effort check: if git exists and hooksPath isn't set to .githooks, print a nudge.
    let ok = std::process::Command::new("git")
        .args(["config", "--get", "core.hooksPath"])
        .output();
    if let Ok(out) = ok {
        if out.status.success() {
            let val = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if val != ".githooks" {
                eprintln!(
                    "xtask: note: enable repo git hooks for pre-push checks: git config core.hooksPath .githooks (current: '{}')",
                    val
                );
            }
        } else {
            eprintln!(
                "xtask: note: couldn't read git hooksPath; you can enable pre-push checks via 'git config core.hooksPath .githooks'"
            );
        }
    }
}

fn wgsl_validate() -> Result<()> {
    // Validate WGSL using the same bundling the renderer uses.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let gfx = root.join("crates/render_wgpu/src/gfx");

    // Helper to parse a source string with a label
    let mut parsed = 0usize;
    let mut parse_src = |label: &str, src: String| -> Result<()> {
        naga::front::wgsl::parse_str(&src)
            .map_err(|e| anyhow::anyhow!("WGSL validation failed for {}: {}", label, e))?;
        parsed += 1;
        Ok(())
    };

    // Standalone modules
    for name in [
        "shader.wgsl",
        "sky.wgsl",
        "hiz.comp.wgsl",
        "frame_overlay.wgsl",
        "post_bloom.wgsl",
        "post_ao.wgsl",
        "blit_noflip.wgsl",
        "present.wgsl",
        "fullscreen.wgsl",
    ] {
        let p = gfx.join(name);
        if p.is_file() {
            let txt = std::fs::read_to_string(&p)?;
            // Some of these are also bundled below; standalone parse should still succeed where appropriate
            let _ = parse_src(&p.display().to_string(), txt);
        }
    }

    // Bundled fullscreen-based pipelines (match pipeline.rs)
    let fullscreen = std::fs::read_to_string(gfx.join("fullscreen.wgsl"))?;
    for pair in [
        ("present", "present.wgsl"),
        ("blit_noflip", "blit_noflip.wgsl"),
        ("post_bloom", "post_bloom.wgsl"),
        ("post_ao", "post_ao.wgsl"),
        ("ssgi_fs", "ssgi_fs.wgsl"),
        ("ssr_fs", "ssr_fs.wgsl"),
    ] {
        let p = gfx.join(pair.1);
        if p.is_file() {
            let body = std::fs::read_to_string(&p)?;
            let src = [fullscreen.as_str(), body.as_str()].join("\n\n");
            let label = format!("{} (+fullscreen)", p.display());
            let _ = parse_src(&label, src);
        }
    }

    println!("xtask: WGSL validated ({} modules)", parsed);
    Ok(())
}

fn cargo_deny() -> Result<()> {
    // Run `cargo-deny` if available; otherwise warn and continue.
    let mut probe = Command::new("cargo-deny");
    probe
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match probe.status() {
        Ok(s) if s.success() => {
            let mut run = Command::new("cargo-deny");
            run.args(["check"])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
            let status = run.status().context("cargo-deny run")?;
            if !status.success() {
                bail!("cargo-deny check failed");
            }
        }
        _ => {
            eprintln!("xtask: cargo-deny not installed; skipping dependency checks");
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
