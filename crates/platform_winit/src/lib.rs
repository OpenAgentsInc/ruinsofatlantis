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
    last_sec_start: std::time::Instant,
    cmds_this_sec: u32,
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
            last_sec_start: std::time::Instant::now(),
            cmds_this_sec: 0,
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
            // Wire a local replication channel for NPC/Boss status
            let (_srv, _cli) = net_core::transport::LocalLoopbackTransport::new(4096);
            let (tx_cli, rx_cli) = _cli.split();
            state.set_replication_rx(rx_cli);
            state.set_command_tx(tx_cli);
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.window = Some(window);
                self.transport_srv = Some(_srv);
                self.state = Some(state);
                #[cfg(feature = "demo_server")]
                {
                    let mut srv = server_core::ServerState::new();
                    // Spawn a few rings so NPC wizards have targets on spawn
                    srv.ring_spawn(8, 15.0, 20);
                    srv.ring_spawn(12, 30.0, 25);
                    srv.ring_spawn(15, 45.0, 30);
                    // Spawn the unique boss near center
                    let _ = srv.spawn_nivita_unique(glam::vec3(0.0, 0.6, 0.0));
                    self.demo_server = Some(srv);
                }
                self.last_time = Some(std::time::Instant::now());
                self.tick = 0;
                self.baseline_tick = 0;
                self.baseline = std::collections::HashMap::new();
                self.interest_radius_m = 40.0;
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
                    if let Some((win, state)) = cell.borrow_mut().take() {
                        self.window = Some(win);
                        self.state = Some(state);
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
                        // rate limit: 20 cmds/sec
                        let now = std::time::Instant::now();
                        if now.duration_since(self.last_sec_start).as_secs_f32() >= 1.0 {
                            self.last_sec_start = now;
                            self.cmds_this_sec = 0;
                        }
                        if self.cmds_this_sec >= 20 {
                            log::debug!("rate-limit: dropping client cmd");
                            continue;
                        }
                        self.cmds_this_sec += 1;
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
                // NPC AI/boss now run within authoritative step via ECS schedule.
                // Build replication messages
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
                // Send actor-centric snapshot v2 (authoritative snapshot)
                let tick64 = self.tick as u64;
                let asnap = srv.tick_snapshot_actors(tick64);
                // Build interest-limited view and delta vs baseline
                let center = s
                    .wizard_positions()
                    .first()
                    .copied()
                    .unwrap_or(glam::vec3(0.0, 0.0, 0.0));
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
                // spawns/updates
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
                        // pos
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
                        // yaw
                        let qy = net_core::snapshot::qyaw(a.yaw);
                        if net_core::snapshot::qyaw(b.yaw) != qy {
                            flags |= 2;
                            rec.qyaw = qy;
                        }
                        // hp
                        if b.hp != a.hp {
                            flags |= 4;
                            rec.hp = a.hp;
                        }
                        // alive
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
                // removals
                for id in self.baseline.keys() {
                    if !cur.contains_key(id) {
                        removals.push(*id);
                    }
                }
                // projectiles (full from ECS)
                let mut projectiles = Vec::new();
                for c in srv.ecs.iter() {
                    if let (Some(proj), Some(vel)) = (c.projectile.as_ref(), c.velocity.as_ref()) {
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
                let delta = net_core::snapshot::ActorSnapshotDelta {
                    v: 3,
                    tick: tick64,
                    baseline: self.baseline_tick,
                    spawns,
                    updates,
                    removals,
                    projectiles,
                };
                // encode + send
                let mut p4 = Vec::new();
                delta.encode(&mut p4);
                let mut f4 = Vec::with_capacity(p4.len() + 8);
                net_core::frame::write_msg(&mut f4, &p4);
                metrics::counter!("net.bytes_sent_total", "dir" => "tx").increment(f4.len() as u64);
                let _ = srv_xport.try_send(f4);
                // Send HUD status for local PC
                if let Some(pc_id) = srv.pc_actor
                    && let Some(pc) = srv.ecs.get(pc_id)
                {
                    let mana = pc.pool.as_ref().map(|p| p.mana).unwrap_or(0).clamp(0, u16::MAX as i32) as u16;
                    let mana_max = pc.pool.as_ref().map(|p| p.max).unwrap_or(0).clamp(0, u16::MAX as i32) as u16;
                    let gcd_ms = (pc.cooldowns.as_ref().map(|c| c.gcd_ready).unwrap_or(0.0) * 1000.0).clamp(0.0, u16::MAX as f32) as u16;
                    let cd = |sid: server_core::SpellId| -> f32 { pc.cooldowns.as_ref().and_then(|c| c.per_spell.get(&sid).copied()).unwrap_or(0.0) };
                    let spell_cds = vec![
                        (0u8, (cd(server_core::SpellId::Firebolt) * 1000.0) as u16),
                        (1u8, (cd(server_core::SpellId::Fireball) * 1000.0) as u16),
                        (2u8, (cd(server_core::SpellId::MagicMissile) * 1000.0) as u16),
                    ];
                    let burning_ms = (pc.burning.as_ref().map(|b| b.remaining_s).unwrap_or(0.0) * 1000.0) as u16;
                    let slow_ms = (pc.slow.as_ref().map(|s| s.remaining_s).unwrap_or(0.0) * 1000.0) as u16;
                    let stunned_ms = (pc.stunned.as_ref().map(|s| s.remaining_s).unwrap_or(0.0) * 1000.0) as u16;
                    let hud = net_core::snapshot::HudStatusMsg { v: net_core::snapshot::HUD_STATUS_VERSION, mana, mana_max, gcd_ms, spell_cds, burning_ms, slow_ms, stunned_ms };
                    let mut hb = Vec::new();
                    hud.encode(&mut hb);
                    let mut fh = Vec::with_capacity(hb.len() + 8);
                    net_core::frame::write_msg(&mut fh, &hb);
                    metrics::counter!("net.bytes_sent_total", "dir" => "tx").increment(fh.len() as u64);
                    let _ = srv_xport.try_send(fh);
                }
                // update baseline
                self.baseline = cur;
                self.baseline_tick = tick64;
                self.tick = self.tick.wrapping_add(1);
                // Now step authoritative systems (projectiles integrate, AI, etc.)
                srv.step_authoritative(dt, &wiz_pos);
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
