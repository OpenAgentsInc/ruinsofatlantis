// Bevy vertical slice entrypoint (ADR-0003) — binary thin wrapper that parses CLI
// and calls the library runner (`run_slice`).
use anyhow::Result;
use clap::Parser;
use roa_slice_bevy::run_slice;

#[derive(Parser, Debug)]
#[command(name = "roa-slice", about = "Ruins of Atlantis — Bevy vertical slice")]
struct Cli {
    /// Load the zone picker UI (otherwise auto-loads a default zone/dragon)
    #[arg(long)]
    zone_picker: bool,

    /// Explicit zone scene path (GLB#Scene)
    #[arg(long)]
    zone: Option<String>,
}

fn main() -> Result<()> {
    let args = Cli::parse();
    run_slice(args.zone_picker, args.zone)
}
