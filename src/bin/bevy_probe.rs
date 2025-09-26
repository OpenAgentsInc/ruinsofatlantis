use bevy::app::AppExit;
use bevy::gltf::Gltf;
use bevy::prelude::*;

#[derive(Resource, Default)]
struct ProbeState {
    handle: Option<Handle<Gltf>>,
    saved: bool,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        }))
        .init_resource::<ProbeState>()
        .add_systems(Startup, load_wizard)
        .add_systems(Update, try_extract_material)
        .run();
}

fn load_wizard(mut state: ResMut<ProbeState>, assets: Res<AssetServer>) {
    if state.handle.is_none() {
        let handle: Handle<Gltf> = assets.load("models/wizard.gltf");
        info!("bevy_probe: loading assets/models/wizard.gltf");
        state.handle = Some(handle);
    }
}

fn try_extract_material(
    mut state: ResMut<ProbeState>,
    gltfs: Res<Assets<Gltf>>,
    materials: Res<Assets<bevy::pbr::StandardMaterial>>,
    images: Res<Assets<bevy::render::texture::Image>>,
    mut exit: EventWriter<AppExit>,
) {
    if state.saved {
        return;
    }
    let Some(handle) = state.handle.as_ref() else {
        return;
    };
    let Some(gltf) = gltfs.get(handle) else {
        return;
    };

    if gltf.materials.is_empty() {
        warn!("bevy_probe: glTF has no materials yet; waiting...");
        return;
    }
    let mat_handle = &gltf.materials[0];
    let Some(mat) = materials.get(mat_handle) else {
        return;
    };

    if let Some(tex_handle) = &mat.base_color_texture {
        if let Some(img) = images.get(tex_handle) {
            let path = std::path::Path::new("data/debug");
            let _ = std::fs::create_dir_all(path);
            let out = path.join("wizard_basecolor.png");
            if let Err(e) = save_image_rgba8(img, &out) {
                error!("bevy_probe: failed to save image: {e}");
            } else {
                info!("bevy_probe: saved {}", out.display());
                state.saved = true;
                exit.send(AppExit::Success);
            }
        }
    } else {
        info!("bevy_probe: base_color_texture is None; nothing to write");
        state.saved = true;
        exit.send(AppExit::Success);
    }
}

fn save_image_rgba8(
    img: &bevy::render::texture::Image,
    out: &std::path::Path,
) -> anyhow::Result<()> {
    use bevy::render::render_resource::TextureFormat as Tf;
    let (w, h) = (
        img.texture_descriptor.size.width,
        img.texture_descriptor.size.height,
    );
    let (bytes, is_owned): (&[u8], bool) = match img.texture_descriptor.format {
        Tf::Rgba8Unorm | Tf::Rgba8UnormSrgb => (img.data.as_slice(), false),
        Tf::Bgra8Unorm | Tf::Bgra8UnormSrgb => {
            // Convert BGRA -> RGBA
            let mut out = vec![0u8; (w * h * 4) as usize];
            for (i, px) in img.data.chunks_exact(4).enumerate() {
                let o = i * 4;
                out[o] = px[2];
                out[o + 1] = px[1];
                out[o + 2] = px[0];
                out[o + 3] = px[3];
            }
            (Box::leak(out.into_boxed_slice()), true)
        }
        other => anyhow::bail!("unsupported format: {:?}", other),
    };
    image::save_buffer(out, bytes, w, h, image::ExtendedColorType::Rgba8)?;
    if is_owned { /* leaked buffer freed on process exit */ }
    Ok(())
}
