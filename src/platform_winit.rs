use crate::render_wgpu::WgpuState;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

pub fn run() -> anyhow::Result<()> {
    // EventLoop is the app driver
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Ruins of Atlantis — Awaken")
        .build(&event_loop)?;

    // Initialize wgpu (blocking for simplicity)
    let mut state = pollster::block_on(WgpuState::new(&window))?;

    event_loop.run(move |event, elwt| match event {
        Event::WindowEvent { window_id, event } if window_id == window.id() => {
            match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(size) => {
                    state.resize(size);
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    state.resize(*new_inner_size);
                }
                _ => {}
            }
        }
        Event::AboutToWait => {
            window.request_redraw();
        }
        Event::WindowEvent { .. } => {}
        Event::RedrawRequested(id) if id == window.id() => {
            if let Err(err) = state.render() {
                // Recreate the surface on lost/outdated
                if err.is_surface_lost() || err.is_outdated() {
                    state.reconfigure_surface();
                } else {
                    eprintln!("render error: {err:?}");
                }
            }
        }
        _ => {}
    })?;

    Ok(())
}

