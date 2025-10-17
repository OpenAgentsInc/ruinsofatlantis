use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::fs;
use std::io::BufRead as _;
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
    /// Convenience alias: build vendored Codex CLI
    CodexBuild,
    /// Convenience alias: run Codex CLI once
    CodexRun {
        /// The single wish text to pass to Codex as the prompt
        #[arg(long)]
        wish: String,
        /// Optional explicit wish id (W-...)
        #[arg(long)]
        id: Option<String>,
        /// Optional JSON array of allowed path globs for scope enforcement
        #[arg(long)]
        scope: Option<String>,
        /// Optional model override for Codex
        #[arg(long)]
        model: Option<String>,
        /// Working directory for Codex (defaults to repo root)
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Time budget in minutes before the Codex process is killed
        #[arg(long, default_value_t = 180u64)]
        timeout_mins: u64,
        /// Mirror raw Codex JSONL to stderr as it arrives
        #[arg(long, default_value_t = false)]
        verbose: bool,
        /// Save raw Codex JSONL to this file (defaults to ~/.roa/wish_runner/<id>.codex.jsonl)
        #[arg(long)]
        raw_file: Option<PathBuf>,
    },
    /// Run a local SSE bridge exposing wish events and WISHES.md
    Bridge {
        /// Address to bind (default 127.0.0.1:7069)
        #[arg(long)]
        addr: Option<String>,
    },
    /// Pretty-print wish events from ~/.roa/wish_runner
    Tail {
        /// Wish id (defaults to latest if omitted)
        id: Option<String>,
        /// Follow (tail -f)
        #[arg(long, default_value_t = false)]
        follow: bool,
        /// Show raw JSON lines as well
        #[arg(long, default_value_t = false)]
        raw: bool,
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
        /// Allow running with a dirty working tree (skips clean check)
        #[arg(long, default_value_t = false)]
        allow_dirty: bool,
        /// Which engine to use: codex (default) or legacy
        #[arg(long, default_value = "codex")]
        engine: String,
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
    /// Build the vendored Codex CLI (codex) binary
    Build,
    /// Run Codex CLI in headless JSONL mode for a one-off wish
    Run {
        /// The single wish text to pass to Codex as the prompt
        #[arg(long)]
        wish: String,
        /// Optional explicit wish id (W-...)
        #[arg(long)]
        id: Option<String>,
        /// Optional JSON array of allowed path globs for scope enforcement
        #[arg(long)]
        scope: Option<String>,
        /// Optional model override for Codex
        #[arg(long)]
        model: Option<String>,
        /// Working directory for Codex (defaults to repo root)
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Time budget in minutes before the Codex process is killed
        #[arg(long, default_value_t = 180u64)]
        timeout_mins: u64,
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
        routing::get,
    };
    // StreamExt not currently needed; keep imports minimal
    use serde_json::json;
    use std::{fs, path::PathBuf, time::Duration};
    use tokio::sync::mpsc;
    use tokio::{
        fs::File,
        io::{AsyncReadExt, AsyncSeekExt},
    };
    use tokio_stream::{Stream, wrappers::ReceiverStream};

    #[derive(Clone)]
    pub struct AppState;

    pub async fn run_bridge(addr: &str) -> anyhow::Result<()> {
        let app = Router::new()
            .route("/last", get(last))
            .route("/wishes", get(wishes).post(wishes_post))
            .route("/ledger/{id}", get(ledger))
            .route("/events/{id}", get(events))
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
        let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(128);
        tokio::spawn(async move {
            let mut offset: u64 = 0;
            if let Ok(md) = tokio::fs::metadata(&path).await {
                offset = md.len();
            }
            loop {
                if let Ok(mut f) = File::open(&path).await {
                    let _ = f.seek(std::io::SeekFrom::Start(offset)).await.ok();
                    let mut buf = String::new();
                    if f.read_to_string(&mut buf).await.is_ok() && !buf.is_empty() {
                        offset += buf.len() as u64;
                        for line in buf.lines() {
                            if tx
                                .send(Ok(Event::default().data(line.to_string())))
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        });
        Sse::new(ReceiverStream::new(rx))
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

        WishCmd::Bridge { addr } => {
            let addr = addr.unwrap_or_else(|| "127.0.0.1:7069".to_string());
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async move { bridge::run_bridge(&addr).await })?;
            Ok(())
        }
        WishCmd::Tail { id, follow, raw } => {
            tail_events(id.as_deref(), follow, raw)?;
            Ok(())
        }
        WishCmd::CodexBuild => {
            codex_build()?;
            Ok(())
        }
        WishCmd::CodexRun {
            wish,
            id,
            scope,
            model,
            cwd,
            timeout_mins,
            verbose,
            raw_file,
        } => {
            let scope_globs: Vec<String> = if let Some(s) = scope {
                serde_json::from_str(&s).unwrap_or_else(|_| vec!["**".into()])
            } else {
                vec!["**".into()]
            };
            codex_run(
                &wish,
                id.as_deref(),
                &scope_globs,
                model.as_deref(),
                cwd.as_ref(),
                timeout_mins,
                verbose,
                raw_file.as_ref(),
            )?;
            Ok(())
        }
        WishCmd::Codex { cmd } => match cmd {
            CodexCmd::Plan {
                file,
                region: _,
                out,
                live,
            } => {
                // existing plan path kept as-is
                let input = wishcraft_openai::conduit::PlanInput {
                    repo: std::env::current_dir()?
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    paths: vec!["**".into()],
                    objective: std::fs::read_to_string(&file).unwrap_or_else(|_| {
                        format!("Read wish objective from {} failed", file.display())
                    }),
                    invariants: vec![],
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
            CodexCmd::Build => {
                codex_build()?;
                Ok(())
            }
            CodexCmd::Run {
                wish,
                id,
                scope,
                model,
                cwd,
                timeout_mins,
            } => {
                // Determine scope
                let scope_globs: Vec<String> = if let Some(s) = scope {
                    serde_json::from_str(&s).unwrap_or_else(|_| vec!["**".into()])
                } else {
                    vec!["**".into()]
                };
                codex_run(
                    &wish,
                    id.as_deref(),
                    &scope_globs,
                    model.as_deref(),
                    cwd.as_ref(),
                    timeout_mins,
                    false,
                    None,
                )?;
                Ok(())
            }
        },
        WishCmd::Run {
            wish,
            timeout_mins,
            max_iters,
            allow_dirty,
            engine,
        } => {
            if engine.to_lowercase() == "codex" {
                let cfg = RunnerConfig {
                    max_time_minutes: timeout_mins.unwrap_or(180),
                    max_iters: max_iters.unwrap_or(50),
                    stall_window: 5,
                    fix_tries_per_step: 3,
                    require_consecutive_greens: 2,
                };
                run_wish_orchestration_codex(&wish, allow_dirty, cfg)?;
                Ok(())
            } else {
                let cfg = RunnerConfig {
                    max_time_minutes: timeout_mins.unwrap_or(180),
                    max_iters: max_iters.unwrap_or(50),
                    stall_window: 5,
                    fix_tries_per_step: 3,
                    require_consecutive_greens: 2,
                };
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(async move { run_wish_orchestration(&wish, allow_dirty, cfg).await })?;
                Ok(())
            }
        }
    }
}

/// Build the vendored Codex CLI binary, returning its path on success.
fn codex_build() -> anyhow::Result<std::path::PathBuf> {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../third_party/openai-codex/codex-rs");
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--release")
        .arg("-p")
        .arg("codex-cli")
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"));
    eprintln!("xtask: building Codex CLI (vendored)...");
    let status = cmd.status().context("codex build")?;
    if !status.success() {
        anyhow::bail!("codex build failed");
    }
    let bin = root.join("target/release/codex");
    if !bin.is_file() {
        anyhow::bail!(format!("codex binary not found at {}", bin.display()));
    }
    Ok(bin)
}

fn resolve_codex_bin() -> anyhow::Result<std::path::PathBuf> {
    if let Ok(p) = std::env::var("ROA_CODEX_BIN") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Ok(pb);
        }
    }
    let vendored = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../third_party/openai-codex/codex-rs/target/release/codex");
    if vendored.is_file() {
        return Ok(vendored);
    }
    // Try PATH
    if let Ok(path) = which::which("codex") {
        return Ok(path);
    }
    // As a last resort, build it
    codex_build()
}

fn codex_run(
    wish_text: &str,
    wish_id_opt: Option<&str>,
    scope_globs: &[String],
    model: Option<&str>,
    cwd: Option<&PathBuf>,
    timeout_mins: u64,
    verbose: bool,
    raw_file: Option<&PathBuf>,
) -> anyhow::Result<()> {
    use std::io::Write;
    use std::sync::mpsc;
    let repo_root = std::env::current_dir()?;
    // Generate or reuse a wish id; avoid touching WISHES.md when running isolated
    let wish_id = wish_id_opt
        .map(|s| s.to_string())
        .unwrap_or_else(|| generate_wish_id(wish_text));
    // Create an isolated git worktree for this run
    let run_root = create_isolated_worktree(&repo_root, &wish_id)?;
    eprintln!(
        "[codex-run] using isolated worktree at {}",
        run_root.display()
    );
    let allowed_paths: Vec<String> = if !scope_globs.is_empty() {
        scope_globs.to_vec()
    } else {
        vec!["**".into()]
    };
    let accept_cmd: Option<String> = None;
    let start_sha = git_head_sha(&run_root)?;
    persist_wish_schema_file_at(&run_root, &wish_id, wish_text)?;

    let codex = resolve_codex_bin()?;
    let mut cmd = Command::new(codex);
    cmd.arg("exec")
        .arg("--json")
        .arg("--full-auto")
        .arg("--include-plan-tool")
        .arg("-c")
        .arg("include_apply_patch_tool=true")
        .arg("-C")
        .arg(cwd.cloned().unwrap_or(run_root.clone()))
        .arg("-")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::piped());
    if let Some(m) = model {
        cmd.arg("-m").arg(m);
    }

    eprintln!(
        "[codex-run] spawning codex exec --json (timeout={}m) bin={}",
        timeout_mins,
        cmd.get_program().to_string_lossy()
    );
    let mut child = cmd.spawn().context("spawn codex")?;
    if let Some(mut sin) = child.stdin.take() {
        use std::io::Write as _;
        let _ = sin.write_all(wish_text.as_bytes());
        let _ = sin.write_all(b"\n");
    }

    let dir = dirs::home_dir().unwrap().join(".roa/wish_runner");
    std::fs::create_dir_all(&dir)?;
    let events_path = dir.join(format!("{}.events.jsonl", wish_id));
    let mut events_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&events_path)?;
    eprintln!(
        "[codex-run] wish_id={} events_file={}",
        wish_id,
        events_path.display()
    );

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let (tx_out, rx_out) = mpsc::channel::<String>();
    let (tx_err, rx_err) = mpsc::channel::<String>();
    let raw_path = raw_file
        .cloned()
        .unwrap_or_else(|| dir.join(format!("{}.codex.jsonl", wish_id)));
    let stderr_path = dir.join(format!("{}.stderr.log", wish_id));
    let raw_path_clone = raw_path.clone();
    std::thread::spawn(move || {
        let reader = std::io::BufReader::new(stdout);
        let mut raw = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(raw_path_clone)
            .ok();
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    if let Some(f) = raw.as_mut() {
                        let _ = writeln!(f, "{}", l);
                    }
                    let _ = tx_out.send(l);
                }
                Err(_) => break,
            }
        }
    });
    std::thread::spawn(move || {
        let reader = std::io::BufReader::new(stderr);
        let mut ferr = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(stderr_path)
            .ok();
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    if let Some(f) = ferr.as_mut() {
                        let _ = writeln!(f, "{}", l);
                    }
                    let _ = tx_err.send(l);
                }
                Err(_) => break,
            }
        }
    });

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_mins * 60);
    let mut last_progress = std::time::Instant::now();
    let mut lines_seen: u64 = 0;
    loop {
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = write_ledger(&wish_id, "Timeout", &[]);
            break;
        }
        // Drain stdout channel
        while let Ok(line) = rx_out.try_recv() {
            lines_seen += 1;
            last_progress = std::time::Instant::now();
            if verbose {
                eprintln!("[codex-run][raw] {}", line);
            }
            if let Err(e) = translate_and_emit_codex_line(&wish_id, &line, &mut events_file) {
                let evt = serde_json::json!({"ts": chrono::Utc::now().to_rfc3339(), "wish_id": wish_id, "iteration": 0, "kind": "wish.terminal", "data": {"line": line.trim_end(), "error": e.to_string()} });
                writeln!(events_file, "{}", serde_json::to_string(&evt)?)?;
            }
        }
        // Drain stderr channel
        while let Ok(line) = rx_err.try_recv() {
            last_progress = std::time::Instant::now();
            let evt = serde_json::json!({"ts": chrono::Utc::now().to_rfc3339(), "wish_id": wish_id, "iteration": 0, "kind": "wish.terminal", "data": {"line": line.trim_end()} });
            writeln!(events_file, "{}", serde_json::to_string(&evt)?)?;
        }
        // Keep-alive every ~5s so users see movement
        if last_progress.elapsed().as_secs() >= 5 {
            eprintln!(
                "[codex-run] alive, lines={} ({}s idle)",
                lines_seen,
                last_progress.elapsed().as_secs()
            );
            last_progress = std::time::Instant::now();
        }
        // Check child status
        if let Some(status) = child.try_wait().ok().flatten() {
            // Process ended
            let ok = status.success();
            // Scope audit
            match audit_and_maybe_revert(&repo_root, &start_sha, &allowed_paths) {
                Ok(()) => {}
                Err(e) => {
                    let evt = serde_json::json!({"ts": chrono::Utc::now().to_rfc3339(), "wish_id": wish_id, "iteration": 0, "kind": "scope.audit.error", "data": {"error": e.to_string()} });
                    writeln!(events_file, "{}", serde_json::to_string(&evt)?)?;
                }
            }
            // Acceptance (optimize docs-only scope)
            let docs_only = allowed_paths
                .iter()
                .all(|g| g.starts_with("docs/") || g.starts_with("./docs/"));
            let (ok_build, ok_test) = if docs_only {
                (true, true)
            } else {
                let rt = tokio::runtime::Runtime::new()?;
                let (b_ok, _) = rt.block_on(run_cmd(
                    &repo_root,
                    "cargo",
                    &["build", "--workspace", "--all-targets"],
                ))?;
                let (t_ok, _) = match &accept_cmd {
                    Some(cmd) => rt.block_on(run_shell(&repo_root, cmd))?,
                    None => {
                        rt.block_on(run_cmd(&repo_root, "cargo", &["test", "--workspace", "-q"]))?
                    }
                };
                (b_ok, t_ok)
            };
            if ok && ok_build && ok_test {
                mark_wish_completed(&wish_id, wish_text)?;
                write_ledger(&wish_id, "Success", &[])?;
                let evt = serde_json::json!({"ts": chrono::Utc::now().to_rfc3339(), "wish_id": wish_id, "iteration": 0, "kind": "wish.success", "data": null });
                writeln!(events_file, "{}", serde_json::to_string(&evt)?)?;
            } else {
                let evt = serde_json::json!({"ts": chrono::Utc::now().to_rfc3339(), "wish_id": wish_id, "iteration": 0, "kind": "wish.failed", "data": {"exit_ok": ok, "build": ok_build, "tests": ok_test} });
                writeln!(events_file, "{}", serde_json::to_string(&evt)?)?;
            }
            // Summary footer (changes, patches, tokens, session)
            if let Err(e) = write_final_summary_event(
                &wish_id,
                &events_path,
                &repo_root,
                &start_sha,
                &mut events_file,
            ) {
                eprintln!("xtask: final summary failed: {}", e);
            }
            break;
        }
        // brief sleep to avoid busy loop
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Ok(())
}

