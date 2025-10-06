//! platform_winit: window + input + present loop (winit 0.30).
//!
//! Provides a minimal `run()` that creates a window and drives the
//! `render_wgpu::gfx::Renderer` via winit's ApplicationHandler API.

use net_core::snapshot::SnapshotEncode;
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
    #[allow(dead_code)]
    repl_tx: Option<net_core::channel::Tx>,
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
            let (tx, rx) = net_core::channel::channel();
            state.set_replication_rx(rx);
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.window = Some(window);
                self.repl_tx = Some(tx);
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
        if let (Some(tx), Some(s)) = (&self.repl_tx, &mut self.state) {
            // Step demo server AI toward wizard positions
            #[cfg(feature = "demo_server")]
            if let Some(srv) = &mut self.demo_server {
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
                log::info!(
                    "demo_server: stepping dt={:.3}s; npcs={} wizards={}",
                    dt,
                    srv.npcs.len(),
                    wiz_pos.len()
                );
                let mut items: Vec<net_core::snapshot::NpcItem> = Vec::new();
                for n in &srv.npcs {
                    items.push(net_core::snapshot::NpcItem {
                        id: n.id.0,
                        hp: n.hp,
                        max: n.max_hp,
                        pos: [n.pos.x, n.pos.y, n.pos.z],
                        radius: n.radius,
                        alive: if n.alive { 1 } else { 0 },
                        attack_anim: n.attack_anim,
                    });
                }
                // Send NPC list
                let list = net_core::snapshot::NpcListMsg { items };
                let mut payload = Vec::new();
                list.encode(&mut payload);
                let mut framed = Vec::with_capacity(payload.len() + 8);
                net_core::frame::write_msg(&mut framed, &payload);
                metrics::counter!("net.bytes_sent_total", "dir" => "tx")
                    .increment(framed.len() as u64);
                let _ = tx.try_send(framed);
                log::info!("demo_server: sent NpcListMsg items={}", list.items.len());
                // Send BossStatus if available
                if let Some(st) = srv.nivita_status() {
                    let bs = net_core::snapshot::BossStatusMsg {
                        name: st.name,
                        ac: st.ac,
                        hp: st.hp,
                        max: st.max,
                        pos: [st.pos.x, st.pos.y, st.pos.z],
                    };
                    let mut p2 = Vec::new();
                    bs.encode(&mut p2);
                    let mut f2 = Vec::with_capacity(p2.len() + 8);
                    net_core::frame::write_msg(&mut f2, &p2);
                    metrics::counter!("net.bytes_sent_total", "dir" => "tx")
                        .increment(f2.len() as u64);
                    let _ = tx.try_send(f2);
                    log::info!(
                        "demo_server: sent BossStatus pos=({:.1},{:.1},{:.1}) hp={}/{}",
                        st.pos.x,
                        st.pos.y,
                        st.pos.z,
                        st.hp,
                        st.max
                    );
                }
                // Also send consolidated TickSnapshot (migration target)
                let mut npc_rep: Vec<net_core::snapshot::NpcRep> = Vec::with_capacity(srv.npcs.len());
                for n in &srv.npcs {
                    // Face nearest wizard for demo yaw until server provides it
                    let mut yaw = 0.0f32;
                    let mut best_d2 = f32::INFINITY;
                    for w in &wiz_pos {
                        let dx = w.x - n.pos.x;
                        let dz = w.z - n.pos.z;
                        let d2 = dx * dx + dz * dz;
                        if d2 < best_d2 {
                            best_d2 = d2;
                            yaw = dx.atan2(dz);
                        }
                    }
                    npc_rep.push(net_core::snapshot::NpcRep {
                        id: n.id.0,
                        archetype: 0,
                        pos: [n.pos.x, n.pos.y, n.pos.z],
                        yaw,
                        radius: n.radius,
                        hp: n.hp,
                        max: n.max_hp,
                        alive: n.alive,
                    });
                }
                let wiz_rep: Vec<net_core::snapshot::WizardRep> = wiz_pos
                    .iter()
                    .enumerate()
                    .map(|(i, p)| net_core::snapshot::WizardRep {
                        id: i as u32,
                        kind: 0,
                        pos: [p.x, p.y, p.z],
                        yaw: 0.0,
                        hp: 100,
                        max: 100,
                    })
                    .collect();
                let boss_rep = srv.nivita_status().map(|st| net_core::snapshot::BossRep {
                    id: srv.nivita_id.map(|i| i.0).unwrap_or(0),
                    name: st.name,
                    pos: [st.pos.x, st.pos.y, st.pos.z],
                    hp: st.hp,
                    max: st.max,
                    ac: st.ac,
                });
                let ts = net_core::snapshot::TickSnapshot {
                    v: 1,
                    tick: self.tick,
                    wizards: wiz_rep,
                    npcs: npc_rep,
                    projectiles: Vec::new(),
                    boss: boss_rep,
                };
                let mut p3 = Vec::new();
                ts.encode(&mut p3);
                let mut f3 = Vec::with_capacity(p3.len() + 8);
                net_core::frame::write_msg(&mut f3, &p3);
                metrics::counter!("net.bytes_sent_total", "dir" => "tx").increment(f3.len() as u64);
                let _ = tx.try_send(f3);
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
