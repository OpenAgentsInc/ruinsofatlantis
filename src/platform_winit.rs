use crate::gfx::Renderer;
use wgpu::SurfaceError;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes},
};

#[derive(Default)]
struct App {
    window: Option<Window>,
    state: Option<Renderer>,
}

// Default is derived

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
            let state = match pollster::block_on(Renderer::new(&window)) {
                Ok(s) => s,
                Err(e) => {
                    log::info!("Renderer init skipped: {e}");
                    event_loop.exit();
                    return;
                }
            };
            self.window = Some(window);
            self.state = Some(state);
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
        // Feed input to the renderer first (for keyboard-driven controller)
        state.handle_window_event(&event);
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size),
            WindowEvent::RedrawRequested => {
                if let Err(err) = state.render() {
                    match err {
                        SurfaceError::Lost | SurfaceError::Outdated => {
                            state.resize(window.inner_size())
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
        if let Some(win) = &self.window {
            win.request_redraw();
        }
    }
}

fn is_headless() -> bool {
    if std::env::var("RA_HEADLESS").map(|v| v == "1").unwrap_or(false) { return true; }
    std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none()
}

pub fn run() -> anyhow::Result<()> {
    if is_headless() {
        // In headless/CI environments without a windowing system, skip creating a window.
        // This keeps `cargo run` non-interactive and error-free.
        return Ok(());
    }
    let event_loop = EventLoop::new()?;
    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}
