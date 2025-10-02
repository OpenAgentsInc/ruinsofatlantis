//! vox_onepath: isolated, no-flags demo that shows a single scripted
//! ray → carve → debris → remesh path. Launch with:
//!   cargo run -p render_wgpu --bin vox_onepath

use anyhow::Result;

fn main() -> Result<()> {
    render_wgpu::gfx::vox_onepath::run()
}
