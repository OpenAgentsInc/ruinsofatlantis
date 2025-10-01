use ruinsofatlantis::platform_winit;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    // Developer-friendly default logging (info+) unless RUST_LOG overrides
    // Suppress noisy GPU backend logs by default; honor RUST_LOG if set.
    let default = "info,ruinsofatlantis=info,wgpu_hal=off,wgpu_core=off,wgpu=off,naga=off";
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default))
        .format_timestamp_secs()
        .try_init();
    // Flag parsing handled in renderer (checks --no-vsync / RA_NO_VSYNC)
    if let Err(e) = platform_winit::run() {
        eprintln!("error: {e}");
    }
}

// On web, provide a `main` symbol that sets up console logging + panic hook
// and then hands control to the winit event loop.
#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);
    let _ = platform_winit::run();
}
