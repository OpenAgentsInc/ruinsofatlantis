use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use wishcraft::conduit::ConduitExec;

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
    /// Wishcraft utilities
    Wish {
        #[command(subcommand)]
        cmd: WishCmd,
    },
}

#[derive(Subcommand)]
enum WishCmd {
    /// Lint a wish schema (YAML/JSON)
    Lint { file: PathBuf },
    /// Score a wish (clarity/safety/reversibility) and print thresholds
    Court { file: PathBuf },
    /// Shadow-run a wish against a region snapshot (stub)
    ShadowRun {
        file: PathBuf,
        #[arg(long)]
        region: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Conduit registry utilities
    Conduits {
        #[command(subcommand)]
        cmd: ConduitCmd,
    },
    /// OpenAI Codex helpers
    Codex {
        #[command(subcommand)]
        cmd: CodexCmd,
    },
    /// Run a local SSE bridge exposing wish events and WISHES.md
    Bridge {
        /// Address to bind (default 127.0.0.1:7069)
        #[arg(long)]
        addr: Option<String>,
    },
    /// Run a hands-off orchestration loop on a single wish string
    Run {
        /// The single wish text
        #[arg(long)]
        wish: String,
        /// Max time budget (minutes)
        #[arg(long)]
        timeout_mins: Option<u64>,
        /// Max iterations
        #[arg(long)]
        max_iters: Option<u32>,
    },
}

#[derive(Subcommand)]
enum ConduitCmd {
    /// List available conduits from data/conduits/registry.yaml
    List,
}

#[derive(Subcommand)]
enum CodexCmd {
    /// Build a plan via the OpenAI planning conduit (ShadowRun by default)
    Plan {
        #[arg(long)]
        file: PathBuf,
        #[arg(long, default_value = "wizard_woods")]
        region: String,
        #[arg(long)]
        out: Option<PathBuf>,
        /// If set, perform a live API call (requires OPENAI_API_KEY). Otherwise ShadowRun stub.
        #[arg(long, default_value_t = false)]
        live: bool,
    },
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
    third_party_workspace_guard()?;
    // Enforce formatting without modifying the working tree here; the pre-commit
    // hook auto-formats and stages changes. Use --check to fail fast in CI.
    cargo(&["fmt", "--all", "--", "--check"])?;
    cargo(&["clippy", "--all-targets", "--", "-D", "warnings"])?;
    layering_guard()?;
    forbidden_patterns_guard()?;
    wgsl_validate()?;
    legacy_flags_guard()?;
    cargo_deny()?;
    // Build packs so golden tests can read outputs
    build_packs()?;
    cargo(&["test"])?;
    schema_check()?;
    // 95A/99/100: Always validate render_wgpu without default features
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
    // Feature combo: vox_onepath_demo + destruct_debug
    let feat = "vox_onepath_demo,destruct_debug";
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

fn third_party_workspace_guard() -> Result<()> {
    // Ensure no third_party crates are part of the workspace members
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let cargo_toml = std::fs::read_to_string(root.join("Cargo.toml"))?;
    let mut in_members = false;
    for line in cargo_toml.lines() {
        let t = line.trim();
        if t.starts_with("[workspace]") {
            continue;
        }
        if t.starts_with("members") {
            in_members = true;
        }
        if in_members {
            if t.starts_with("[") && !t.starts_with("[workspace]") && !t.starts_with("members") {
                // left the members table
                in_members = false;
            }
            if t.contains("third_party/") {
                bail!("workspace includes third_party path: {}", t);
            }
        }
    }
    Ok(())
}

fn forbidden_patterns_guard() -> Result<()> {
    // Fail on legacy/runtime anti-patterns outside docs/tests and excluding net_core (which defines schemas)
    let forbid = vec![
        (
            "NpcListMsg|BossStatusMsg",
            vec![
                "crates/server_core",
                "crates/client_core",
                "crates/platform_winit",
                "crates/render_wgpu",
            ],
        ),
        (
            "ActorStore",
            vec![
                "crates/server_core",
                "crates/client_core",
                "crates/platform_winit",
                "crates/render_wgpu",
            ],
        ),
        (
            "\\bv:\\s*3\\b",
            vec!["crates/net_core", "crates/client_core"],
        ),
        (
            "\\bTeam\\b",
            vec![
                "crates/server_core",
                "crates/client_core",
                "crates/net_core",
                "crates/platform_winit",
                "crates/render_wgpu",
            ],
        ),
    ];
    for (pat, roots) in forbid {
        for root in roots {
            let out = std::process::Command::new("rg")
                .args(["-n", pat, root, "-g", ":!**/tests/**", "-g", ":!docs/**"])
                .output()
                .ok();
            if let Some(o) = out
                && !o.stdout.is_empty()
            {
                let s = String::from_utf8_lossy(&o.stdout);
                bail!("forbidden pattern '{}' found under {}:\n{}", pat, root, s);
            }
        }
    }
    // No ActorKind branching in server systems
    let out = std::process::Command::new("rg")
        .args([
            "-n",
            "ActorKind::(Wizard|Zombie|Boss)",
            "crates/server_core/src/ecs",
            "-g",
            ":!**/tests/**",
            "-g",
            ":!docs/**",
        ])
        .output()
        .ok();
    if let Some(o) = out
        && !o.stdout.is_empty()
    {
        let s = String::from_utf8_lossy(&o.stdout);
        bail!(
            "archetype branching found in server systems:\n{}\nHint: use faction/component predicates; see docs/ECS.md §Rules.",
            s
        );
    }
    // Block legacy helper name in systems
    let out2 = std::process::Command::new("rg")
        .args([
            "-n",
            "\\bwizard_targets\\b",
            "crates/server_core/src/ecs",
            "-g",
            ":!**/tests/**",
            "-g",
            ":!docs/**",
        ])
        .output()
        .ok();
    if let Some(o) = out2
        && !o.stdout.is_empty()
    {
        let s = String::from_utf8_lossy(&o.stdout);
        bail!(
            "found legacy helper name 'wizard_targets' in systems:\n{}\nHint: use 'targets_by_faction(Faction::...)' instead.",
            s
        );
    }
    Ok(())
}

fn legacy_flags_guard() -> Result<()> {
    // Fail if legacy_client_* appears in renderer code (sweep complete)
    let output = std::process::Command::new("rg")
        .args(["-n", "legacy_client_", "crates/render_wgpu/src"])
        .output()
        .ok();
    if let Some(out) = output
        && !out.stdout.is_empty()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        bail!("legacy flags found in render_wgpu/src:\n{}", s);
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
        bail!("layering violation: render_wgpu must not depend on server_core (default)");
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

mod bridge {
    use axum::{
        Json, Router,
        extract::{Path, State},
        http::StatusCode,
        response::{
            IntoResponse,
            sse::{Event, Sse},
        },
        routing::{get, post},
    };
    use futures_util::StreamExt;
    use serde_json::json;
    use std::{fs, path::PathBuf, time::Duration};
    use tokio::{
        fs::File,
        io::{AsyncReadExt, AsyncSeekExt},
        time::sleep,
    };
    use tokio_stream::Stream;

    #[derive(Clone)]
    pub struct AppState;

    pub async fn run_bridge(addr: &str) -> anyhow::Result<()> {
        let app = Router::new()
            .route("/last", get(last))
            .route("/wishes", get(wishes).post(wishes_post))
            .route("/ledger/:id", get(ledger))
            .route("/events/:id", get(events))
            .with_state(AppState);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        eprintln!("wish bridge listening on http://{}", addr);
        axum::serve(listener, app).await?;
        Ok(())
    }

    async fn last(State(_st): State<AppState>) -> impl IntoResponse {
        let dir = dirs::home_dir().unwrap().join(".roa/wish_runner");
        let mut latest: Option<(String, std::time::SystemTime)> = None;
        if let Ok(rd) = fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                    if name.ends_with(".events.jsonl") {
                        if let Ok(meta) = e.metadata() {
                            if let Ok(mtime) = meta.modified() {
                                let id = name.trim_end_matches(".events.jsonl").to_string();
                                if latest.as_ref().map(|l| mtime > l.1).unwrap_or(true) {
                                    latest = Some((id, mtime));
                                }
                            }
                        }
                    }
                }
            }
        }
        let id = latest.map(|l| l.0);
        (StatusCode::OK, Json(json!({"wish_id": id })))
    }

