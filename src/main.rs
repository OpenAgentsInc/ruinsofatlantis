use ruinsofatlantis::platform_winit;

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
