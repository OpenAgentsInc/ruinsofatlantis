use ruinsofatlantis::platform_winit;

fn main() {
    // Developer-friendly default logging (info+) unless RUST_LOG overrides
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .try_init();
    if let Err(e) = platform_winit::run() {
        eprintln!("error: {e}");
    }
}