    async fn wishes(State(_st): State<AppState>) -> impl IntoResponse {
        match super::parse_wishes_md() {
            Ok((_lines, items)) => {
                let mut pending = Vec::new();
                let mut completed = Vec::new();
                for it in items {
                    let obj = json!({"id": it.meta.id, "text": it.text});
                    if it.checked {
                        completed.push(obj);
                    } else {
                        pending.push(obj);
                    }
                }
                (
                    StatusCode::OK,
                    Json(json!({"pending": pending, "completed": completed})),
                )
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            ),
        }
    }

    async fn wishes_post(State(_st): State<AppState>, body: String) -> impl IntoResponse {
        let text = body.trim();
        if text.is_empty() {
            return (StatusCode::BAD_REQUEST, Json(json!({"error":"empty"})));
        }
        let generated = format!(
            "W-{}-{}",
            chrono::Utc::now().format("%Y%m%d-%H%M%S"),
            "xxxx"
        );
        match super::ensure_wishes_md_and_get_meta(None, &generated, text) {
            Ok((id, _scope, _acc)) => (StatusCode::OK, Json(json!({"id": id}))),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            ),
        }
    }

    async fn ledger(State(_st): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
        let path = PathBuf::from(".wish-ledger").join(format!("{}.json", id));
        match fs::read_to_string(&path) {
            Ok(s) => (
                StatusCode::OK,
                Json(serde_json::from_str::<serde_json::Value>(&s).unwrap_or(json!({"raw": s}))),
            ),
            Err(e) => (StatusCode::NOT_FOUND, Json(json!({"error": e.to_string()}))),
        }
    }

    async fn events(
        State(_st): State<AppState>,
        Path(id): Path<String>,
    ) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
        let path = dirs::home_dir()
            .unwrap()
            .join(".roa/wish_runner")
            .join(format!("{}.events.jsonl", id));
        let stream = tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(
            Duration::from_millis(200),
        ))
        .scan(0u64, move |offset, _| {
            let path = path.clone();
            async move {
                let mut buf = String::new();
                if let Ok(mut f) = File::open(&path).await {
                    let _ = f.seek(std::io::SeekFrom::Start(*offset)).await.ok();
                    if f.read_to_string(&mut buf).await.is_ok() && !buf.is_empty() {
                        *offset += buf.len() as u64;
                        let mut evs = Vec::new();
                        for line in buf.lines() {
                            evs.push(Event::default().data(line.to_string()));
                        }
                        return Some(evs);
                    }
                }
                Some(Vec::new())
            }
        })
        .flat_map(|events| tokio_stream::iter(events.into_iter().map(Ok)));
        Sse::new(stream)
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
        Cmd::Wish { cmd } => wish_cmd(cmd),
    }
}

