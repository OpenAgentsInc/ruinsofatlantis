//! platform_winit: window + input + present loop (winit 0.30).
//!
//! Provides a minimal `run()` that creates a window and drives the
//! `render_wgpu::gfx::Renderer` via winit's ApplicationHandler API.

use net_core::snapshot::{SnapshotDecode, SnapshotEncode};
use net_core::transport::Transport;
use render_wgpu::gfx::Renderer;
use std::sync::mpsc;
use wgpu::SurfaceError;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes},
};

// Phase-one scaffolding: split modules by concern (no behavior change yet)
pub mod app;
pub mod builder_overlay;
pub mod input;
pub mod picker;
pub mod replication;
pub mod telemetry;

enum LoaderMsg {
    Progress {
        what: String,
        step: u32,
        of: u32,
    },
    Done(
        Box<
            anyhow::Result<(
                client_core::zone_client::ZonePresentation,
                Option<roa_assets::types::SkinnedMeshCPU>,
            )>,
        >,
    ),
}

// Background asset worker (pure CPU) messages
#[cfg(not(target_arch = "wasm32"))]
enum AssetMsg {
    Progress { i: u32, of: u32 },
    Batch(render_wgpu::gfx::foliage_stream::TreeCpuBatch),
    Done,
}

#[allow(dead_code)]
enum BootMode {
    Picker,
    #[cfg(not(target_arch = "wasm32"))]
    Loading {
        slug: String,
        rx: mpsc::Receiver<LoaderMsg>,
    },
    Running {
        slug: String,
    },
}

#[cfg(test)]
fn can_cast(boot: &BootMode, local_pc: Option<u32>) -> bool {
    matches!(boot, BootMode::Running { .. }) && local_pc.is_some()
}

#[cfg(test)]
mod input_guard_tests {
    use super::{BootMode, can_cast};

    #[test]
    fn guard_blocks_when_not_playing() {
        assert!(!can_cast(&BootMode::Picker, None));
    }

    #[test]
    fn guard_blocks_when_no_pc() {
        assert!(!can_cast(
            &BootMode::Running {
                slug: "cc_demo".into()
            },
            None
        ));
    }

    #[test]
    fn guard_allows_when_playing_and_pc_present() {
        assert!(can_cast(
            &BootMode::Running {
                slug: "cc_demo".into()
            },
            Some(1)
        ));
    }
}

#[derive(Default, Clone)]
struct ZoneEntry {
    slug: String,
    #[allow(dead_code)]
    display: String,
}

#[derive(Default)]
struct ZonePickerModel {
    #[allow(dead_code)]
    filter: String,
    items: Vec<ZoneEntry>,
    selected: usize,
    #[allow(dead_code)]
    load_error: Option<String>,
}

#[allow(dead_code)]
impl ZonePickerModel {
    fn refresh(&mut self) {
        let root = packs_zones_root();
        let mut next: Vec<ZoneEntry> = Vec::new();
        match data_runtime::zone_snapshot::ZoneRegistry::discover(&root) {
            Ok(reg) => {
                for slug in reg.slugs.iter() {
                    let disp = reg
                        .load_meta(slug)
                        .ok()
                        .and_then(|m| m.display_name)
                        .unwrap_or_else(|| slug.to_string());
                    next.push(ZoneEntry {
                        slug: slug.clone(),
                        display: disp,
                    });
                }
            }
            Err(e) => {
                log::warn!("picker: discover() failed at {:?}: {e:?}", root);
            }
        }
        if next.is_empty()
            && let Ok(rd) = std::fs::read_dir(&root)
        {
            for e in rd.flatten() {
                if e.path().join("snapshot.v1").is_dir()
                    && let Some(os) = e.file_name().to_str()
                {
                    let slug = os.to_string();
                    let disp = match slug.as_str() {
                        "wizard_woods" => "Wizard Woods".to_string(),
                        "cc_demo" => "Character Controller Demo".to_string(),
                        _ => slug.clone(),
                    };
                    next.push(ZoneEntry {
                        slug,
                        display: disp,
                    });
                }
            }
        }
        if next.iter().all(|e| e.slug != "wizard_woods") {
            next.push(ZoneEntry {
                slug: "wizard_woods".into(),
                display: "Wizard Woods".into(),
            });
        }
        if next.iter().all(|e| e.slug != "cc_demo") {
            next.push(ZoneEntry {
                slug: "cc_demo".into(),
                display: "Character Controller Demo".into(),
            });
        }
        next.sort_by(|a, b| a.display.to_lowercase().cmp(&b.display.to_lowercase()));
        log::info!(
            "picker: packs root {:?}; zones: {}",
            root,
            next.iter()
                .map(|z| z.slug.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        self.items = next;
        self.selected = 0;
    }
    fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }
    fn select_next(&mut self) {
        if self.selected + 1 < self.items.len() {
            self.selected += 1;
        }
    }
    #[allow(dead_code)]
    fn current_slug(&self) -> Option<String> {
        self.items.get(self.selected).map(|e| e.slug.clone())
    }
    fn display_lines(&self) -> Vec<String> {
        self.items.iter().map(|e| e.display.clone()).collect()
    }
    #[cfg(test)]
    fn refresh_with_root_for_tests(&mut self, root: &std::path::Path) {
        let mut next: Vec<ZoneEntry> = Vec::new();
        match data_runtime::zone_snapshot::ZoneRegistry::discover(root) {
            Ok(reg) => {
                for slug in reg.slugs.iter() {
                    let disp = reg
                        .load_meta(slug)
                        .ok()
                        .and_then(|m| m.display_name)
                        .unwrap_or_else(|| slug.to_string());
                    next.push(ZoneEntry {
                        slug: slug.clone(),
                        display: disp,
                    });
                }
            }
            Err(e) => {
                log::warn!("picker: discover() failed at {:?}: {e:?}", root);
            }
        }
        next.sort_by(|a, b| a.display.to_lowercase().cmp(&b.display.to_lowercase()));
        self.items = next;
        self.selected = 0;
    }
}

#[allow(dead_code)]
fn packs_zones_root() -> std::path::PathBuf {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../packs/zones");
    if ws.exists() {
        ws
    } else {
        here.join("../../packs").join("zones")
    }
}

// Determine whether to spawn demo encounter content (NPC rings, boss, destructible).
// Keep this whitelist explicit to avoid accidental spawns in authoring/testing zones.
#[allow(dead_code)]
fn is_demo_content_zone(slug: &str) -> bool {
    matches!(slug, "wizard_woods")
}

struct App {
    window: Option<Window>,
    state: Option<Renderer>,
    // Loopback transport (server side) used to send snapshots to the client/renderer
    transport_srv: Option<net_core::transport::LocalLoopbackTransport>,
    #[cfg(feature = "demo_server")]
    demo_server: Option<server_core::ServerState>,
    #[cfg(not(target_arch = "wasm32"))]
    last_time: Option<std::time::Instant>,
    #[cfg(target_arch = "wasm32")]
    last_time: Option<web_time::Instant>,
    #[cfg(not(target_arch = "wasm32"))]
    t0: std::time::Instant,
    #[cfg(target_arch = "wasm32")]
    t0: web_time::Instant,
    tick: u32,
    // Delta baseline for interest/deltas (per local client)
    baseline_tick: u64,
    baseline: std::collections::HashMap<u32, net_core::snapshot::ActorRep>,
    interest_radius_m: f32,
    // Simple server-side rate limiter for client commands
    #[cfg(not(target_arch = "wasm32"))]
    last_sec_start: std::time::Instant,
    #[cfg(target_arch = "wasm32")]
    last_sec_start: web_time::Instant,
    cmds_this_sec: u32,
    // Track which destructible instances have been sent to the client
    sent_destr_instances: std::collections::HashSet<u64>,
    #[allow(dead_code)]
    boot: BootMode,
    #[allow(dead_code)]
    picker: ZonePickerModel,
    // Builder mode state (campaign_builder only)
    builder: BuilderState,
    // Background asset worker channel (native only)
    #[cfg(not(target_arch = "wasm32"))]
    asset_tx: Option<mpsc::Sender<AssetMsg>>,
    #[cfg(not(target_arch = "wasm32"))]
    asset_rx: Option<mpsc::Receiver<AssetMsg>>,
    #[cfg(not(target_arch = "wasm32"))]
    pending_foliage_cpu: Vec<render_wgpu::gfx::foliage_stream::TreeCpuBatch>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            window: None,
            state: None,
            transport_srv: None,
            #[cfg(feature = "demo_server")]
            demo_server: None,
            #[cfg(not(target_arch = "wasm32"))]
            last_time: None,
            #[cfg(target_arch = "wasm32")]
            last_time: None,
            tick: 0,
            baseline_tick: 0,
            baseline: std::collections::HashMap::new(),
            interest_radius_m: 40.0,
            #[cfg(not(target_arch = "wasm32"))]
            last_sec_start: std::time::Instant::now(),
            #[cfg(target_arch = "wasm32")]
            last_sec_start: web_time::Instant::now(),
            #[cfg(not(target_arch = "wasm32"))]
            t0: std::time::Instant::now(),
            #[cfg(target_arch = "wasm32")]
            t0: web_time::Instant::now(),
            cmds_this_sec: 0,
            sent_destr_instances: std::collections::HashSet::new(),
            boot: BootMode::Picker,
            picker: Default::default(),
            builder: Default::default(),
            #[cfg(not(target_arch = "wasm32"))]
            asset_tx: None,
            #[cfg(not(target_arch = "wasm32"))]
            asset_rx: None,
            #[cfg(not(target_arch = "wasm32"))]
            pending_foliage_cpu: Vec::new(),
        }
    }
}

