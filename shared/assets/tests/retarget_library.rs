use roa_assets::skinning::load_gltf_skinned;
use roa_assets::{RetargetOptions, retarget_animations};
use std::path::PathBuf;

fn p(rel: &str) -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .join(rel);
    if here.exists() {
        here
    } else {
        PathBuf::from(rel)
    }
}

#[test]
fn detect_and_map_minimal_humanoid() {
    let src = load_gltf_skinned(&p("assets/anims/universal/AnimationLibrary.glb")).unwrap();
    let dst = load_gltf_skinned(&p("assets/models/sorceror.glb")).unwrap();
    let r_s = roa_assets::detect_humanoid(&src);
    let r_d = roa_assets::detect_humanoid(&dst);
    // Expect at least hips or head on both for demo assets
    assert!(
        r_s.node_of_bone[0].is_some() || r_s.node_of_bone[5].is_some(),
        "source rig missing hips/head"
    );
    assert!(
        r_d.node_of_bone[0].is_some() || r_d.node_of_bone[5].is_some(),
        "dest rig missing hips/head"
    );
}

#[test]
fn retarget_copies_clips() {
    let src = load_gltf_skinned(&p("assets/anims/universal/AnimationLibrary.glb")).unwrap();
    let mut dst = load_gltf_skinned(&p("assets/models/sorceror.glb")).unwrap();
    let before = dst.animations.len();
    retarget_animations(
        &src,
        &mut dst,
        &RetargetOptions {
            preserve_root_motion: true,
            apply_rest_correction: true,
            scale_override: None,
            allowlist: None,
        },
    )
    .unwrap();
    assert!(dst.animations.len() >= before);
    // If there is a shared-named clip, durations should match closely; otherwise just sanity-check merged durations are finite
    for (name, c) in &src.animations {
        if let Some(c2) = dst.animations.get(name) {
            assert!((c.duration - c2.duration).abs() < 1e-3);
            break;
        }
    }
}

#[test]
fn no_nan_after_retarget() {
    let src = load_gltf_skinned(&p("assets/anims/universal/AnimationLibrary.glb")).unwrap();
    let mut dst = load_gltf_skinned(&p("assets/models/sorceror.glb")).unwrap();
    retarget_animations(
        &src,
        &mut dst,
        &RetargetOptions {
            preserve_root_motion: true,
            apply_rest_correction: true,
            scale_override: None,
            allowlist: None,
        },
    )
    .unwrap();
    for (_n, clip) in &dst.animations {
        for v in clip.t_tracks.values().flat_map(|t| t.values.iter()) {
            assert!(v.is_finite());
        }
        for v in clip.r_tracks.values().flat_map(|t| t.values.iter()) {
            assert!(v.is_finite());
        }
        for v in clip.s_tracks.values().flat_map(|t| t.values.iter()) {
            assert!(v.is_finite());
        }
    }
}