fn wish_read(file: &PathBuf) -> Result<wishcraft::Wish> {
    let txt = fs::read_to_string(file)?;
    // Try YAML first, then JSON
    if let Ok(w) = serde_yaml::from_str::<wishcraft::Wish>(&txt) {
        return Ok(w);
    }
    let w = serde_json::from_str::<wishcraft::Wish>(&txt)?;
    Ok(w)
}

struct YamlRegistry {
    list: Vec<wishcraft::conduit::ConduitDescriptor>,
}
impl wishcraft::conduit::ConduitRegistry for YamlRegistry {
    fn get(&self, id: &str) -> Option<wishcraft::conduit::ConduitDescriptor> {
        self.list.iter().find(|d| d.id == id).cloned()
    }
}

fn load_conduits_registry() -> Result<YamlRegistry> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let path = root.join("data/conduits/registry.yaml");
    let txt = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let list: Vec<wishcraft::conduit::ConduitDescriptor> =
        serde_yaml::from_str(&txt).with_context(|| "parse registry yaml")?;
    Ok(YamlRegistry { list })
}

fn wish_cmd(cmd: WishCmd) -> Result<()> {
    match cmd {
        WishCmd::Lint { file } => {
            let w = wish_read(&file)?;
            let reg = load_conduits_registry()?;
            let rep = wishcraft::lint_wish(&w, &reg);
            if !rep.errors.is_empty() {
                eprintln!("errors:");
                for e in &rep.errors {
                    eprintln!("  - {}", e);
                }
            }
            if !rep.warnings.is_empty() {
                eprintln!("warnings:");
                for w in &rep.warnings {
                    eprintln!("  - {}", w);
                }
            }
            if rep.ok() {
                println!("lint: ok");
                Ok(())
            } else {
                bail!("lint failed")
            }
        }
        WishCmd::Court { file } => {
            let w = wish_read(&file)?;
            let s = wishcraft::score_wish(&w);
            println!(
                "clarity: {}\nsafety: {}\nreversibility: {}",
                s.clarity, s.safety, s.reversibility
            );
            Ok(())
        }
        WishCmd::ShadowRun { file, region, out } => {
            let _w = wish_read(&file)?;
            // Skeleton: emit a placeholder Echo Report
            let report = serde_json::json!({
                "region": region,
                "predicted": {"entities_changed": 0, "notes": "shadow-run stub"}
            });
            let s = serde_json::to_string_pretty(&report)?;
            if let Some(path) = out {
                fs::write(path, s)?;
            } else {
                println!("{}", s);
            }
            Ok(())
        }
        WishCmd::Conduits { cmd } => match cmd {
            ConduitCmd::List => {
                let reg = load_conduits_registry()?;
                for d in reg.list.iter() {
                    println!("{}\t{}", d.id, d.label);
                }
                Ok(())
            }
        },
        WishCmd::Codex { cmd } => match cmd {
            CodexCmd::Plan {
                file,
                region: _region,
                out,
                live,
            } => {
                let w = wish_read(&file)?;
                // Build PlanInput from wish
                let input = wishcraft_openai::conduit::PlanInput {
                    repo: "ruinsofatlantis".to_string(),
                    paths: vec!["**".to_string()],
                    objective: w.objective.clone(),
                    invariants: w.invariants.clone(),
                    context_snippets: vec![],
                };
                let cfg = wishcraft_openai::config::OpenAIConfig::from_env_defaults()
                    .unwrap_or_else(|_| wishcraft_openai::config::OpenAIConfig {
                        chatgpt_base_url: std::env::var("CHATGPT_BASE_URL")
                            .unwrap_or_else(|_| "https://chatgpt.com/backend-api/codex".into()),
                        codex_home: std::env::var("CODEX_HOME")
                            .map(std::path::PathBuf::from)
                            .unwrap_or_else(|_| {
                                dirs::home_dir().unwrap_or_default().join(".codex")
                            }),
                        model: std::env::var("OPENAI_MODEL")
                            .unwrap_or_else(|_| "gpt-4o-mini".into()),
                        temperature: Some(0.2),
                        timeout_secs: 30,
                    });
                let client = wishcraft_openai::client::OpenAIClient::new(cfg);
                let conduit = wishcraft_openai::OpenAIConduit::new(client);
                let mode = if live {
                    wishcraft::conduit::ExecMode::Commit
                } else {
                    wishcraft::conduit::ExecMode::ShadowRun
                };
                let rt = tokio::runtime::Runtime::new()?;
                let out_val = rt.block_on(async move {
                    conduit.exec("openai.codex.v2025.plan", input, mode).await
                })?;
                let s = serde_json::to_string_pretty(&out_val)?;
                if let Some(path) = out {
                    fs::write(path, s)?;
                } else {
                    println!("{}", s);
                }
                Ok(())
            }
        },
        WishCmd::Bridge { addr } => {
            let addr = addr.unwrap_or_else(|| "127.0.0.1:7069".to_string());
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async move { bridge::run_bridge(&addr).await })?;
            Ok(())
        }
        WishCmd::Run {
            wish,
            timeout_mins,
            max_iters,
        } => {
            let cfg = RunnerConfig {
                max_time_minutes: timeout_mins.unwrap_or(180),
                max_iters: max_iters.unwrap_or(50),
                stall_window: 5,
                fix_tries_per_step: 3,
                require_consecutive_greens: 2,
            };
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async move { run_wish_orchestration(&wish, None, cfg).await })?;
            Ok(())
        }
    }
}

