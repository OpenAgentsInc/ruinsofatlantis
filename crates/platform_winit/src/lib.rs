//! platform_winit: window + input + present loop (winit 0.30).
//!
//! Provides a minimal `run()` that creates a window and drives the
//! `render_wgpu::gfx::Renderer` via winit's ApplicationHandler API.

use net_core::snapshot::{SnapshotDecode, SnapshotEncode};
use net_core::transport::Transport;
use render_wgpu::gfx::Renderer;
use wgpu::SurfaceError;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes},
};

#[allow(dead_code)]
enum BootMode {
    Picker,
    Running { slug: String },
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
        if let Ok(reg) = data_runtime::zone_snapshot::ZoneRegistry::discover(&root) {
            let mut next: Vec<ZoneEntry> = Vec::new();
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
            next.sort_by(|a, b| a.display.to_lowercase().cmp(&b.display.to_lowercase()));
            self.items = next;
            self.selected = 0;
        }
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
            cmds_this_sec: 0,
            sent_destr_instances: std::collections::HashSet::new(),
            boot: BootMode::Picker,
            picker: Default::default(),
        }
    }
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
                    self.picker.refresh();
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
                    // Only spawn encounter actors when running a zone
                    if let BootMode::Running { .. } = self.boot {
                        srv.ring_spawn(8, 15.0, 20);
                        srv.ring_spawn(12, 30.0, 25);
                        srv.ring_spawn(15, 45.0, 30);
                        let wiz_count = 4usize;
                        let wiz_r = 8.0f32;
                        for i in 0..wiz_count {
                            let a = (i as f32) / (wiz_count as f32) * std::f32::consts::TAU;
                            let p = glam::vec3(wiz_r * a.cos(), 0.6, wiz_r * a.sin());
                            let _ = srv.spawn_wizard_npc(p);
                        }
                        let _ = srv.spawn_nivita_unique(glam::vec3(0.0, 0.6, 0.0));
                        let _dk = srv.spawn_death_knight(glam::vec3(60.0, 0.6, 0.0));
                        server_core::scene_build::add_demo_ruins_destructible(&mut srv);
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
        state.handle_window_event(&event);
        // Apply any pointer-lock request emitted by controller systems.
        if let Some(lock) = state.take_pointer_lock_request() {
            use winit::window::CursorGrabMode;
            let grab_mode = if lock {
                CursorGrabMode::Locked
            } else {
                CursorGrabMode::None
            };
            match window.set_cursor_grab(grab_mode) {
                Ok(()) => {
                    window.set_cursor_visible(!lock);
                    state.set_pointer_locked(lock);
                }
                Err(e) => {
                    // If locking failed (e.g., WASM denied), fall back to cursor mode
                    log::debug!("pointer lock request failed: {:?}", e);
                    window.set_cursor_visible(true);
                    state.set_pointer_locked(false);
                    state.set_mouselook(false);
                }
            }
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size),
            WindowEvent::RedrawRequested => {
                if let Err(err) = state.render() {
                    match err {
                        SurfaceError::Lost | SurfaceError::Outdated => {
                            state.recreate_surface_current_size(window)
                        }
                        SurfaceError::OutOfMemory => event_loop.exit(),
                        e => eprintln!("render error: {e:?}"),
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        #[cfg(target_arch = "wasm32")]
        {
            // If the async init finished, move Renderer into self.
            if self.window.is_none() || self.state.is_none() {
                RENDERER_CELL.with(|cell| {
                    if let Some((win, mut state)) = cell.borrow_mut().take() {
                        self.window = Some(win);
                        // Load Zone batches if a zone slug is provided (env/URL)
                        if let Some(slug) = detect_zone_slug() {
                            if let Ok(zp) = client_core::zone_client::ZonePresentation::load(&slug)
                            {
                                let gz = render_wgpu::gfx::zone_batches::upload_zone_batches(
                                    &state, &zp,
                                );
                                state.set_zone_batches(Some(gz));
                            } else {
                                log::warn!("zone: failed to load snapshot for slug='{}'", slug);
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
                            // If a Zone is selected and it's a minimal controller demo,
                            // do not spawn encounter actors. Otherwise, spawn demo content.
                            let z = detect_zone_slug();
                            if z.as_deref() != Some("cc_demo") {
                                srv.ring_spawn(8, 15.0, 20);
                                srv.ring_spawn(12, 30.0, 25);
                                srv.ring_spawn(15, 45.0, 30);
                                let wiz_count = 4usize;
                                let wiz_r = 8.0f32;
                                for i in 0..wiz_count {
                                    let a = (i as f32) / (wiz_count as f32) * std::f32::consts::TAU;
                                    let p = glam::vec3(wiz_r * a.cos(), 0.6, wiz_r * a.sin());
                                    let _ = srv.spawn_wizard_npc(p);
                                }
                                let _ = srv.spawn_nivita_unique(glam::vec3(0.0, 0.6, 0.0));
                                let _dk = srv.spawn_death_knight(glam::vec3(60.0, 0.6, 0.0));
                                server_core::scene_build::add_demo_ruins_destructible(&mut srv);
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
    // WASM: parse ?zone=<slug> from the URL.
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(win) = web_sys::window() {
            if let Ok(search) = win.location().search() {
                let q = web_sys::UrlSearchParams::new_with_str(&search).ok();
                if let Some(qs) = q {
                    if let Some(v) = qs.get("zone") {
                        if !v.is_empty() {
                            return Some(v);
                        }
                    }
                }
            }
        }
    }
    // Back-compat: allow legacy RA_ZONE if present
    if let Ok(v) = std::env::var("RA_ZONE")
        && !v.is_empty()
    {
        return Some(v);
    }
    None
}
