use render_wgpu::gfx::Renderer;
use winit::event::WindowEvent;

#[test]
fn cc_demo_disables_cast_and_hud() {
    // Renderer::new() is heavy; instead validate policy mapping via set_zone_batches
    // using a minimal window that won't actually draw.
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    let window = winit::window::Window::new(&event_loop).unwrap();
    let mut r = pollster::block_on(Renderer::new(&window)).expect("renderer");
    r.set_zone_batches(Some(render_wgpu::gfx::zone_batches::GpuZoneBatches {
        slug: "cc_demo".into(),
    }));
    assert!(!r.zone_policy.allow_casting);
    assert!(!r.zone_policy.show_player_hud);

    // Try to queue a cast: key 1 should not set pc_cast_queued when casting disabled
    let kev = winit::event::KeyEvent::new(
        winit::keyboard::Key::Character("1".into()),
        winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Digit1),
        winit::event::ElementState::Pressed,
    );
    r.handle_window_event(&WindowEvent::KeyboardInput {
        device_id: winit::event::DeviceId::dummy(),
        event: kev,
        is_synthetic: true,
    });
    assert!(!r.pc_cast_queued, "cast should be gated off by zone policy");
}
