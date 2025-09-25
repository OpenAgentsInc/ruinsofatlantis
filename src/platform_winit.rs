use crate::render_wgpu::WgpuState;
use wgpu::SurfaceError;
use winit::{
    event::*,
    event_loop::EventLoop,
    window::WindowAttributes,
};

pub fn run() -> anyhow::Result<()> {
    // EventLoop is the app driver
    let event_loop = EventLoop::new()?;
    let window = event_loop.create_window(
        WindowAttributes::default().with_title("Ruins of Atlantis — Awaken"),
    )?;

    // Initialize wgpu (blocking for simplicity)
    let mut state = pollster::block_on(WgpuState::new(&window))?;

    event_loop.run(|event, elwt| match event {
        Event::WindowEvent { window_id, event } if window_id == window.id() => match event {
            WindowEvent::CloseRequested => elwt.exit(),
            WindowEvent::Resized(size) => {
                state.resize(size);
            }
            WindowEvent::RedrawRequested => {
                if let Err(err) = state.render() {
                    match err {
                        SurfaceError::Lost | SurfaceError::Outdated => state.reconfigure_surface(),
                        SurfaceError::OutOfMemory => elwt.exit(),
                        e => eprintln!("render error: {e:?}"),
                    }
                }
            }
            _ => {}
        },
        Event::AboutToWait => {
            window.request_redraw();
        }
        _ => {}
    })?;

    Ok(())
}