fn generate_wish_id(wish_text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(wish_text.as_bytes());
    format!(
        "W-{}-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S"),
        hex::encode(h.finalize())[..4].to_string()
    )
}

fn git_head_sha(root: &std::path::PathBuf) -> anyhow::Result<String> {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "HEAD"])
        .output()?;
    if !out.status.success() {
        anyhow::bail!(
            "git rev-parse HEAD failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn audit_and_maybe_revert(
    root: &std::path::PathBuf,
    base_sha: &str,
    allowed: &[String],
) -> anyhow::Result<()> {
    use globset::{Glob, GlobSetBuilder};
    let mut builder = GlobSetBuilder::new();
    for g in allowed {
        builder.add(Glob::new(g).map_err(|e| anyhow::anyhow!("bad glob {}: {}", g, e))?);
    }
    let set = builder
        .build()
        .map_err(|e| anyhow::anyhow!("glob build: {}", e))?;
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["diff", "--name-only", base_sha, "HEAD"])
        .output()?;
    if !out.status.success() {
        anyhow::bail!("git diff failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    let changed = String::from_utf8_lossy(&out.stdout);
    let mut violations = Vec::new();
    for p in changed.lines() {
        if p.trim().is_empty() {
            continue;
        }
        if !set.is_match(p) {
            violations.push(p.to_string());
        }
    }
    // Detect deletions inside the allowed scope and block them by policy
    let out_del = std::process::Command::new("git")
        .current_dir(root)
        .args(["diff", "--name-status", base_sha, "HEAD"])
        .output()?;
    let mut delete_violations: Vec<String> = Vec::new();
    if out_del.status.success() {
        let s = String::from_utf8_lossy(&out_del.stdout);
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix('D') {
                let path = rest.trim().trim_start_matches('\t');
                if set.is_match(path) {
                    delete_violations.push(path.to_string());
                }
            }
        }
    }
    if violations.is_empty() && delete_violations.is_empty() {
        return Ok(());
    }
    // Revert to base_sha
    let reset = std::process::Command::new("git")
        .current_dir(root)
        .args(["reset", "--hard", base_sha])
        .status()?;
    if !reset.success() {
        anyhow::bail!("git reset --hard failed");
    }
    if !violations.is_empty() {
        eprintln!(
            "[codex-run] scope.violation: reverted files outside scope: {:?}",
            violations
        );
    }
    if !delete_violations.is_empty() {
        eprintln!(
            "[codex-run] delete.policy.violation: reverted deletions under allowed scope: {:?}",
            delete_violations
        );
        anyhow::bail!("delete policy violation under allowed scope");
    }
    Ok(())
}

#[allow(dead_code)]
fn read_line_nonblocking<R: std::io::BufRead>(reader: &mut R) -> anyhow::Result<String> {
    let mut buf = String::new();
    let mut byte = [0u8; 1];
    let mut saw = false;
    loop {
        match std::io::Read::read(reader, &mut byte) {
            Ok(0) => break,
            Ok(_) => {
                saw = true;
                let c = byte[0] as char;
                buf.push(c);
                if c == '\n' {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(anyhow::anyhow!(e)),
        }
        // Clamp lines to a reasonable size
        if buf.len() > 1_000_000 {
            break;
        }
    }
    if saw { Ok(buf) } else { Ok(String::new()) }
}

fn translate_and_emit_codex_line(
    wish_id: &str,
    line: &str,
    out: &mut std::fs::File,
) -> anyhow::Result<()> {
    use std::io::Write;
    let v: serde_json::Value = serde_json::from_str(line)?;
    let mut kind = None::<&'static str>;
    let mut data = serde_json::json!({});
    let t = v.get("type").and_then(|s| s.as_str()).unwrap_or("");
    match t {
        // lowercase variants from codex-rs jsonl
        "thread.started" => {
            kind = Some("codex.session");
            if let Some(id) = v.get("thread_id").and_then(|s| s.as_str()) {
                data = serde_json::json!({"session_id": id});
            }
        }
        "turn.completed" => {
            kind = Some("wish.success");
            if let Some(u) = v.get("usage") {
                data = u.clone();
            }
        }
        "turn.failed" => {
            kind = Some("wish.failed");
        }
        // legacy/camelcase variants
        "ThreadStarted" => {
            kind = Some("codex.session");
            if let Some(id) = v.get("thread_id").and_then(|s| s.as_str()) {
                data = serde_json::json!({"session_id": id});
            }
        }
        "ItemStarted" | "ItemUpdated" | "ItemCompleted" => {
            if let Some(details) = v.get("item").and_then(|i| i.get("details")) {
                let dty = details.get("type").and_then(|s| s.as_str()).unwrap_or("");
                match dty {
                    "reasoning" | "agent_message" => { /* ignore chatty items */ }
                    "TodoList" => {
                        if t == "ItemStarted" {
                            kind = Some("plan.started");
                        } else if t == "ItemUpdated" {
                            kind = Some("plan.updated");
                        } else {
                            kind = Some("plan.completed");
                        }
                        let steps = details
                            .get("items")
                            .and_then(|i| i.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0);
                        data = serde_json::json!({"steps": steps});
                    }
                    "CommandExecution" => {
                        if t == "ItemStarted" {
                            kind = Some("exec.started");
                        } else {
                            kind = Some("exec.completed");
                        }
                        let cmd = details
                            .get("command")
                            .and_then(|s| s.as_str())
                            .unwrap_or("");
                        let status = details.get("status").and_then(|s| s.as_str()).unwrap_or("");
                        let exit_code = details
                            .get("exit_code")
                            .and_then(|n| n.as_i64())
                            .unwrap_or(0);
                        data = serde_json::json!({"command": cmd, "status": status, "exit_code": exit_code});
                    }
                    "McpToolCall" => {
                        if t == "ItemStarted" {
                            kind = Some("tool.started");
                        } else {
                            kind = Some("tool.completed");
                        }
                        let server = details.get("server").and_then(|s| s.as_str()).unwrap_or("");
                        let tool = details.get("tool").and_then(|s| s.as_str()).unwrap_or("");
                        let status = details.get("status").and_then(|s| s.as_str()).unwrap_or("");
                        data =
                            serde_json::json!({"server": server, "tool": tool, "status": status});
                    }
                    "FileChange" => {
                        let status = details.get("status").and_then(|s| s.as_str()).unwrap_or("");
                        if status.eq_ignore_ascii_case("Completed") {
                            kind = Some("patch.applied");
                        } else {
                            kind = Some("patch.failed");
                        }
                        let changes = details
                            .get("changes")
                            .and_then(|a| a.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0);
                        data = serde_json::json!({"changes": changes});
                    }
                    _ => {}
                }
            }
        }
        "TurnCompleted" => {
            kind = Some("wish.success");
            if let Some(u) = v.get("usage") {
                data = u.clone();
            }
        }
        "TurnFailed" => {
            kind = Some("wish.failed");
            if let Some(err) = v
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|s| s.as_str())
            {
                data = serde_json::json!({"error": err});
            }
        }
        _ => {}
    }
    let evt = serde_json::json!({
        "ts": chrono::Utc::now().to_rfc3339(),
        "wish_id": wish_id,
        "iteration": 0,
        "kind": kind.unwrap_or("wish.event"),
        "data": if data.as_object().map(|o| o.is_empty()).unwrap_or(true) { serde_json::Value::Null } else { data }
    });
    writeln!(out, "{}", serde_json::to_string(&evt)?)?;
    Ok(())
}

fn latest_wish_id_in_runner() -> Option<String> {
    let dir = dirs::home_dir()?.join(".roa/wish_runner");
    let mut latest: Option<(String, std::time::SystemTime)> = None;
    if let Ok(rd) = std::fs::read_dir(&dir) {
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
    latest.map(|v| v.0)
}

fn color_kind(kind: &str) -> String {
    match kind {
        "plan.started" => "\x1b[36mplan.started\x1b[0m".to_string(),
        "plan.updated" => "\x1b[36mplan.updated\x1b[0m".to_string(),
        "plan.completed" => "\x1b[36mplan.completed\x1b[0m".to_string(),
        "patch.applied" => "\x1b[32mpatch.applied\x1b[0m".to_string(),
        "patch.failed" => "\x1b[31mpatch.failed\x1b[0m".to_string(),
        "wish.success" => "\x1b[32mwish.success\x1b[0m".to_string(),
        "wish.failed" => "\x1b[31mwish.failed\x1b[0m".to_string(),
        "wish.terminal" => "\x1b[90mwish.terminal\x1b[0m".to_string(),
        "exec.started" => "\x1b[35mexec.started\x1b[0m".to_string(),
        "exec.completed" => "\x1b[35mexec.completed\x1b[0m".to_string(),
        "tool.started" => "\x1b[33mtool.started\x1b[0m".to_string(),
        "tool.completed" => "\x1b[33mtool.completed\x1b[0m".to_string(),
        "codex.session" => "\x1b[34mcodex.session\x1b[0m".to_string(),
        _ => kind.to_string(),
    }
}

fn tail_events(id: Option<&str>, follow: bool, raw: bool) -> anyhow::Result<()> {
    use std::io::BufRead;
    let wish_id = id
        .map(|s| s.to_string())
        .or_else(latest_wish_id_in_runner)
        .ok_or_else(|| anyhow::anyhow!("no wish id provided and no events found"))?;
    let dir = dirs::home_dir().unwrap().join(".roa/wish_runner");
    let path = dir.join(format!("{}.events.jsonl", wish_id));
    let raw_path = dir.join(format!("{}.codex.jsonl", wish_id));
    eprintln!("xtask: tailing {}", path.display());
    let mut counts: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    let fp = std::fs::File::open(&path)?;
    let mut reader = std::io::BufReader::new(fp);
    let mut line = String::new();
    let mut printed = 0usize;
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            if follow {
                std::thread::sleep(std::time::Duration::from_millis(200));
                continue;
            } else {
                break;
            }
        }
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
            let ts = v.get("ts").and_then(|s| s.as_str()).unwrap_or("");
            let kind = v
                .get("kind")
                .and_then(|s| s.as_str())
                .unwrap_or("wish.event");
            *counts.entry(kind.to_string()).or_insert(0) += 1;
            printed += 1;
            // Short data summary
            let mut summary = String::new();
            match kind {
                "patch.applied" => {
                    if let Some(changes) = v
                        .get("data")
                        .and_then(|d| d.get("changes"))
                        .and_then(|n| n.as_u64())
                    {
                        summary = format!("changes={}", changes);
                    }
                }
                "exec.started" | "exec.completed" => {
                    if let Some(cmd) = v
                        .get("data")
                        .and_then(|d| d.get("command"))
                        .and_then(|s| s.as_str())
                    {
                        summary = format!("cmd={}", cmd);
                    }
                }
                "tool.started" | "tool.completed" => {
                    let server = v
                        .get("data")
                        .and_then(|d| d.get("server"))
                        .and_then(|s| s.as_str())
                        .unwrap_or("");
                    let tool = v
                        .get("data")
                        .and_then(|d| d.get("tool"))
                        .and_then(|s| s.as_str())
                        .unwrap_or("");
                    summary = format!("{}:{}", server, tool);
                }
                "wish.success" => {
                    if let Some(u) = v.get("data") {
                        summary = format!("usage={}", u);
                    }
                }
                _ => {}
            }
            println!("{} {} {}", ts, color_kind(kind), summary);
        }
        if raw {
            if let Ok(mut rf) = std::fs::File::open(&raw_path) {
                let mut rr = std::io::BufReader::new(&mut rf);
                let mut rline = String::new();
                while rr.read_line(&mut rline).unwrap_or(0) > 0 {}
                if !rline.is_empty() {
                    eprintln!("\x1b[90mRAW\x1b[0m {}", rline.trim_end());
                }
            }
        }
        if !follow && printed % 25 == 0 {}
        if follow && printed % 25 == 0 {
            eprintln!("-- counts --");
            for (k, c) in &counts {
                eprintln!("{:>6} {}", c, k);
            }
        }
    }
    eprintln!("-- summary --");
    let total: u64 = counts.values().copied().sum();
    for (k, c) in counts {
        eprintln!(
            "{:>6} {:<18} {:>5.1}%",
            c,
            k,
            (c as f64 * 100.0) / (total as f64).max(1.0)
        );
    }
    Ok(())
}

