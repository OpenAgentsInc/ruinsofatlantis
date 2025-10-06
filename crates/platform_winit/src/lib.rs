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

#[derive(Default)]
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
                                // Owner wizard id 1 = PC in server_core
                                srv.spawn_projectile_from_pc(p, d, server_core::ProjKind::Firebolt);
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
                                // Owner wizard id 1 = PC in server_core
                                srv.spawn_projectile_from_pc(p, d, server_core::ProjKind::Fireball);
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
                                // Spawn a small volley (3) with slight spread
                                let spreads = [-0.06f32, 0.0, 0.06];
                                for sgn in spreads {
                                    let yaw = sgn;
                                    let ry = glam::Quat::from_rotation_y(yaw);
                                    let dir2 = (ry * d).normalize_or_zero();
                                    srv.spawn_projectile_from_pc(p, dir2, server_core::ProjKind::MagicMissile);
                                }
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
                let _hits = srv.step_npc_ai(dt, &wiz_pos);
                server_core::systems::boss::boss_seek_and_integrate(srv, dt, &wiz_pos);
                // Build replication messages
                if std::env::var("RA_LOG_DEMO")
                    .map(|v| v == "1")
                    .unwrap_or(false)
                {
                    log::info!(
                        "demo_server: stepping dt={:.3}s; npcs={} wizards={}",
                        dt,
                        srv.npcs.len(),
                        wiz_pos.len()
                    );
                } else {
                    log::debug!(
                        "demo_server: stepping dt={:.3}s; npcs={} wizards={}",
                        dt,
                        srv.npcs.len(),
                        wiz_pos.len()
                    );
                }
                // Send actor-centric snapshot v2 (authoritative snapshot)
                let asnap = srv.tick_snapshot_actors(self.tick as u64);
                let mut p4 = Vec::new();
                asnap.encode(&mut p4);
                let mut f4 = Vec::with_capacity(p4.len() + 8);
                net_core::frame::write_msg(&mut f4, &p4);
                metrics::counter!("net.bytes_sent_total", "dir" => "tx").increment(f4.len() as u64);
                let _ = srv_xport.try_send(f4);
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
