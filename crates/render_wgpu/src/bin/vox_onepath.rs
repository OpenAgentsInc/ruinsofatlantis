//! vox_onepath: isolated, no-flags demo that shows a single scripted
//! ray → carve → debris → remesh path. Launch with:
//!   cargo run -p render_wgpu --bin vox_onepath

use anyhow::Result;

fn init_logging() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .try_init();
}

fn main() -> Result<()> {
    init_logging();
    render_wgpu::gfx::vox_onepath::run()
}