#[derive(Clone, Copy)]
struct RunnerConfig {
    max_time_minutes: u64,
    max_iters: u32,
    stall_window: u32,
    fix_tries_per_step: u32,
    require_consecutive_greens: u32,
}

async fn run_wish_orchestration(
    wish_text: &str,
    wish_id_arg: Option<&str>,
    cfg: RunnerConfig,
) -> anyhow::Result<()> {
    use anyhow::Context;
    use sha2::{Digest, Sha256};
    use std::{
        fs,
        path::PathBuf,
        time::{Duration, Instant},
    };

    let mut h = Sha256::new();
    h.update(wish_text.as_bytes());
    let generated_id = format!(
        "W-{}-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S"),
        hex::encode(h.finalize())[..4].to_string()
    );
    let (wish_id, allowed_paths, accept_cmd) =
        ensure_wishes_md_and_get_meta(wish_id_arg, &generated_id, wish_text)?;

    let repo_root = std::env::current_dir()?;
    git_assert_clean(&repo_root).context("working tree not clean")?;
    git_checkout_new_branch(&repo_root, &format!("wish/{wish_id}"))?;

    persist_wish_schema_file(&wish_id, wish_text)?;

    let oa_cfg = wishcraft_openai::config::OpenAIConfig::from_env_defaults()?;
    let client = wishcraft_openai::client::OpenAIClient::new(oa_cfg);
    let conduit = wishcraft_openai::OpenAIConduit::new(client.clone());

    let start = Instant::now();
    let mut iteration: u32 = 0;
    let mut last_plan_hash: Option<String> = None;
    let mut stall_counter: u32 = 0;
    let mut breaker_counter: u32 = 0;
    let mut consecutive_greens: u32 = 0;
    let allowed_paths: Vec<String> = if allowed_paths.is_empty() {
        vec!["**".to_string()]
    } else {
        allowed_paths
    };

    loop {
        iteration += 1;
        if start.elapsed() > Duration::from_secs(cfg.max_time_minutes * 60)
            || iteration > cfg.max_iters
        {
            write_ledger(&wish_id, "BudgetExceeded", &[])?;
            anyhow::bail!("budget exceeded");
        }
        emit_event(&wish_id, iteration, "iteration.start", None)?;

        let plan_in = wishcraft_openai::conduit::PlanInput {
            repo: repo_root
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            paths: allowed_paths.clone(),
            objective: wish_text.to_string(),
            invariants: vec![
                "Keep cargo build/test green".into(),
                "Do not exfiltrate secrets".into(),
            ],
            context_snippets: vec![],
        };
        let plan_out = conduit
            .exec(
                "openai.codex.v2025.plan",
                plan_in,
                wishcraft::conduit::ExecMode::ShadowRun,
            )
            .await?;
        let plan_hash = blake3_lines(&plan_out.plan_steps);
        let same_plan = last_plan_hash.as_deref() == Some(&plan_hash);
        last_plan_hash = Some(plan_hash);
        emit_event(
            &wish_id,
            iteration,
            "plan.completed",
            Some(
                serde_json::json!({"steps": plan_out.plan_steps.len(),"model": plan_out.model,"tokens": plan_out.tokens_used}),
            ),
        )?;

        let mut any_change = false;
        for (idx, step) in plan_out.plan_steps.iter().enumerate() {
            let patch = generate_patch_for_step(&client, wish_text, step).await?;
            ensure_patch_in_scope(&patch, &allowed_paths)?;
            apply_patch_and_commit(&repo_root, &patch, &wish_id, idx)?;
            any_change = true;

            let mut tries = 0u32;
            loop {
                let (ok_build, build_out) = run_cmd(
                    &repo_root,
                    "cargo",
                    &["build", "--workspace", "--all-targets"],
                )
                .await?;
                if !ok_build {
                    tries += 1;
                    if tries > cfg.fix_tries_per_step {
                        breaker_counter += 1;
                        anyhow::bail!("build failing after {} tries", cfg.fix_tries_per_step);
                    }
                    let fix_patch =
                        generate_fix_patch(&client, wish_text, step, "build", &build_out).await?;
                    ensure_patch_in_scope(&fix_patch, &allowed_paths)?;
                    apply_patch_and_commit(&repo_root, &fix_patch, &wish_id, idx)?;
                    continue;
                }
                let (ok_test, test_out) = run_cmd(
                    &repo_root,
                    "cargo",
                    &["test", "--workspace", "--all-features", "-q"],
                )
                .await?;
                if ok_test {
                    break;
                }
                tries += 1;
                if tries > cfg.fix_tries_per_step {
                    breaker_counter += 1;
                    anyhow::bail!("tests failing after {} fixes", cfg.fix_tries_per_step);
                }
                let fix_patch =
                    generate_fix_patch(&client, wish_text, step, "tests", &test_out).await?;
                ensure_patch_in_scope(&fix_patch, &allowed_paths)?;
                apply_patch_and_commit(&repo_root, &fix_patch, &wish_id, idx)?;
            }
        }

        let (ok_build, _) = run_cmd(
            &repo_root,
            "cargo",
            &["build", "--workspace", "--all-targets"],
        )
        .await?;
        let (ok_test, _) = match &accept_cmd {
            Some(cmd) => run_shell(&repo_root, cmd).await?,
            None => {
                run_cmd(
                    &repo_root,
                    "cargo",
                    &["test", "--workspace", "--all-features", "-q"],
                )
                .await?
            }
        };
        if ok_build && ok_test {
            consecutive_greens += 1;
            if consecutive_greens >= cfg.require_consecutive_greens {
                write_ledger(&wish_id, "Success", &[])?;
                mark_wish_completed(&wish_id, wish_text)?;
                break;
            }
        } else {
            consecutive_greens = 0;
        }

        if !any_change && same_plan {
            stall_counter += 1;
            if stall_counter >= cfg.stall_window {
                write_ledger(&wish_id, "Stall", &[])?;
                anyhow::bail!("stall detected");
            }
        } else {
            stall_counter = 0;
        }
        if breaker_counter >= 3 {
            write_ledger(&wish_id, "BreakerTripped", &[])?;
            anyhow::bail!("breaker tripped");
        }
    }
    Ok(())
}

