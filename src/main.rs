#[cfg(not(target_arch = "wasm32"))]
fn main() {
    // For the vertical slice (ADR-0003), run the Bevy app by default.
    // Bevy's LogPlugin sets up logging; avoid initializing a second logger.
    if let Err(e) = roa_slice_bevy::run_slice(false, None) {
        eprintln!("error: {e}");
    }
}

// On web, provide a `main` symbol that sets up console logging + panic hook
// and then hands control to the winit event loop.
#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);
    // Keep winit-based runtime on the web target.
    let _ = ruinsofatlantis::platform_winit::run();
}