fn write_final_summary_event(
    wish_id: &str,
    events_path: &std::path::Path,
    repo_root: &std::path::Path,
    base_sha: &str,
    events_file: &mut std::fs::File,
) -> anyhow::Result<()> {
    use std::io::Write;
    // Count patch.applied and extract session id and usage if present
    let mut patch_applied = 0u64;
    let mut session_id: Option<String> = None;
    let mut usage: Option<serde_json::Value> = None;
    if let Ok(txt) = std::fs::read_to_string(events_path) {
        for line in txt.lines() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                if v.get("kind").and_then(|s| s.as_str()) == Some("patch.applied") {
                    patch_applied += 1;
                }
                if v.get("kind").and_then(|s| s.as_str()) == Some("codex.session") {
                    if let Some(id) = v
                        .get("data")
                        .and_then(|d| d.get("session_id"))
                        .and_then(|s| s.as_str())
                    {
                        session_id = Some(id.to_string());
                    }
                }
                if v.get("kind").and_then(|s| s.as_str()) == Some("wish.success") {
                    if let Some(d) = v.get("data") {
                        if !d.is_null() {
                            usage = Some(d.clone());
                        }
                    }
                }
            }
        }
    }
    // Changed files list
    let out = std::process::Command::new("git")
        .current_dir(repo_root)
        .args(["diff", "--name-only", base_sha, "HEAD"])
        .output()?;
    let changed_files: Vec<String> = if out.status.success() {
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .collect()
    } else {
        Vec::new()
    };
    // Emit summary event
    let data = serde_json::json!({
        "changed_files": changed_files,
        "patches_applied": patch_applied,
        "usage": usage,
        "session_id": session_id,
    });
    let evt = serde_json::json!({
        "ts": chrono::Utc::now().to_rfc3339(),
        "wish_id": wish_id,
        "iteration": 0,
        "kind": "wish.summary",
        "data": data,
    });
    writeln!(events_file, "{}", serde_json::to_string(&evt)?)?;
    // Print a short human summary
    eprintln!(
        "[summary] files={}, patches={}, session={}",
        evt["data"]["changed_files"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0),
        patch_applied,
        evt["data"]["session_id"].as_str().unwrap_or("")
    );
    if let Some(id) = evt["data"]["session_id"].as_str() {
        eprintln!("[resume] codex resume {}", id);
    }
    Ok(())
}

