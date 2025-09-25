mod platform_winit;
pub mod gfx;
pub mod assets;
pub mod ecs;
pub mod core; // production types (data/rules/combat)
pub mod sim;  // deterministic simulation harness modules

fn main() {
    // Minimal logger to see wgpu/winit info in dev
    env_logger::init();
    if let Err(e) = platform_winit::run() {
        eprintln!("error: {e}");
    }
}