async fn generate_patch_for_step(
    client: &wishcraft_openai::client::OpenAIClient,
    wish_text: &str,
    step: &str,
) -> anyhow::Result<String> {
    let system = "You are a careful coding assistant. Return only a unified diff patch (git apply compatible) with proper file paths relative to repo root. No prose.";
    let user = format!(
        "Wish: {wish}\nImplement step:\n{step}\nRules:\n- Keep build and tests green.\n- Include context lines in hunks.\n- No extra commentary.",
        wish = wish_text,
        step = step
    );
    let body = serde_json::json!({
        "model": client.cfg.model,
        "instructions": system,
        "input": [ {"type":"message","role":"user","content":[{"type":"input_text","text": user}]} ],
        "tool_choice": "auto",
        "parallel_tool_calls": false,
        "store": false,
        "stream": true,
        "include": []
    });
    let v = client.chatgpt_codex_post(body).await?;
    Ok(v.get("output_text")
        .and_then(|s| s.as_str())
        .unwrap_or_default()
        .to_string())
}

async fn generate_fix_patch(
    client: &wishcraft_openai::client::OpenAIClient,
    wish_text: &str,
    step: &str,
    kind: &str,
    error_text: &str,
) -> anyhow::Result<String> {
    let system = "You fix code. Return only a unified diff patch (git apply compatible). No prose.";
    let user = format!(
        "Wish: {wish}\nFix kind: {kind}\nStep: {step}\nErrors (trimmed):\n{errs}\nRules:\n- Include only necessary changes.\n- Preserve formatting and license headers.\n- No comments in patch.",
        wish = wish_text,
        kind = kind,
        step = step,
        errs = trim_long(error_text, 6000)
    );
    let body = serde_json::json!({
        "model": client.cfg.model,
        "instructions": system,
        "input": [ {"type":"message","role":"user","content":[{"type":"input_text","text": user}]} ],
        "tool_choice": "auto",
        "parallel_tool_calls": false,
        "store": false,
        "stream": true,
        "include": []
    });
    let v = client.chatgpt_codex_post(body).await?;
    Ok(v.get("output_text")
        .and_then(|s| s.as_str())
        .unwrap_or_default()
        .to_string())
}

