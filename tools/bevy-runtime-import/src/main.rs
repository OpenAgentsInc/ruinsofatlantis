use anyhow::{Result, anyhow};
use bevy_app::App;
use bevy_asset::{AssetServer, Assets, Handle};
use bevy_gltf::Gltf;
use bevy_scene::ScenePlugin;
use bevy_transform::TransformPlugin;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "bevy-runtime-import")]
#[command(about = "Spike: import a GLB/GLTF at runtime using bevy_gltf and summarize")]
struct Cli {
    /// Path to a .gltf or .glb
    input: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    // Minimal Bevy app: Asset + GLTF + Scene + Transform plugins
    let mut app = App::new();
    app.add_plugins((
        bevy_asset::AssetPlugin::default(),
        bevy_gltf::GltfPlugin::default(),
        ScenePlugin::default(),
        TransformPlugin::default(),
    ));

    // Load the GLTF as a Gltf asset
    let h: Handle<Gltf> = {
        let asset_server = app.world().resource::<AssetServer>();
        asset_server.load(cli.input.as_str())
    };

    // Pump the app until the asset is available or we timeout after N frames
    let mut found: Option<Gltf> = None;
    for _ in 0..300 {
        // ~300 ticks budget
        app.update();
        if let Some(gl) = app.world().resource::<Assets<Gltf>>().get(&h).cloned() {
            found = Some(gl);
            break;
        }
    }
    let gltf = found.ok_or_else(|| anyhow!("timed out waiting for GLTF to load"))?;
    // Summarize
    let scenes = gltf.scenes.len();
    let meshes = gltf.meshes.len();
    let nodes = gltf.nodes.len();
    let skins = gltf.skins.len();
    #[cfg(feature = "bevy_animation")]
    let animations = gltf.animations.len();
    #[cfg(not(feature = "bevy_animation"))]
    let animations = 0usize;
    println!(
        "bevy-runtime-import: scenes={} nodes={} meshes={} skins={} animations={}",
        scenes, nodes, meshes, skins, animations
    );
    Ok(())
}