struct BuilderState {
    active: bool,
    yaw_deg: f32,
    ws: worldsmithing::WorldsmithingState,
    kinds: Vec<String>,
    kind_idx: usize,
}

impl Default for BuilderState {
    fn default() -> Self {
        let mut rules = worldsmithing::Rules::default();
        rules.allowed_kinds.insert("tree.default".into());
        let caps = worldsmithing::Caps::default();
        let ws = worldsmithing::Builder::new()
            .caps(caps)
            .rules(rules)
            .build();
        Self {
            active: false,
            yaw_deg: 0.0,
            ws,
            kinds: vec!["tree.default".into()],
            kind_idx: 0,
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct SpawnMarker {
    id: String,
    kind: String,
    pos: [f32; 3],
    yaw_deg: f32,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SceneDoc {
    version: String,
    seed: i64,
    layers: Vec<serde_json::Value>,
    instances: Vec<serde_json::Value>,
    logic: SceneLogic,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SceneLogic {
    triggers: Vec<serde_json::Value>,
    spawns: Vec<SpawnMarker>,
    waypoints: Vec<serde_json::Value>,
    links: Vec<serde_json::Value>,
}

#[cfg(not(target_arch = "wasm32"))]
fn data_scene_path(slug: &str) -> std::path::PathBuf {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = here.join("../../data");
    data.join("zones").join(slug).join("scene.json")
}

#[cfg(not(target_arch = "wasm32"))]
fn ensure_scene_parent(path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_scene(path: &std::path::Path) -> Option<SceneDoc> {
    let txt = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&txt).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn save_scene(path: &std::path::Path, mut doc: SceneDoc) -> anyhow::Result<()> {
    if doc.version.is_empty() {
        doc.version = "1.0.0".into();
    }
    let json = serde_json::to_string_pretty(&doc)?;
    std::fs::write(path, json)?;
    Ok(())
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window = event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_title("Ruins of Atlantis")
                        .with_maximized(true),
                )
                .expect("create window");
            // Attach canvas on web builds so it's visible.
            #[cfg(target_arch = "wasm32")]
            {
                use winit::platform::web::WindowExtWebSys;
                if let Some(canvas) = window.canvas() {
                    let _ = web_sys::window()
                        .and_then(|w| w.document())
                        .and_then(|d| d.body())
                        .map(|body| {
                            // Avoid duplicate attachments on hot-reload.
                            if canvas.parent_element().is_none() {
                                let _ = body.append_child(&canvas);
                            }
                        });
                }
            }

            // Initialize Renderer: native blocks; web spawns async.
            #[cfg(not(target_arch = "wasm32"))]
            let mut state = match pollster::block_on(Renderer::new(&window)) {
                Ok(s) => s,
                Err(e) => {
                    log::info!("Renderer init skipped: {e}");
                    event_loop.exit();
                    return;
                }
            };
            // Wire a local replication channel for NPC/Boss status (native only)
            #[cfg(not(target_arch = "wasm32"))]
            {
                let (_srv, _cli) = net_core::transport::LocalLoopbackTransport::new(4096);
                let (tx_cli, rx_cli) = _cli.split();
                state.set_replication_rx(rx_cli);
                state.set_command_tx(tx_cli);
                self.transport_srv = Some(_srv);
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Decide boot mode and optionally load explicit zone batches
                let force_picker = std::env::var("RA_FORCE_PICKER")
                    .map(|v| v == "1")
                    .unwrap_or(false);
                let explicit = detect_zone_slug();
                if !force_picker && let Some(slug) = explicit.as_ref() {
                    if let Ok(zp) = client_core::zone_client::ZonePresentation::load(slug) {
                        let gz = render_wgpu::gfx::zone_batches::upload_zone_batches(&state, &zp);
                        state.set_zone_batches(Some(gz));
                    } else {
                        log::warn!("zone: failed to load snapshot for slug='{}'", slug);
                    }
                }
                self.window = Some(window);
                self.state = Some(state);
                // Boot mode
                self.boot = if !force_picker {
                    if let Some(slug) = explicit {
                        BootMode::Running { slug }
                    } else {
                        BootMode::Picker
                    }
                } else {
                    BootMode::Picker
                };
                if matches!(self.boot, BootMode::Picker) {
                    // Zone Picker: refresh list and force renderer into "no legacy scene" mode.
                    // We set a dummy zone-batch so render_wgpu::Renderer::has_zone_batches() is true,
                    // which suppresses the legacy static scene draws while the picker is shown.
                    self.picker.refresh();
                    if let Some(st) = self.state.as_mut() {
                        let gb = render_wgpu::gfx::zone_batches::GpuZoneBatches {
                            slug: "<picker>".to_string(),
                        };
                        st.set_zone_batches(Some(gb));
                    }
                    if let Some(win) = &self.window {
                        win.set_title("Zone Picker — no zone selected — ↑/↓, Enter, Esc");
                    }
                }
                #[cfg(feature = "demo_server")]
                {
                    let mut srv = server_core::ServerState::new();
                    // Ensure a PC actor exists (server-authoritative player); place at renderer's first wizard or origin
                    let wiz_now = self
                        .state
                        .as_ref()
                        .map(|s| s.wizard_positions())
                        .unwrap_or_default();
                    let pc0 = wiz_now
                        .first()
                        .copied()
                        .unwrap_or(glam::vec3(0.0, 0.6, 0.0));
                    if srv.pc_actor.is_none() {
                        let _ = srv.spawn_pc_at(pc0);
                    }
                    // Delegate zone-specific spawns to server_core::zones
                    if let BootMode::Running { slug } = &self.boot {
                        let _ = server_core::zones::boot_with_zone(&mut srv, slug);
                    }
                    self.demo_server = Some(srv);
                }
                self.last_time = Some(std::time::Instant::now());
                self.tick = 0;
                self.baseline_tick = 0;
                self.baseline = std::collections::HashMap::new();
                // Temp: widen interest culling radius to include far casters/targets in demo
                self.interest_radius_m = 60.0;
                self.last_sec_start = std::time::Instant::now();
                self.cmds_this_sec = 0;
            }

            #[cfg(target_arch = "wasm32")]
            {
                use wasm_bindgen_futures::spawn_local;
                // Defer the renderer construction asynchronously.
                // We'll pick it up in about_to_wait.
                spawn_local(async move {
                    if let Ok(state) = Renderer::new(&window).await {
                        RENDERER_CELL.with(|cell| {
                            *cell.borrow_mut() = Some((window, state));
                        });
                    } else {
                        // log is already set up by wasm main
                        log::error!("Renderer init failed (wasm)");
                    }
                });
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let (Some(window), Some(state)) = (&self.window, &mut self.state) else {
            return;
        };
        if window.id() != window_id {
            return;
        }
        // Zone Picker keyboard handling (native): arrows to change selection; Enter to load
        if matches!(self.boot, BootMode::Picker) {
            use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
            if let WindowEvent::KeyboardInput { event: kev, .. } = &event {
                // Navigation
                match (&kev.logical_key, &kev.physical_key) {
                    (Key::Named(NamedKey::ArrowUp), _)
                    | (_, PhysicalKey::Code(KeyCode::ArrowUp)) => {
                        self.picker.select_prev();
                        if let Some(st) = self.state.as_mut() {
                            st.set_picker_selected_index(self.picker.selected);
                        }
                        let disp = self
                            .picker
                            .items
                            .get(self.picker.selected)
                            .map(|e| e.display.clone())
                            .unwrap_or_else(|| "".into());
                        window.set_title(&format!("Zone Picker — {} — ↑/↓, Enter, Esc", disp));
                        return;
                    }
                    (Key::Named(NamedKey::ArrowDown), _)
                    | (_, PhysicalKey::Code(KeyCode::ArrowDown)) => {
                        self.picker.select_next();
                        if let Some(st) = self.state.as_mut() {
                            st.set_picker_selected_index(self.picker.selected);
                        }
                        let disp = self
                            .picker
                            .items
                            .get(self.picker.selected)
                            .map(|e| e.display.clone())
                            .unwrap_or_else(|| "".into());
                        window.set_title(&format!("Zone Picker — {} — ↑/↓, Enter, Esc", disp));
                        return;
                    }
                    (Key::Named(NamedKey::Enter), _) | (_, PhysicalKey::Code(KeyCode::Enter)) => {
                        if let Some(slug) = self.picker.current_slug() {
                            #[cfg(target_arch = "wasm32")]
                            {
                                if let Ok(zp) =
                                    client_core::zone_client::ZonePresentation::load(&slug)
                                {
                                    let gz = render_wgpu::gfx::zone_batches::upload_zone_batches(
                                        state, &zp,
                                    );
                                    state.set_zone_batches(Some(gz));
                                    state.ensure_pc_assets();
                                    self.boot = BootMode::Running { slug: slug.clone() };
                                    window.set_title(&format!("RuinsofAtlantis — {}", slug));
                                } else {
                                    log::error!(
                                        "zone picker: failed to load zone '{}': snapshot missing or invalid",
                                        slug
                                    );
                                }
                            }
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                // Spawn background worker using channel; keep UI responsive
                                let (tx, rx) = mpsc::channel::<LoaderMsg>();
                                let slug_clone = slug.clone();
                                std::thread::spawn(move || {
                                    let send = |m| {
                                        let _ = tx.send(m);
                                    };
                                    send(LoaderMsg::Progress {
                                        what: "Reading zone".into(),
                                        step: 1,
                                        of: 3,
                                    });
                                    let zp = client_core::zone_client::ZonePresentation::load(
                                        &slug_clone,
                                    );
                                    let pc_cpu = if slug_clone == "campaign_builder"
                                        || slug_clone == "cc_demo"
                                    {
                                        None
                                    } else {
                                        use roa_assets::skinning::load_gltf_skinned;
                                        let ubc_path = std::path::Path::new(env!(
                                            "CARGO_MANIFEST_DIR"
                                        ))
                                        .join("../../assets/models/ubc/godot/Superhero_Male.gltf");
                                        load_gltf_skinned(&ubc_path).ok()
                                    };
                                    send(LoaderMsg::Progress {
                                        what: "Finalizing".into(),
                                        step: 3,
                                        of: 3,
                                    });
                                    match zp {
                                        Ok(zp_ok) => {
                                            let _ = tx.send(LoaderMsg::Done(Box::new(Ok((
                                                zp_ok, pc_cpu,
                                            )))));
                                        }
                                        Err(e) => {
                                            let _ = tx.send(LoaderMsg::Done(Box::new(Err(e))));
                                        }
                                    }
                                });
                                self.boot = BootMode::Loading {
                                    slug: slug.clone(),
                                    rx,
                                };
                                // Latch HUD-only path throughout Loading
                                state.set_picker_mode(true);
                                window.set_title(&format!("Loading — {}", slug));
                            }
                        }
                        return;
                    }
                    (Key::Named(NamedKey::Escape), _) | (_, PhysicalKey::Code(KeyCode::Escape)) => {
                        event_loop.exit();
                        return;
                    }
                    _ => {}
                }
            }
        }
        state.handle_window_event(&event);
        // Apply any pointer-lock request emitted by controller systems.
        if let Some(lock) = state.take_pointer_lock_request() {
            use winit::window::CursorGrabMode;
            // Try Locked first; if not supported, try Confined as a fallback.
            let mut applied_lock = false;
            if lock {
                match window.set_cursor_grab(CursorGrabMode::Locked) {
                    Ok(()) => {
                        applied_lock = true;
                        window.set_cursor_visible(false);
                        state.set_pointer_locked(true);
                    }
                    Err(e_locked) => {
                        log::debug!(
                            "pointer lock (Locked) failed: {:?}; trying Confined",
                            e_locked
                        );
                        match window.set_cursor_grab(CursorGrabMode::Confined) {
                            Ok(()) => {
                                applied_lock = true;
                                window.set_cursor_visible(false);
                                state.set_pointer_locked(true);
                            }
                            Err(e_confined) => {
                                log::debug!(
                                    "pointer grab (Confined) also failed: {:?}; falling back to free cursor",
                                    e_confined
                                );
                                window.set_cursor_visible(true);
                                state.set_pointer_locked(false);
                                state.set_mouselook(false);
                            }
                        }
                    }
                }
            }
            if !lock || !applied_lock {
                let _ = window.set_cursor_grab(CursorGrabMode::None);
                window.set_cursor_visible(true);
                if !applied_lock {
                    state.set_pointer_locked(false);
                }
            }
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size),
            WindowEvent::RedrawRequested => {
                // If we just transitioned to Running, release HUD-only latch right before rendering
                if let BootMode::Running { .. } = self.boot
                    && state.picker_mode()
                {
                    state.set_picker_mode(false);
                }
                // In Picker, draw overlay lines from platform before rendering.
                if let BootMode::Picker = self.boot {
                    let lines = self.picker.display_lines();
                    state.draw_picker_overlay(
                        "Choose a Zone",
                        "Use ↑/↓ to select   Enter to load   Esc to quit",
                        &lines,
                        self.picker.selected,
                    );
                }
                // Builder overlay when running the Campaign Builder zone
                if let BootMode::Running { slug } = &self.boot
                    && slug.as_str() == "campaign_builder"
                    && self.builder.active
                {
                    // Place the ghost ~10 ft (3.048 m) in front of the PLAYER facing
                    let pos = state.forward_point_from_player(3.048);
                    let yaw = self.builder.yaw_deg.rem_euclid(360.0).to_radians();
                    let (c, s) = (yaw.cos(), yaw.sin());
                    let model = [
                        [c, 0.0, s, 0.0],
                        [0.0, 1.0, 0.0, 0.0],
                        [-s, 0.0, c, 0.0],
                        [pos[0], pos[1], pos[2], 1.0],
                    ];
                    // Semi-transparent green ghost
                    state.set_ghost_transform(model, [0.2, 0.8, 0.3]);
                    // Ensure ghost mesh matches selected kind
                    let k = self
                        .builder
                        .kinds
                        .get(self.builder.kind_idx)
                        .cloned()
                        .unwrap_or_else(|| "tree.default".into());
                    let kk = if let Some(rest) = k.strip_prefix("tree.") {
                        rest.to_ascii_lowercase()
                    } else {
                        k.to_ascii_lowercase()
                    };
                    state.set_ghost_kind(&kk);
                    let mut lines: Vec<String> = Vec::new();
                    let util = (self.builder.ws.cap_utilization() * 100.0).round();
                    let cur_kind = self
                        .builder
                        .kinds
                        .get(self.builder.kind_idx)
                        .cloned()
                        .unwrap_or_else(|| "tree.default".into());
                    lines.push(format!(
                        "Kind: {}   Placed: {}   Yaw: {:.0}°   Cap: {:.0}%",
                        cur_kind,
                        self.builder.ws.placed.len(),
                        self.builder.yaw_deg,
                        util
                    ));
                    lines.push(
                        "Enter: place   ,/. rotate   X: export   I: import   Z: undo   B: exit"
                            .into(),
                    );
                    for (i, m) in self.builder.ws.placed.iter().enumerate().take(10) {
                        lines.push(format!(
                            "#{:02} {} [{:.1},{:.1},{:.1}] yaw={:.0}°",
                            i + 1,
                            m.kind,
                            m.pos[0],
                            m.pos[1],
                            m.pos[2],
                            m.yaw_deg
                        ));
                    }
                    state.draw_picker_overlay(
                        "Campaign Builder",
                        "B toggle   Enter place   ,/. rotate   I import   X export   Z undo",
                        &lines,
                        0,
                    );
                }
                if let Err(err) = state.render_with_window(window) {
                    match err {
                        SurfaceError::Lost | SurfaceError::Outdated => {
                            state.recreate_surface_current_size(window)
                        }
                        SurfaceError::OutOfMemory => event_loop.exit(),
                        e => eprintln!("render error: {e:?}"),
                    }
                }
                if let BootMode::Picker = self.boot
                    && let Some(slug) = state.take_picker_selected()
                {
                    // Clear any placement ghost when leaving builder overlay
                    state.set_ghost_instance(None);
                    // Spawn background loader so UI remains responsive
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let (tx, rx) = mpsc::channel::<LoaderMsg>();
                        let slug_clone = slug.clone();
                        std::thread::spawn(move || {
                            let send = |m| {
                                let _ = tx.send(m);
                            };
                            send(LoaderMsg::Progress {
                                what: "Reading zone".into(),
                                step: 1,
                                of: 3,
                            });
                            let zp = client_core::zone_client::ZonePresentation::load(&slug_clone);
                            let pc_cpu =
                                if slug_clone == "campaign_builder" || slug_clone == "cc_demo" {
                                    None
                                } else {
                                    use roa_assets::skinning::load_gltf_skinned;
                                    let ubc_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                                        .join("../../assets/models/ubc/godot/Superhero_Male.gltf");
                                    load_gltf_skinned(&ubc_path).ok()
                                };
                            send(LoaderMsg::Progress {
                                what: "Finalizing".into(),
                                step: 3,
                                of: 3,
                            });
                            match zp {
                                Ok(zp_ok) => {
                                    let _ = tx.send(LoaderMsg::Done(Box::new(Ok((zp_ok, pc_cpu)))));
                                }
                                Err(e) => {
                                    let _ = tx.send(LoaderMsg::Done(Box::new(Err(e))));
                                }
                            }
                        });
                        self.boot = BootMode::Loading {
                            slug: slug.clone(),
                            rx,
                        };
                        // Keep HUD-only path active until Running so the graph never executes mid-load
                        state.set_picker_mode(true);
                        window.set_title(&format!("Loading — {}", slug));
                    }
                    #[cfg(target_arch = "wasm32")]
                    if let Ok(zp) = client_core::zone_client::ZonePresentation::load(&slug) {
                        let gz = render_wgpu::gfx::zone_batches::upload_zone_batches(state, &zp);
                        state.set_zone_batches(Some(gz));
                        self.boot = BootMode::Running { slug: slug.clone() };
                        window.set_title(&format!("RuinsofAtlantis — {}", slug));
                        // Configure worldsmithing rules/caps from manifest (if any)
                        if slug.as_str() == "campaign_builder"
                            && let Ok(man) = data_runtime::zone::load_zone_manifest(&slug)
                        {
                            let mut rules = worldsmithing::Rules::default();
                            let mut caps = worldsmithing::Caps::default();
                            let mut kinds: Vec<String> = vec![];
                            if let Some(wsp) = man.worldsmithing {
                                if !wsp.kinds.is_empty() {
                                    for k in wsp.kinds {
                                        rules.allowed_kinds.insert(k.clone());
                                        kinds.push(k);
                                    }
                                } else {
                                    rules.allowed_kinds.insert("tree.default".into());
                                    kinds.push("tree.default".into());
                                }
                                if let Some(c) = wsp.caps {
                                    if let Some(t) = c.trees {
                                        caps.max_trees_per_zone = t;
                                    }
                                    if let Some(p) = c.place_per_second {
                                        caps.max_place_per_second = p;
                                    }
                                }
                            } else {
                                rules.allowed_kinds.insert("tree.default".into());
                            }
                            self.builder.ws = worldsmithing::Builder::new()
                                .caps(caps)
                                .rules(rules)
                                .build();
                            self.builder.kinds = kinds;
                            self.builder.kind_idx = 0;
                        }
                    } else {
                        log::error!("zone picker: failed to queue zone '{}' for loading", slug);
                    }
                }
            }
            // Builder key handling (Campaign Builder zone only)
            WindowEvent::KeyboardInput { event: kev, .. } => {
                if let BootMode::Running { slug } = &self.boot
                    && slug.as_str() == "campaign_builder"
                {
                    use winit::event::ElementState;
                    use winit::keyboard::KeyCode as KC;
                    let pressed = matches!(kev.state, ElementState::Pressed);
                    if let winit::keyboard::PhysicalKey::Code(code) = kev.physical_key {
                        match code {
                            KC::KeyB if pressed => {
                                self.builder.active = !self.builder.active;
                                self.builder.ws.set_active(self.builder.active);
                            }
                            KC::Enter | KC::NumpadEnter if pressed && self.builder.active => {
                                // Place ~10ft ahead of player facing
                                let pos = state.forward_point_from_player(3.048);
                                let yaw = self.builder.yaw_deg.rem_euclid(360.0);
                                let now_ms = {
                                    #[cfg(not(target_arch = "wasm32"))]
                                    {
                                        self.t0.elapsed().as_millis() as u64
                                    }
                                    #[cfg(target_arch = "wasm32")]
                                    {
                                        self.t0.elapsed().as_millis() as u64
                                    }
                                };
                                let k = self
                                    .builder
                                    .kinds
                                    .get(self.builder.kind_idx)
                                    .cloned()
                                    .unwrap_or_else(|| "tree.default".into());
                                log::info!(
                                    "builder: Enter pressed; kind='{}' pos=({:.2},{:.2},{:.2}) yaw={:.1}",
                                    k,
                                    pos[0],
                                    pos[1],
                                    pos[2],
                                    yaw
                                );
                                match self.builder.ws.place(&k, pos, yaw, now_ms) {
                                    Ok(_) => {
                                        // Immediately append to renderer session draws
                                        let yawr = yaw.to_radians();
                                        let (c, s) = (yawr.cos(), yawr.sin());
                                        let model = [
                                            [c, 0.0, s, 0.0],
                                            [0.0, 1.0, 0.0, 0.0],
                                            [-s, 0.0, c, 0.0],
                                            [pos[0], pos[1], pos[2], 1.0],
                                        ];
                                        // Map kind -> renderer key (strip optional "tree.")
                                        let kk = if let Some(rest) = k.strip_prefix("tree.") {
                                            rest.to_ascii_lowercase()
                                        } else {
                                            k.to_ascii_lowercase()
                                        };
                                        log::info!(
                                            "builder: placed '{}' (total={})",
                                            k,
                                            self.builder.ws.placed.len()
                                        );
                                        state.add_session_tree(&kk, model);
                                    }
                                    Err(e) => log::warn!("builder: place rejected: {e}"),
                                }
                            }
                            // Select kind with number keys 1..9
                            KC::Digit1 if pressed && self.builder.active => {
                                self.builder.kind_idx =
                                    0.min(self.builder.kinds.len().saturating_sub(1));
                            }
                            KC::Digit2 if pressed && self.builder.active => {
                                self.builder.kind_idx =
                                    1.min(self.builder.kinds.len().saturating_sub(1));
                            }
                            KC::Comma if pressed && self.builder.active => {
                                self.builder.yaw_deg =
                                    (self.builder.yaw_deg - 15.0).rem_euclid(360.0);
                                self.builder.ws.current_yaw_deg = self.builder.yaw_deg;
                            }
                            KC::Period if pressed && self.builder.active => {
                                self.builder.yaw_deg =
                                    (self.builder.yaw_deg + 15.0).rem_euclid(360.0);
                                self.builder.ws.current_yaw_deg = self.builder.yaw_deg;
                            }
                            KC::KeyZ if pressed && self.builder.active => {
                                let _ = self.builder.ws.undo_last();
                            }
                            KC::KeyX if pressed && self.builder.active => {
                                #[cfg(not(target_arch = "wasm32"))]
                                {
                                    let path = data_scene_path(slug);
                                    ensure_scene_parent(&path);
                                    let mut doc = load_scene(&path).unwrap_or(SceneDoc {
                                        version: "1.0.0".into(),
                                        seed: 0,
                                        layers: vec![],
                                        instances: vec![],
                                        logic: SceneLogic {
                                            triggers: vec![],
                                            spawns: vec![],
                                            waypoints: vec![],
                                            links: vec![],
                                        },
                                    });
                                    let r3 = |x: f32| (x * 1000.0).round() / 1000.0;
                                    doc.logic.spawns = self
                                        .builder
                                        .ws
                                        .placed
                                        .iter()
                                        .map(|p| SpawnMarker {
                                            id: p.id.clone(),
                                            kind: p.kind.clone(),
                                            pos: [r3(p.pos[0]), r3(p.pos[1]), r3(p.pos[2])],
                                            yaw_deg: r3(p.yaw_deg),
                                            tags: Vec::new(),
                                        })
                                        .collect();
                                    let _ = save_scene(&path, doc);
                                }
                            }
                            KC::KeyI if pressed && self.builder.active => {
                                #[cfg(not(target_arch = "wasm32"))]
                                {
                                    let path = data_scene_path(slug);
                                    if let Some(doc) = load_scene(&path) {
                                        self.builder.ws.placed = doc
                                            .logic
                                            .spawns
                                            .into_iter()
                                            .map(|m| worldsmithing::PlacedTreeV1 {
                                                id: m.id,
                                                kind: m.kind,
                                                pos: m.pos,
                                                yaw_deg: m.yaw_deg,
                                            })
                                            .collect();
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            #[cfg(feature = "demo_server")]
            self.pump_demo_server();

            // Poll background zone loader (if any) without blocking the UI thread.
            let mut restore: Option<BootMode> = None;
            let cur = std::mem::replace(&mut self.boot, BootMode::Picker);
            match cur {
                BootMode::Loading { slug, rx } => {
                    // Non-blocking pump of loader messages
                    let mut done: Option<
                        anyhow::Result<(
                            client_core::zone_client::ZonePresentation,
                            Option<roa_assets::types::SkinnedMeshCPU>,
                        )>,
                    > = None;
                    while let Ok(msg) = rx.try_recv() {
                        match msg {
                            LoaderMsg::Progress { what, step, of } => {
                                if let Some(st) = self.state.as_mut() {
                                    st.hud_reset();
                                    st.draw_picker_overlay(
                                        &format!("Loading — {}", slug),
                                        &format!("{}… ({}/{})", what, step, of),
                                        &[],
                                        0,
                                    );
                                }
                            }
                            LoaderMsg::Done(res) => done = Some(*res),
                        }
                    }
                    if let Some(res) = done.take() {
                        match res {
                            Ok((zp, pc_cpu_opt)) => {
                                if let Some(st) = self.state.as_mut() {
                                    // Attach zone quickly without heavy foliage on UI thread
                                    let gz = render_wgpu::gfx::zone_batches::upload_zone_batches(
                                        st, &zp,
                                    );
                                    st.set_zone_batches_defer_foliage(Some(gz));
                                    if let Some(cpu) = pc_cpu_opt {
                                        st.install_pc_cpu(cpu);
                                    } else {
                                        st.ensure_pc_assets();
                                    }
                                    st.hud_reset();
                                }
                                #[cfg(feature = "demo_server")]
                                if let Some(srv) = self.demo_server.as_mut() {
                                    let _ = srv.spawn_pc_at(glam::vec3(0.0, 0.6, 0.0));
                                    let _ = server_core::zones::boot_with_zone(srv, &slug);
                                }
                                self.boot = BootMode::Running { slug: slug.clone() };
                                if let Some(win) = &self.window {
                                    win.set_title(&format!("RuinsofAtlantis — {}", slug));
                                    win.request_redraw();
                                }
                                // Start CPU foliage worker and stream batches back (native only)
                                #[cfg(not(target_arch = "wasm32"))]
                                if let Some(st) = self.state.as_ref() {
                                    let (txa, rxa) = mpsc::channel::<AssetMsg>();
                                    self.asset_tx = Some(txa.clone());
                                    self.asset_rx = Some(rxa);
                                    let terr_cpu = st.terrain_cpu_clone();
                                    let slug_clone = slug.clone();
                                    std::thread::spawn(move || {
                                        use render_wgpu::gfx::foliage_stream::build_foliage_cpu_by_kind;
                                        if let Ok(batches) =
                                            build_foliage_cpu_by_kind(&slug_clone, &terr_cpu)
                                        {
                                            let total = batches.len() as u32;
                                            for (i, b) in batches.into_iter().enumerate() {
                                                let _ = txa.send(AssetMsg::Progress {
                                                    i: i as u32 + 1,
                                                    of: total.max(1),
                                                });
                                                let _ = txa.send(AssetMsg::Batch(b));
                                            }
                                        }
                                        let _ = txa.send(AssetMsg::Done);
                                    });
                                }
                            }
                            Err(e) => {
                                log::error!("zone load failed: {:?}", e);
                                if let Some(st) = self.state.as_mut() {
                                    st.hud_reset();
                                }
                                self.boot = BootMode::Picker;
                            }
                        }
                    } else {
                        // Still loading; keep mode and UI responsive
                        restore = Some(BootMode::Loading { slug, rx });
                        if let Some(win) = &self.window {
                            win.request_redraw();
                        }
                    }
                }
                other => {
                    // Not loading; restore the previous mode
                    restore = Some(other);
                }
            }
            if let Some(b) = restore.take() {
                self.boot = b;
            }
            // Pump background asset worker and upload a small slice per frame (native only)
            #[cfg(not(target_arch = "wasm32"))]
            if let (Some(rx), Some(st)) = (&self.asset_rx, &mut self.state) {
                let mut got_done = false;
                // Drain messages quickly, but only queue one batch for this frame
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        AssetMsg::Progress { i, of } => {
                            st.hud_reset();
                            st.draw_picker_overlay(
                                "Loading — Foliage",
                                &format!("Building foliage… ({}/{})", i, of),
                                &[],
                                0,
                            );
                        }
                        AssetMsg::Batch(batch) => {
                            self.pending_foliage_cpu.push(batch);
                            break;
                        }
                        AssetMsg::Done => got_done = true,
                    }
                }
                if let Some(b) = self.pending_foliage_cpu.pop() {
                    st.install_tree_cpu_batch(b);
                    // First content installed: ensure gameplay HUD path is active
                    if st.picker_mode() {
                        st.set_picker_mode(false);
                    }
                }
                if got_done {
                    st.hud_reset();
                    self.asset_tx = None;
                    self.asset_rx = None;
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            // If the async init finished, move Renderer into self.
            if self.window.is_none() || self.state.is_none() {
                RENDERER_CELL.with(|cell| {
                    if let Some((win, mut state)) = cell.borrow_mut().take() {
                        self.window = Some(win);
                        // Load Zone batches if a zone slug is provided (env/URL);
                        // otherwise, set a dummy batch so legacy static draws are suppressed
                        if let Some(slug) = detect_zone_slug() {
                            if let Ok(zp) = client_core::zone_client::ZonePresentation::load(&slug)
                            {
                                let gz = render_wgpu::gfx::zone_batches::upload_zone_batches(
                                    &state, &zp,
                                );
                                state.set_zone_batches(Some(gz));
                                if let Some(trees) = zp.trees.as_ref() {
                                    state.set_tree_instances(trees);
                                } else {
                                    state.clear_tree_instances();
                                }
                                // Ensure PC rig assets are available when skipping the Picker via URL
                                state.ensure_pc_assets();
                                self.boot = BootMode::Running { slug };
                            } else {
                                log::warn!("zone: failed to load snapshot for slug='{}'", slug);
                            }
                        } else {
                            let gb = render_wgpu::gfx::zone_batches::GpuZoneBatches {
                                slug: "<picker>".to_string(),
                            };
                            state.set_zone_batches(Some(gb));
                            // Populate the picker list on web as well so the overlay shows entries
                            self.picker.refresh();
                            self.boot = BootMode::Picker;
                            if let Some(w) = &self.window {
                                w.set_title("Zone Picker — no zone selected — ↑/↓, Enter, Esc");
                            }
                        }
                        self.state = Some(state);
                        // Wire loopback transport and seed demo server on wasm when enabled
                        #[cfg(feature = "demo_server")]
                        {
                            let (srv, cli) = net_core::transport::LocalLoopbackTransport::new(4096);
                            let (tx_cli, rx_cli) = cli.split();
                            if let Some(st) = self.state.as_mut() {
                                st.set_replication_rx(rx_cli);
                                st.set_command_tx(tx_cli);
                            }
                            self.transport_srv = Some(srv);
                            // Spawn demo server content similar to native path
                            let mut srv = server_core::ServerState::new();
                            let wiz_now = self
                                .state
                                .as_ref()
                                .map(|s| s.wizard_positions())
                                .unwrap_or_default();
                            let pc0 = wiz_now
                                .first()
                                .copied()
                                .unwrap_or(glam::vec3(0.0, 0.6, 0.0));
                            if srv.pc_actor.is_none() {
                                let _ = srv.spawn_pc_at(pc0);
                            }
                            // Only spawn encounter actors when a zone is explicitly selected,
                            // and skip them for cc_demo. When no zone is selected (Picker), spawn none.
                            let z = detect_zone_slug();
                            if let Some(slug) = z {
                                let _ = server_core::zones::boot_with_zone(&mut srv, slug.as_str());
                            }
                            self.demo_server = Some(srv);
                        }
                    }
                });
            }
        }
        // Emit replicated NPC/Boss each frame and step demo server (demo only)
        #[cfg(feature = "demo_server")]
        if let (Some(srv_xport), Some(s)) = (&self.transport_srv, &mut self.state) {
            // Step server; drain client->server commands before stepping
            #[cfg(feature = "demo_server")]
            if let Some(srv) = &mut self.demo_server {
                // Drain any client commands (projectiles, etc.)
                while let Some(bytes) = srv_xport.try_recv() {
                    let payload = match net_core::frame::read_msg(&bytes) {
                        Ok(p) => p,
                        Err(_) => &bytes,
                    };
                    let mut slice: &[u8] = payload;
                    if let Ok(cmd) = net_core::command::ClientCmd::decode(&mut slice) {
                        // Rate limit only spell-cast commands; Move/Aim are intents (state).
                        let rate_limited = matches!(
                            cmd,
                            net_core::command::ClientCmd::FireBolt { .. }
                                | net_core::command::ClientCmd::Fireball { .. }
                                | net_core::command::ClientCmd::MagicMissile { .. }
                        );
                        if rate_limited {
                            let now = {
                                #[cfg(not(target_arch = "wasm32"))]
                                {
                                    std::time::Instant::now()
                                }
                                #[cfg(target_arch = "wasm32")]
                                {
                                    web_time::Instant::now()
                                }
                            };
                            if now.duration_since(self.last_sec_start).as_secs_f32() >= 1.0 {
                                self.last_sec_start = now;
                                self.cmds_this_sec = 0;
                            }
                            if self.cmds_this_sec >= 20 {
                                log::debug!("rate-limit: dropping spell cmd");
                                continue;
                            }
                            self.cmds_this_sec += 1;
                        }
                        match cmd {
                            net_core::command::ClientCmd::FireBolt { pos, dir } => {
                                let p = glam::vec3(pos[0], pos[1], pos[2]);
                                let d = glam::vec3(dir[0], dir[1], dir[2]).normalize_or_zero();
                                log::info!(
                                    "cmd: FireBolt at ({:.2},{:.2},{:.2}) dir=({:.2},{:.2},{:.2})",
                                    p.x,
                                    p.y,
                                    p.z,
                                    d.x,
                                    d.y,
                                    d.z
                                );
                                srv.enqueue_cast(p, d, server_core::SpellId::Firebolt);
                            }
                            net_core::command::ClientCmd::Fireball { pos, dir } => {
                                let p = glam::vec3(pos[0], pos[1], pos[2]);
                                let d = glam::vec3(dir[0], dir[1], dir[2]).normalize_or_zero();
                                log::info!(
                                    "cmd: Fireball at ({:.2},{:.2},{:.2}) dir=({:.2},{:.2},{:.2})",
                                    p.x,
                                    p.y,
                                    p.z,
                                    d.x,
                                    d.y,
                                    d.z
                                );
                                srv.enqueue_cast(p, d, server_core::SpellId::Fireball);
                            }
                            net_core::command::ClientCmd::MagicMissile { pos, dir } => {
                                let p = glam::vec3(pos[0], pos[1], pos[2]);
                                let d = glam::vec3(dir[0], dir[1], dir[2]).normalize_or_zero();
                                log::info!(
                                    "cmd: MagicMissile at ({:.2},{:.2},{:.2}) dir=({:.2},{:.2},{:.2})",
                                    p.x,
                                    p.y,
                                    p.z,
                                    d.x,
                                    d.y,
                                    d.z
                                );
                                srv.enqueue_cast(p, d, server_core::SpellId::MagicMissile);
                            }
                            net_core::command::ClientCmd::Move { dx, dz, run } => {
                                let runb = run != 0;
                                srv.apply_move_intent(dx, dz, runb);
                            }
                            net_core::command::ClientCmd::Aim { yaw } => {
                                srv.apply_aim_intent(yaw);
                            }
                        }
                    }
                }
                // dt
                let dt = if let Some(t0) = self.last_time.take() {
                    let now = {
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            std::time::Instant::now()
                        }
                        #[cfg(target_arch = "wasm32")]
                        {
                            web_time::Instant::now()
                        }
                    };
                    let d = (now - t0).as_secs_f32();
                    self.last_time = Some(now);
                    d.clamp(0.0, 0.1)
                } else {
                    1.0 / 60.0
                };
                // wizard positions from renderer
                let wiz_pos: Vec<glam::Vec3> = s.wizard_positions();
                // Step authoritative server first so replication reflects the latest state
                srv.step_authoritative(dt);
                // Build and send replication messages AFTER stepping
                if std::env::var("RA_LOG_DEMO")
                    .map(|v| v == "1")
                    .unwrap_or(false)
                {
                    let actors = srv.ecs.len();
                    log::info!(
                        "demo_server: stepping dt={:.3}s; actors={} wizards={}",
                        dt,
                        actors,
                        wiz_pos.len()
                    );
                } else {
                    let actors = srv.ecs.len();
                    log::debug!(
                        "demo_server: stepping dt={:.3}s; actors={} wizards={} ",
                        dt,
                        actors,
                        wiz_pos.len()
                    );
                }
                let tick64 = self.tick as u64;
                // Always build v3 deltas with interest limiting and send after stepping
                let asnap = srv.tick_snapshot_actors(tick64);
                // Interest center: authoritative PC position from server when available
                let center = if let Some(pc_id) = srv.pc_actor
                    && let Some(pc) = srv.ecs.get(pc_id)
                {
                    pc.tr.pos
                } else {
                    s.wizard_positions()
                        .first()
                        .copied()
                        .unwrap_or(glam::vec3(0.0, 0.0, 0.0))
                };
                let r2 = self.interest_radius_m * self.interest_radius_m;
                let mut cur: std::collections::HashMap<u32, net_core::snapshot::ActorRep> =
                    std::collections::HashMap::new();
                for a in asnap.actors {
                    let dx = a.pos[0] - center.x;
                    let dz = a.pos[2] - center.z;
                    if dx * dx + dz * dz <= r2 {
                        cur.insert(a.id, a);
                    }
                }
                // spawns/removals/updates
                let mut spawns = Vec::new();
                let mut removals = Vec::new();
                let mut updates = Vec::new();
                for (id, a) in &cur {
                    if let Some(b) = self.baseline.get(id) {
                        let mut flags = 0u8;
                        let mut rec = net_core::snapshot::ActorDeltaRec {
                            id: *id,
                            flags: 0,
                            qpos: [0; 3],
                            qyaw: 0,
                            hp: 0,
                            alive: 0,
                        };
                        let qpx = net_core::snapshot::qpos(a.pos[0]);
                        let qpy = net_core::snapshot::qpos(a.pos[1]);
                        let qpz = net_core::snapshot::qpos(a.pos[2]);
                        if net_core::snapshot::qpos(b.pos[0]) != qpx
                            || net_core::snapshot::qpos(b.pos[1]) != qpy
                            || net_core::snapshot::qpos(b.pos[2]) != qpz
                        {
                            flags |= 1;
                            rec.qpos = [qpx, qpy, qpz];
                        }
                        let qy = net_core::snapshot::qyaw(a.yaw);
                        if net_core::snapshot::qyaw(b.yaw) != qy {
                            flags |= 2;
                            rec.qyaw = qy;
                        }
                        if b.hp != a.hp {
                            flags |= 4;
                            rec.hp = a.hp;
                        }
                        if b.alive != a.alive {
                            flags |= 8;
                            rec.alive = u8::from(a.alive);
                        }
                        if flags != 0 {
                            rec.flags = flags;
                            updates.push(rec);
                        }
                    } else {
                        spawns.push(a.clone());
                    }
                }
                for id in self.baseline.keys() {
                    if !cur.contains_key(id) {
                        removals.push(*id);
                    }
                }
                // Projectiles: interest-limited to same center/radius
                let mut projectiles = Vec::new();
                for c in srv.ecs.iter() {
                    if let (Some(proj), Some(vel)) = (c.projectile.as_ref(), c.velocity.as_ref()) {
                        let dx = c.tr.pos.x - center.x;
                        let dz = c.tr.pos.z - center.z;
                        if dx * dx + dz * dz <= r2 {
                            projectiles.push(net_core::snapshot::ProjectileRep {
                                id: c.id.0,
                                kind: match proj.kind {
                                    server_core::ProjKind::Firebolt => 0,
                                    server_core::ProjKind::Fireball => 1,
                                    server_core::ProjKind::MagicMissile => 2,
                                },
                                pos: [c.tr.pos.x, c.tr.pos.y, c.tr.pos.z],
                                vel: [vel.v.x, vel.v.y, vel.v.z],
                            });
                        }
                    }
                }
                let delta = net_core::snapshot::ActorSnapshotDelta {
                    v: 4,
                    tick: tick64,
                    baseline: self.baseline_tick,
                    spawns,
                    updates,
                    removals,
                    projectiles,
                    hits: {
                        let mut v = Vec::new();
                        // drain server-side hitfx for this frame
                        std::mem::swap(&mut v, &mut srv.fx_hits);
                        v
                    },
                };
                let mut p4 = Vec::new();
                delta.encode(&mut p4);
                let mut f4 = Vec::with_capacity(p4.len() + 8);
                net_core::frame::write_msg(&mut f4, &p4);
                metrics::counter!("net.bytes_sent_total", "dir" => "tx").increment(f4.len() as u64);
                let _ = srv_xport.try_send(f4);
                // update baseline
                self.baseline = cur;
                self.baseline_tick = tick64;
                // Send HUD status for local PC
                if let Some(pc_id) = srv.pc_actor
                    && let Some(pc) = srv.ecs.get(pc_id)
                {
                    let mana = pc
                        .pool
                        .as_ref()
                        .map(|p| p.mana)
                        .unwrap_or(0)
                        .clamp(0, u16::MAX as i32) as u16;
                    let mana_max = pc
                        .pool
                        .as_ref()
                        .map(|p| p.max)
                        .unwrap_or(0)
                        .clamp(0, u16::MAX as i32) as u16;
                    let gcd_ms = (pc.cooldowns.as_ref().map(|c| c.gcd_ready).unwrap_or(0.0)
                        * 1000.0)
                        .clamp(0.0, u16::MAX as f32) as u16;
                    let cd = |sid: server_core::SpellId| -> f32 {
                        pc.cooldowns
                            .as_ref()
                            .and_then(|c| c.per_spell.get(&sid).copied())
                            .unwrap_or(0.0)
                    };
                    let spell_cds = vec![
                        (0u8, (cd(server_core::SpellId::Firebolt) * 1000.0) as u16),
                        (1u8, (cd(server_core::SpellId::Fireball) * 1000.0) as u16),
                        (
                            2u8,
                            (cd(server_core::SpellId::MagicMissile) * 1000.0) as u16,
                        ),
                    ];
                    let burning_ms =
                        (pc.burning.as_ref().map(|b| b.remaining_s).unwrap_or(0.0) * 1000.0) as u16;
                    let slow_ms =
                        (pc.slow.as_ref().map(|s| s.remaining_s).unwrap_or(0.0) * 1000.0) as u16;
                    let stunned_ms =
                        (pc.stunned.as_ref().map(|s| s.remaining_s).unwrap_or(0.0) * 1000.0) as u16;
                    let hud = net_core::snapshot::HudStatusMsg {
                        v: net_core::snapshot::HUD_STATUS_VERSION,
                        mana,
                        mana_max,
                        gcd_ms,
                        spell_cds,
                        burning_ms,
                        slow_ms,
                        stunned_ms,
                    };
                    let mut hb = Vec::new();
                    hud.encode(&mut hb);
                    let mut fh = Vec::with_capacity(hb.len() + 8);
                    net_core::frame::write_msg(&mut fh, &hb);
                    metrics::counter!("net.bytes_sent_total", "dir" => "tx")
                        .increment(fh.len() as u64);
                    let _ = srv_xport.try_send(fh);
                }
                // Drain HUD toasts and send messages
                while let Some(code) = srv.hud_toasts.pop() {
                    let toast = net_core::snapshot::HudToastMsg {
                        v: net_core::snapshot::HUD_TOAST_VERSION,
                        code,
                    };
                    let mut tb = Vec::new();
                    toast.encode(&mut tb);
                    let mut ft = Vec::with_capacity(tb.len() + 8);
                    net_core::frame::write_msg(&mut ft, &tb);
                    metrics::counter!("net.bytes_sent_total", "dir" => "tx")
                        .increment(ft.len() as u64);
                    let _ = srv_xport.try_send(ft);
                }
                // Destructible replication: send instances once, deltas per change
                if srv.destruct_bootstrap_instances_outstanding {
                    let insts = srv.all_destructible_instances();
                    for d in insts {
                        let mut buf = Vec::new();
                        d.encode(&mut buf);
                        let mut framed = Vec::with_capacity(buf.len() + 8);
                        net_core::frame::write_msg(&mut framed, &buf);
                        metrics::counter!("net.bytes_sent_total", "dir" => "tx")
                            .increment(framed.len() as u64);
                        let _ = srv_xport.try_send(framed);
                        self.sent_destr_instances.insert(d.did);
                    }
                    srv.destruct_bootstrap_instances_outstanding = false;
                }
                // Interest-cull destructible deltas using planar distance to instance AABB
                // Build a quick DID -> (min,max) map
                let mut inst_map: std::collections::HashMap<u64, (glam::Vec3, glam::Vec3)> =
                    std::collections::HashMap::new();
                for d in &srv.destruct_instances {
                    inst_map.insert(
                        d.did,
                        (glam::Vec3::from(d.world_min), glam::Vec3::from(d.world_max)),
                    );
                }
                // Interest center: same as actor interest center (PC)
                let center = s
                    .wizard_positions()
                    .first()
                    .copied()
                    .unwrap_or(glam::vec3(0.0, 0.0, 0.0));
                let r2 = self.interest_radius_m * self.interest_radius_m;
                for delta in srv.drain_destruct_mesh_deltas() {
                    if !self.sent_destr_instances.contains(&delta.did) {
                        continue; // ensure instance precedes deltas
                    }
                    // Planar AABB vs circle test for interest culling
                    if let Some((min, max)) = inst_map.get(&delta.did).copied() {
                        // closest XY in XZ-plane
                        let cx = center.x.clamp(min.x, max.x);
                        let cz = center.z.clamp(min.z, max.z);
                        let dx = cx - center.x;
                        let dz = cz - center.z;
                        if dx * dx + dz * dz > r2 {
                            continue;
                        }
                    }
                    let mut buf = Vec::new();
                    delta.encode(&mut buf);
                    let mut framed = Vec::with_capacity(buf.len() + 8);
                    net_core::frame::write_msg(&mut framed, &buf);
                    metrics::counter!("net.bytes_sent_total", "dir" => "tx")
                        .increment(framed.len() as u64);
                    let _ = srv_xport.try_send(framed);
                }
                self.tick = self.tick.wrapping_add(1);
            }
        }
        if let Some(win) = &self.window {
            win.request_redraw();
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        let Some(state) = &mut self.state else {
            return;
        };
        if let winit::event::DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            // Forward relative motion to the renderer. It decides whether to apply
            // based on pointer-lock and controller mode.
            state.handle_mouse_motion(dx as f32, dy as f32);
        }
    }
}

// Thread-local handoff for async renderer initialization on wasm.
#[cfg(target_arch = "wasm32")]
thread_local! {
    static RENDERER_CELL: std::cell::RefCell<Option<(Window, Renderer)>> = std::cell::RefCell::new(None);
}

fn is_headless() -> bool {
    if std::env::var("RA_HEADLESS")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        return true;
    }
    if std::env::var("CI")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false)
    {
        return true;
    }
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd",
    ))]
    {
        if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
            return true;
        }
    }
    false
}

pub fn run() -> anyhow::Result<()> {
    if is_headless() {
        return Ok(());
    }
    let event_loop = EventLoop::new()?;
    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}

/// Detect selected zone slug from environment (native) or query string (web).
fn detect_zone_slug() -> Option<String> {
    // Prefer explicit env var in both native/web builds if set by the harness.
    if let Ok(v) = std::env::var("ROA_ZONE")
        && !v.is_empty()
    {
        return Some(v);
    }
    // WASM: parse ?zone=<slug> from the URL (manual parser; avoids extra web-sys features).
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(win) = web_sys::window() {
            if let Ok(search) = win.location().search() {
                let s = search.trim_start_matches('?');
                for pair in s.split('&') {
                    let mut it = pair.splitn(2, '=');
                    if let (Some(k), Some(v)) = (it.next(), it.next()) {
                        if k == "zone" && !v.is_empty() {
                            // Slugs are plain ASCII; keep as-is
                            return Some(v.to_string());
                        }
                    }
                }
            }
        }
        // Build-time default: if set, fall back to ROA_ZONE_DEFAULT compiled into the Wasm.
        // This lets versioned snapshots boot a specific zone without needing a query param.
        if let Some(def) = option_env!("ROA_ZONE_DEFAULT") {
            if !def.is_empty() {
                return Some(def.to_string());
            }
        }
    }
    // No back-compat for RA_ZONE
    None
}

#[cfg(all(feature = "demo_server", not(target_arch = "wasm32")))]
impl App {
    fn pump_demo_server(&mut self) {
        let (Some(srv_xport), Some(state)) = (&self.transport_srv, &mut self.state) else {
            return;
        };
        if let Some(srv) = &mut self.demo_server {
            while let Some(bytes) = srv_xport.try_recv() {
                let payload = match net_core::frame::read_msg(&bytes) {
                    Ok(p) => p,
                    Err(_) => &bytes,
                };
                let mut slice: &[u8] = payload;
                if let Ok(cmd) = net_core::command::ClientCmd::decode(&mut slice) {
                    let rate_limited = matches!(
                        cmd,
                        net_core::command::ClientCmd::FireBolt { .. }
                            | net_core::command::ClientCmd::Fireball { .. }
                            | net_core::command::ClientCmd::MagicMissile { .. }
                    );
                    if rate_limited {
                        let now = {
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                std::time::Instant::now()
                            }
                            #[cfg(target_arch = "wasm32")]
                            {
                                web_time::Instant::now()
                            }
                        };
                        if now.duration_since(self.last_sec_start).as_secs_f32() >= 1.0 {
                            self.last_sec_start = now;
                            self.cmds_this_sec = 0;
                        }
                        if self.cmds_this_sec >= 20 {
                            continue;
                        }
                        self.cmds_this_sec += 1;
                    }
                    match cmd {
                        net_core::command::ClientCmd::FireBolt { pos, dir } => {
                            let p = glam::vec3(pos[0], pos[1], pos[2]);
                            let d = glam::vec3(dir[0], dir[1], dir[2]).normalize_or_zero();
                            srv.enqueue_cast(p, d, server_core::SpellId::Firebolt);
                        }
                        net_core::command::ClientCmd::Fireball { pos, dir } => {
                            let p = glam::vec3(pos[0], pos[1], pos[2]);
                            let d = glam::vec3(dir[0], dir[1], dir[2]).normalize_or_zero();
                            srv.enqueue_cast(p, d, server_core::SpellId::Fireball);
                        }
                        net_core::command::ClientCmd::MagicMissile { pos, dir } => {
                            let p = glam::vec3(pos[0], pos[1], pos[2]);
                            let d = glam::vec3(dir[0], dir[1], dir[2]).normalize_or_zero();
                            srv.enqueue_cast(p, d, server_core::SpellId::MagicMissile);
                        }
                        net_core::command::ClientCmd::Move { dx, dz, run } => {
                            srv.apply_move_intent(dx, dz, run != 0);
                        }
                        net_core::command::ClientCmd::Aim { yaw } => {
                            srv.apply_aim_intent(yaw);
                        }
                    }
                }
            }
            let dt = if let Some(t0) = self.last_time.take() {
                let now = {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        std::time::Instant::now()
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        web_time::Instant::now()
                    }
                };
                let d = now.duration_since(t0).as_secs_f32();
                self.last_time = Some(now);
                d.clamp(0.0, 0.1)
            } else {
                1.0 / 60.0
            };
            let _wiz_pos: Vec<glam::Vec3> = state.wizard_positions();
            srv.step_authoritative(dt);
            let tick64 = self.tick as u64;
            let asnap = srv.tick_snapshot_actors(tick64);
            let center = if let Some(pc_id) = srv.pc_actor
                && let Some(pc) = srv.ecs.get(pc_id)
            {
                pc.tr.pos
            } else {
                state
                    .wizard_positions()
                    .first()
                    .copied()
                    .unwrap_or(glam::vec3(0.0, 0.0, 0.0))
            };
            let r2 = self.interest_radius_m * self.interest_radius_m;
            let mut cur = std::collections::HashMap::new();
            for a in asnap.actors {
                let dx = a.pos[0] - center.x;
                let dz = a.pos[2] - center.z;
                if dx * dx + dz * dz <= r2 {
                    cur.insert(a.id, a);
                }
            }
            let mut spawns = Vec::new();
            let mut removals = Vec::new();
            let mut updates = Vec::new();
            for (id, a) in &cur {
                if let Some(b) = self.baseline.get(id) {
                    let mut flags = 0u8;
                    let mut rec = net_core::snapshot::ActorDeltaRec {
                        id: *id,
                        flags: 0,
                        qpos: [0; 3],
                        qyaw: 0,
                        hp: 0,
                        alive: 0,
                    };
                    let qpx = net_core::snapshot::qpos(a.pos[0]);
                    let qpy = net_core::snapshot::qpos(a.pos[1]);
                    let qpz = net_core::snapshot::qpos(a.pos[2]);
                    if net_core::snapshot::qpos(b.pos[0]) != qpx
                        || net_core::snapshot::qpos(b.pos[1]) != qpy
                        || net_core::snapshot::qpos(b.pos[2]) != qpz
                    {
                        flags |= 1;
                        rec.qpos = [qpx, qpy, qpz];
                    }
                    let qy = net_core::snapshot::qyaw(a.yaw);
                    if net_core::snapshot::qyaw(b.yaw) != qy {
                        flags |= 2;
                        rec.qyaw = qy;
                    }
                    if b.hp != a.hp {
                        flags |= 4;
                        rec.hp = a.hp;
                    }
                    if b.alive != a.alive {
                        flags |= 8;
                        rec.alive = u8::from(a.alive);
                    }
                    if flags != 0 {
                        rec.flags = flags;
                        updates.push(rec);
                    }
                } else {
                    spawns.push(a.clone());
                }
            }
            for id in self.baseline.keys() {
                if !cur.contains_key(id) {
                    removals.push(*id);
                }
            }
            let mut projectiles = Vec::new();
            for c in srv.ecs.iter() {
                if let (Some(proj), Some(vel)) = (c.projectile.as_ref(), c.velocity.as_ref()) {
                    let dx = c.tr.pos.x - center.x;
                    let dz = c.tr.pos.z - center.z;
                    if dx * dx + dz * dz <= r2 {
                        projectiles.push(net_core::snapshot::ProjectileRep {
                            id: c.id.0,
                            kind: match proj.kind {
                                server_core::ProjKind::Firebolt => 0,
                                server_core::ProjKind::Fireball => 1,
                                server_core::ProjKind::MagicMissile => 2,
                            },
                            pos: [c.tr.pos.x, c.tr.pos.y, c.tr.pos.z],
                            vel: [vel.v.x, vel.v.y, vel.v.z],
                        });
                    }
                }
            }
            let delta = net_core::snapshot::ActorSnapshotDelta {
                v: 4,
                tick: tick64,
                baseline: self.baseline_tick,
                spawns,
                updates,
                removals,
                projectiles,
                hits: {
                    let mut v = Vec::new();
                    std::mem::swap(&mut v, &mut srv.fx_hits);
                    v
                },
            };
            let mut p4 = Vec::new();
            delta.encode(&mut p4);
            let mut f4 = Vec::with_capacity(p4.len() + 8);
            net_core::frame::write_msg(&mut f4, &p4);
            let _ = srv_xport.try_send(f4);
            self.baseline = cur;
            self.baseline_tick = tick64;
            self.tick = self.tick.wrapping_add(1);
            if let Some(win) = &self.window {
                win.request_redraw();
            }
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod asset_stream_tests {
    use super::*;
    use std::sync::mpsc;

    use render_wgpu::gfx::foliage_stream::TreeCpuBatch;

    /// Minimal stand‑in for the renderer pieces we touch during streaming.
    struct StubRenderer {
        installs: usize,
        picker: bool,
        hud_draws: usize,
    }

    impl StubRenderer {
        fn new() -> Self {
            Self {
                installs: 0,
                picker: true,
                hud_draws: 0,
            }
        }
        fn install_tree_cpu_batch(&mut self, _b: TreeCpuBatch) {
            self.installs += 1;
        }
        fn picker_mode(&self) -> bool {
            self.picker
        }
        fn set_picker_mode(&mut self, on: bool) {
            self.picker = on;
        }
        fn hud_reset(&mut self) {}
        fn draw_picker_overlay(
            &mut self,
            _title: &str,
            _sub: &str,
            _lines: &[String],
            _sel: usize,
        ) {
            self.hud_draws += 1;
        }
    }

    /// Mirror the asset-pump section of `about_to_wait`, but on our stub.
    fn pump_once(
        st: &mut StubRenderer,
        rx: &mpsc::Receiver<AssetMsg>,
        pending: &mut Vec<TreeCpuBatch>,
    ) -> bool {
        let mut got_done = false;
        // Drain messages without blocking; queue exactly one batch per frame
        while let Ok(msg) = rx.try_recv() {
            match msg {
                AssetMsg::Progress { .. } => {
                    st.hud_reset();
                    st.draw_picker_overlay("Loading — Foliage", "Building foliage…", &[], 0);
                }
                AssetMsg::Batch(batch) => {
                    pending.push(batch);
                    break; // budget: at most one upload per frame
                }
                AssetMsg::Done => got_done = true,
            }
        }
        if let Some(b) = pending.pop() {
            st.install_tree_cpu_batch(b);
            // Release HUD latch after first content arrives
            if st.picker_mode() {
                st.set_picker_mode(false);
            }
        }
        got_done
    }

    fn make_batch(kind: &str) -> TreeCpuBatch {
        TreeCpuBatch {
            kind: kind.to_string(),
            instances: Vec::new(),
            verts_uv: Vec::new(),
            indices_u16: Vec::new(),
            base_tex_rgba8: None,
        }
    }

    #[test]
    fn streams_one_batch_per_tick_and_releases_picker() {
        let (tx, rx) = mpsc::channel::<AssetMsg>();
        // Simulate worker: progress spam + three batches + done
        tx.send(AssetMsg::Progress { i: 1, of: 3 }).unwrap();
        tx.send(AssetMsg::Batch(make_batch("birch"))).unwrap();
        tx.send(AssetMsg::Progress { i: 2, of: 3 }).unwrap();
        tx.send(AssetMsg::Batch(make_batch("pine"))).unwrap();
        tx.send(AssetMsg::Batch(make_batch("giantpine"))).unwrap();
        tx.send(AssetMsg::Done).unwrap();

        let mut st = StubRenderer::new();
        let mut pending = Vec::new();

        // First pump installs exactly one batch and drops latch
        let done1 = pump_once(&mut st, &rx, &mut pending);
        assert_eq!(st.installs, 1, "exactly one batch per tick");
        assert!(
            !st.picker_mode(),
            "picker/HUD latch must be released after first batch"
        );
        assert!(!done1, "not done yet");

        // Second pump installs second batch
        let done2 = pump_once(&mut st, &rx, &mut pending);
        assert_eq!(st.installs, 2);
        assert!(!done2);

        // Third pump installs third batch; there is also a trailing Done message
        let done3 = pump_once(&mut st, &rx, &mut pending);
        assert_eq!(st.installs, 3);
        // One more pump will observe the queued Done (if not already flushed)
        let done4 = pump_once(&mut st, &rx, &mut pending);
        assert!(done3 || done4, "eventually reports done");
    }

    #[test]
    fn progress_messages_do_not_trigger_uploads() {
        let (tx, rx) = mpsc::channel::<AssetMsg>();
        for i in 0..10 {
            tx.send(AssetMsg::Progress { i, of: 10 }).unwrap();
        }
        let mut st = StubRenderer::new();
        let mut pending = Vec::new();
        let done = pump_once(&mut st, &rx, &mut pending);
        assert_eq!(st.installs, 0, "progress must not cause uploads");
        assert!(
            st.picker_mode(),
            "no batch yet -> still latched in HUD-only"
        );
        assert!(!done);
    }
}