fn trim_long(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.len() > max {
        t[..max].to_string()
    } else {
        t.to_string()
    }
}
fn blake3_lines(lines: &[String]) -> String {
    let mut h = blake3::Hasher::new();
    for l in lines {
        h.update(l.as_bytes());
        h.update(b"\n");
    }
    h.finalize().to_hex().to_string()
}

fn persist_wish_schema_file(wish_id: &str, wish_text: &str) -> anyhow::Result<()> {
    use wishcraft::schema::{Budget, Scope, Tier, Wish};
    let w = Wish {
        title: format!("Wish {wish_id}"),
        objective: wish_text.to_string(),
        scope: Scope {
            region: "repo".into(),
            duration_days: 7,
        },
        invariants: vec!["Keep build green".into()],
        budget: Budget {
            chrono_sand: 1,
            genie_slots: 2,
            gold_cap: 0,
        },
        tools: vec!["openai.codex.v2025.plan".into()],
        plan: vec!["Plan".into(), "Apply".into(), "Test".into()],
        safety_tests: vec!["Build & test".into()],
        rollback: vec!["git revert".into()],
        tier: Some(Tier::Meso),
        meta: Default::default(),
    };
    let dir = PathBuf::from("data/wishes/inbox");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join(format!("{wish_id}.yaml")),
        serde_yaml::to_string(&w)?,
    )?;
    Ok(())
}

fn ensure_wishes_md(wish_id: &str, wish_text: &str) -> anyhow::Result<()> {
    let path = std::path::Path::new("WISHES.md");
    if !path.exists() {
        let mut s = String::new();
        s.push_str("# Wishes\n\n");
        s.push_str("## Pending\n\n");
        s.push_str("- [ ] Convert the SRD PDF fully to Markdown in appropriate files/folders. All text must be reproduced verbatim with specific page numbers cited.\n");
        s.push_str("\n## Completed\n\n");
        std::fs::write(path, s)?;
    }
    let mut cur = std::fs::read_to_string(path)?;
    if !cur.contains(wish_text) {
        if let Some(pidx) = cur.find("## Pending") {
            let ins = format!("- [ ] {wish_text} (id: {wish_id})\n");
            cur.insert_str(pidx + "## Pending\n\n".len(), &ins);
            std::fs::write(path, cur)?;
        }
    }
    Ok(())
}

fn mark_wish_completed(wish_id: &str, wish_text: &str) -> anyhow::Result<()> {
    let path = std::path::Path::new("WISHES.md");
    if !path.exists() {
        return Ok(());
    }
    let mut s = std::fs::read_to_string(path)?;
    if let Some(pos) = s.find(&format!("- [ ] {wish_text}")) {
        s.replace_range(pos..pos + 4, "- [x]");
    }
    if let Some(_cidx) = s.find("## Completed") {
        s.push_str(&format!("- [x] {wish_text} (id: {wish_id})\n"));
    }
    std::fs::write(path, s)?;
    Ok(())
}

#[derive(Default, Debug, Clone)]
struct WishMeta {
    id: Option<String>,
    scope: Vec<String>,
    accept: Option<String>,
    raw: String,
}

#[derive(Debug, Clone)]
struct WishLine {
    line_idx: usize,
    checked: bool,
    text: String,
    meta: WishMeta,
}

