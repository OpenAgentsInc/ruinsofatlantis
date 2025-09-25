mod platform_winit;
pub mod gfx;

fn main() {
    // Minimal logger to see wgpu/winit info in dev
    env_logger::init();
    if let Err(e) = platform_winit::run() {
        eprintln!("error: {e}");
    }
}