fn create_isolated_worktree(
    repo_root: &std::path::PathBuf,
    wish_id: &str,
) -> anyhow::Result<std::path::PathBuf> {
    let base_sha = git_head_sha(repo_root)?;
    let work_dir = dirs::home_dir()
        .unwrap()
        .join(".roa/wish_runner")
        .join(format!("{}.worktree", wish_id));
    if work_dir.exists() {
        return Ok(work_dir);
    }
    std::fs::create_dir_all(&work_dir)?;
    let status = std::process::Command::new("git")
        .current_dir(repo_root)
        .args([
            "worktree",
            "add",
            "--detach",
            work_dir.to_str().unwrap(),
            &base_sha,
        ])
        .status()?;
    if !status.success() {
        anyhow::bail!("git worktree add failed");
    }
    Ok(work_dir)
}

fn persist_wish_schema_file_at(
    root: &std::path::Path,
    wish_id: &str,
    wish_text: &str,
) -> anyhow::Result<()> {
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
    let dir = root.join("data/wishes/inbox");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join(format!("{wish_id}.yaml")),
        serde_yaml::to_string(&w)?,
    )?;
    Ok(())
}

fn run_wish_orchestration_codex(
    wish_text: &str,
    allow_dirty: bool,
    cfg: RunnerConfig,
) -> anyhow::Result<()> {
    let repo_root = std::env::current_dir()?;
    if !allow_dirty {
        if let Err(e) = git_assert_clean(&repo_root) {
            eprintln!(
                "working tree not clean. Stash or commit changes, or pass --allow-dirty to proceed (will proceed on main). Error: {e}"
            );
            return Err(e);
        }
    } else {
        eprintln!("xtask: proceeding with dirty working tree (--allow-dirty)");
    }
    // Do not switch branches for Codex engine; operate on current (expected main)
    // Resolve scope/id from WISHES.md and then spawn codex
    let (_id, scope, _acc) =
        ensure_wishes_md_and_get_meta(None, &generate_wish_id(wish_text), wish_text)?;
    let scope = if scope.is_empty() {
        vec!["**".to_string()]
    } else {
        scope
    };
    codex_run(
        wish_text,
        None,
        &scope,
        None,
        Some(&repo_root),
        cfg.max_time_minutes,
        false,
        None,
    )?;
    Ok(())
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
    allow_dirty: bool,
    cfg: RunnerConfig,
) -> anyhow::Result<()> {
    use sha2::{Digest, Sha256};
    use std::time::{Duration, Instant};

    let mut h = Sha256::new();
    h.update(wish_text.as_bytes());
    let generated_id = format!(
        "W-{}-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S"),
        hex::encode(h.finalize())[..4].to_string()
    );
    let (wish_id, allowed_paths, accept_cmd) =
        ensure_wishes_md_and_get_meta(None, &generated_id, wish_text)?;

    let repo_root = std::env::current_dir()?;
    if !allow_dirty {
        if let Err(e) = git_assert_clean(&repo_root) {
            eprintln!(
                "working tree not clean. Stash or commit changes, or pass --allow-dirty to proceed (will commit on a new branch). Error: {e}"
            );
            return Err(e);
        }
    } else {
        eprintln!("xtask: proceeding with dirty working tree (--allow-dirty)");
    }
    git_checkout_branch(&repo_root, &format!("wish/{wish_id}"))?;

    persist_wish_schema_file(&wish_id, wish_text)?;

    let oa_cfg = wishcraft_openai::config::OpenAIConfig::from_env_defaults()?;
    let client = wishcraft_openai::client::OpenAIClient::new(oa_cfg);
    let conduit = wishcraft_openai::OpenAIConduit::new(client.clone());

    let start = Instant::now();
    let mut iteration: u32 = 0;
    let mut last_plan_hash: Option<String> = None;
    let mut stall_counter: u32 = 0;
    // breaker_counter removed; we bail immediately once retries exceed budget
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
            // Log patch request summary
            let preview = step.chars().take(120).collect::<String>();
            let _ = emit_event(
                &wish_id,
                iteration,
                "patch.request",
                Some(serde_json::json!({"step": idx, "preview": preview})),
            );
            eprintln!(
                "[wish-run] step={} preview=\"{}\"",
                idx,
                preview.replace('\n', " ")
            );
            let patch = generate_patch_for_step(&client, wish_text, step).await?;
            if !looks_like_unified_diff(&patch) {
                emit_event(
                    &wish_id,
                    iteration,
                    "patch.skipped",
                    Some(serde_json::json!({"step": idx, "reason": "empty-or-invalid-diff"})),
                )?;
                continue;
            }
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
        // no breaker counter; failures above already bail with a clear reason
    }
    Ok(())
}