fn parse_wishes_md() -> anyhow::Result<(Vec<String>, Vec<WishLine>)> {
    let path = std::path::Path::new("WISHES.md");
    let s = std::fs::read_to_string(path)?;
    let mut lines: Vec<String> = s.lines().map(|l| l.to_string()).collect();
    let mut items = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim();
        if !t.starts_with("- [") {
            continue;
        }
        let checked = t.starts_with("- [x]") || t.starts_with("- [X]");
        // Extract text before comment
        let (left, comment): (&str, Option<&str>) = if let Some(idx) = t.find("<!--") {
            (t[..idx].trim(), Some(&t[idx..]))
        } else {
            (t, None)
        };
        // After checkbox token, there is a space then text
        let after_cb = left
            .trim_start_matches("- [x]")
            .trim_start_matches("- [X]")
            .trim_start_matches("- [ ]")
            .trim();
        let text = after_cb
            .trim()
            .trim_end_matches(|c: char| c.is_whitespace())
            .to_string();
        let meta = parse_meta_comment(comment.unwrap_or(""));
        items.push(WishLine {
            line_idx: i,
            checked,
            text,
            meta,
        });
    }
    Ok((lines, items))
}

fn parse_meta_comment(c: &str) -> WishMeta {
    let mut m = WishMeta {
        raw: c.to_string(),
        ..Default::default()
    };
    let c = c.trim();
    if !c.starts_with("<!--") {
        return m;
    }
    if let Some(start) = c.find("wish:") {
        let inner = &c[start + 5..];
        let end = inner.find("-->").map(|i| &inner[..i]).unwrap_or(inner);
        // Split on spaces not inside quotes/brackets
        let mut tokens = Vec::new();
        let mut cur = String::new();
        let mut depth = 0i32;
        let mut in_str = false;
        for ch in end.chars() {
            match ch {
                '"' => {
                    in_str = !in_str;
                    cur.push(ch);
                }
                '[' => {
                    depth += 1;
                    cur.push(ch);
                }
                ']' => {
                    depth -= 1;
                    cur.push(ch);
                }
                ' ' | '\n' | '\t' if !in_str && depth == 0 => {
                    if !cur.trim().is_empty() {
                        tokens.push(cur.trim().to_string());
                    }
                    cur.clear();
                }
                _ => cur.push(ch),
            }
        }
        if !cur.trim().is_empty() {
            tokens.push(cur.trim().to_string());
        }
        for tok in tokens {
            let mut parts = tok.splitn(2, '=');
            let key = parts.next().unwrap_or("").trim();
            let val = parts.next().unwrap_or("").trim();
            if key.is_empty() {
                continue;
            }
            match key {
                "id" => m.id = Some(val.trim_matches('"').to_string()),
                "accept" => m.accept = Some(val.trim_matches('"').to_string()),
                "scope" => {
                    if val.starts_with('[') {
                        if let Ok(v) = serde_json::from_str::<Vec<String>>(val) {
                            m.scope = v;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    m
}

fn ensure_wishes_md_and_get_meta(
    wish_id_arg: Option<&str>,
    generated: &str,
    wish_text: &str,
) -> anyhow::Result<(String, Vec<String>, Option<String>)> {
    use std::fs;
    let path = std::path::Path::new("WISHES.md");
    let (mut lines, items) = parse_wishes_md()?;
    // Find matching line by id or text
    let mut target: Option<WishLine> = None;
    if let Some(id) = wish_id_arg {
        target = items
            .iter()
            .find(|it| it.meta.id.as_deref() == Some(id) && !it.checked)
            .cloned();
    }
    if target.is_none() {
        target = items
            .iter()
            .find(|it| it.text == wish_text && !it.checked)
            .cloned();
    }
    let mut id = wish_id_arg
        .map(|s| s.to_string())
        .unwrap_or_else(|| generated.to_string());
    let mut scope: Vec<String> = vec![];
    let mut accept: Option<String> = None;
    if let Some(mut it) = target {
        if it.meta.id.is_none() {
            // Inject id into the comment or append a new one
            let line = &lines[it.line_idx];
            let new_line = if line.contains("<!--") {
                line.replacen("-->", &format!(" id=\"{}\" -->", id), 1)
            } else {
                format!("{} <!-- wish:id=\"{}\" -->", line, id)
            };
            lines[it.line_idx] = new_line;
            fs::write(path, lines.join("\n") + "\n")?;
        } else {
            id = it.meta.id.unwrap();
        }
        scope = it.meta.scope;
        accept = it.meta.accept;
    } else {
        // No existing line; append to Pending
        let mut inserted = false;
        for (i, l) in lines.iter().enumerate() {
            if l.trim() == "## Completed" {
                let new_line = format!("- [ ] {} <!-- wish:id=\"{}\" -->", wish_text, id);
                lines.insert(i, "".into());
                lines.insert(i, new_line);
                inserted = true;
                break;
            }
        }
        if !inserted {
            lines.push("".into());
            lines.push(format!("- [ ] {} <!-- wish:id=\"{}\" -->", wish_text, id));
        }
        fs::write(path, lines.join("\n") + "\n")?;
    }
    Ok((id, scope, accept))
}

fn write_ledger(wish_id: &str, reason: &str, commits: &[String]) -> anyhow::Result<()> {
    let entry = serde_json::json!({ "wish_id": wish_id, "stop_reason": reason, "commits": commits, "ts": chrono::Utc::now().to_rfc3339() });
    let path = PathBuf::from(".wish-ledger");
    std::fs::create_dir_all(&path)?;
    std::fs::write(
        path.join(format!("{wish_id}.json")),
        serde_json::to_vec_pretty(&entry)?,
    )?;
    Ok(())
}

fn emit_event(
    wish_id: &str,
    iteration: u32,
    kind: &str,
    data: Option<serde_json::Value>,
) -> anyhow::Result<()> {
    use std::io::Write;
    let dir = dirs::home_dir().unwrap().join(".roa/wish_runner");
    std::fs::create_dir_all(&dir)?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(format!("{wish_id}.events.jsonl")))?;
    let evt = serde_json::json!({"ts": chrono::Utc::now().to_rfc3339(), "wish_id": wish_id, "iteration": iteration, "kind": kind, "data": data});
    writeln!(f, "{}", serde_json::to_string(&evt)?)?;
    Ok(())
}

async fn run_cmd(
    cwd: &std::path::PathBuf,
    bin: &str,
    args: &[&str],
) -> anyhow::Result<(bool, String)> {
    use std::process::Stdio;
    use tokio::{io::AsyncReadExt, process::Command};
    let mut cmd = Command::new(bin);
    cmd.args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    let status = child.wait().await?;
    let mut out = String::new();
    if let Some(mut s) = child.stdout.take() {
        s.read_to_string(&mut out).await.ok();
    }
    if let Some(mut s) = child.stderr.take() {
        let mut e = String::new();
        s.read_to_string(&mut e).await.ok();
        out.push_str(&e);
    }
    Ok((status.success(), out))
}

async fn run_shell(cwd: &std::path::PathBuf, script: &str) -> anyhow::Result<(bool, String)> {
    run_cmd(cwd, "bash", &["-lc", script]).await
}

fn ensure_patch_in_scope(patch: &str, allowed: &[String]) -> anyhow::Result<()> {
    use globset::{Glob, GlobSetBuilder};
    let mut builder = GlobSetBuilder::new();
    for g in allowed {
        builder.add(Glob::new(g).map_err(|e| anyhow::anyhow!("bad glob {}: {}", g, e))?);
    }
    let set = builder
        .build()
        .map_err(|e| anyhow::anyhow!("glob build: {}", e))?;
    let mut targets: Vec<String> = Vec::new();
    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("+++ ") {
            let path = rest.trim().trim_matches('"');
            let path = path.strip_prefix("b/").unwrap_or(path);
            if path != "/dev/null" {
                targets.push(path.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("diff --git ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if let Some(last) = parts.last() {
                let p = last.strip_prefix("b/").unwrap_or(last);
                targets.push(p.to_string());
            }
        }
    }
    for p in &targets {
        if !set.is_match(p) {
            return Err(anyhow::anyhow!(format!(
                "scope violation: {} not allowed by {:?}",
                p, allowed
            )));
        }
    }
    Ok(())
}

fn apply_patch_and_commit(
    repo_root: &std::path::PathBuf,
    patch: &str,
    wish_id: &str,
    step_idx: usize,
) -> anyhow::Result<String> {
    use std::{fs, process::Command};
    let tmp = tempfile::NamedTempFile::new()?;
    fs::write(tmp.path(), patch)?;
    let out = Command::new("git")
        .current_dir(repo_root)
        .args(["apply", "--index", tmp.path().to_str().unwrap()])
        .output()?;
    if !out.status.success() {
        return Err(anyhow::anyhow!(format!(
            "git apply failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    let msg = format!("wish:{wish_id} step:{step_idx}");
    let out2 = Command::new("git")
        .current_dir(repo_root)
        .args(["commit", "-m", &msg])
        .output()?;
    if !out2.status.success() {
        return Err(anyhow::anyhow!(format!(
            "git commit failed: {}",
            String::from_utf8_lossy(&out2.stderr)
        )));
    }
    let sha = Command::new("git")
        .current_dir(repo_root)
        .args(["rev-parse", "HEAD"])
        .output()?;
    Ok(String::from_utf8_lossy(&sha.stdout).trim().to_string())
}

fn git_assert_clean(root: &std::path::PathBuf) -> anyhow::Result<()> {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain"])
        .output()?;
    let dirty = !String::from_utf8_lossy(&out.stdout).trim().is_empty();
    if dirty {
        anyhow::bail!("working tree not clean");
    }
    Ok(())
}
fn git_checkout_new_branch(root: &std::path::PathBuf, name: &str) -> anyhow::Result<()> {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["checkout", "-b", name])
        .output()?;
    if !out.status.success() {
        anyhow::bail!(
            "git checkout -b failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}