async fn generate_patch_for_step(
    client: &wishcraft_openai::client::OpenAIClient,
    wish_text: &str,
    step: &str,
) -> anyhow::Result<String> {
    let base = codex_base_instructions_for_model(&client.cfg.model);
    let user = format!(
        "Return only a unified diff patch (git apply compatible) with proper file paths relative to repo root. No prose.\\n\\nWish: {wish}\\nImplement step:\\n{step}\\nRules:\\n- Keep build and tests green.\\n- Include sufficient context lines in hunks.\\n- Do not modify files outside scope.",
        wish = wish_text,
        step = step
    );
    let body = serde_json::json!({
        "model": client.cfg.model,
        "instructions": base,
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
    let base = codex_base_instructions_for_model(&client.cfg.model);
    let user = format!(
        "You fix code. Return only a unified diff patch (git apply compatible). No prose.\\n\\nWish: {wish}\\nFix kind: {kind}\\nStep: {step}\\nErrors (trimmed):\\n{errs}\\nRules:\\n- Include only necessary changes.\\n- Preserve formatting and license headers.\\n- No comments in patch.",
        wish = wish_text,
        kind = kind,
        step = step,
        errs = trim_long(error_text, 6000)
    );
    let body = serde_json::json!({
        "model": client.cfg.model,
        "instructions": base,
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

fn codex_base_instructions_for_model(model: &str) -> &'static str {
    const PROMPT_BASE: &str =
        include_str!("../../third_party/openai-codex/codex-rs/core/prompt.md");
    const PROMPT_G5_CODEX: &str =
        include_str!("../../third_party/openai-codex/codex-rs/core/gpt_5_codex_prompt.md");
    if model.starts_with("gpt-5-codex") || model.starts_with("codex-") {
        PROMPT_G5_CODEX
    } else {
        PROMPT_BASE
    }
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

// (deprecated) legacy helper retained for context; no longer used.

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
    let lines: Vec<String> = s.lines().map(|l| l.to_string()).collect();
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
    if let Some(it) = target {
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

fn looks_like_unified_diff(patch: &str) -> bool {
    let mut has_header = false;
    let mut has_hunks = false;
    for line in patch.lines() {
        if line.starts_with("diff --git ") || line.starts_with("--- ") || line.starts_with("+++") {
            has_header = true;
        }
        if line.starts_with("@@ ") {
            has_hunks = true;
        }
        if has_header && has_hunks {
            return true;
        }
    }
    false
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
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        if err.contains("No valid patches") {
            // Treat as skip; no commit is made
            return Err(anyhow::anyhow!("skip-empty-diff"));
        }
        return Err(anyhow::anyhow!(format!("git apply failed: {}", err)));
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
fn git_current_branch(root: &std::path::PathBuf) -> anyhow::Result<String> {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()?;
    if !out.status.success() {
        anyhow::bail!(
            "git rev-parse failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn git_checkout_branch(root: &std::path::PathBuf, name: &str) -> anyhow::Result<()> {
    if git_current_branch(root).unwrap_or_default() == name {
        return Ok(());
    }
    // Try to checkout existing branch
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["checkout", name])
        .output()?;
    if out.status.success() {
        return Ok(());
    }
    // Create new branch
    let out_new = std::process::Command::new("git")
        .current_dir(root)
        .args(["checkout", "-b", name])
        .output()?;
    if !out_new.status.success() {
        anyhow::bail!(
            "git checkout failed: {}",
            String::from_utf8_lossy(&out_new.stderr)
        );
    }
    Ok(())
}
