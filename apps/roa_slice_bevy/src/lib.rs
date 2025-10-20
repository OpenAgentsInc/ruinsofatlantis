//! Bevy vertical slice runner (library entry) so other crates can launch the slice.
use anyhow::Result;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::Mesh3d;
use bevy::prelude::Name;
use bevy::prelude::*;
use bevy::scene::SceneRoot;
use bevy_animation::graph::AnimationNodeIndex;
use bevy_animation::prelude::{AnimationGraph, AnimationGraphHandle, AnimationPlayer};
use bevy_gltf::Gltf;

use roa_domain::{
    Command, DragonController, SimTime, TransformState, sys_apply_commands_to_controller,
    tick_sim_time,
};

const DEFAULT_ZONE: &str = "models/ruins.decompressed.gltf";
const DEFAULT_DRAGON: &str = "models/DragonProto_v2.glb";

#[derive(Resource, Default, Clone)]
struct SliceConfig {
    zone_picker: bool,
}

#[derive(Component)]
struct IsDragon;

pub fn run_slice(zone_picker: bool, zone_override: Option<String>) -> Result<()> {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "ROA Slice".into(),
            resolution: (1600u32, 900u32).into(),
            ..default()
        }),
        ..default()
    }));

    // Domain events/resources
    app.add_message::<Command>();
    app.insert_resource(SimTime::default());

    // Config
    let zone_picker_env = std::env::var("ROA_ZONE_PICKER")
        .ok()
        .map(|v| v == "1")
        .unwrap_or(false);
    let _zone_scene = zone_override.unwrap_or_else(|| DEFAULT_ZONE.to_string());
    app.insert_resource(SliceConfig {
        zone_picker: zone_picker || zone_picker_env,
    });

    // Schedules: Sim (FixedUpdate) and Present (Update/PostUpdate)
    app.add_systems(Startup, (setup_camera_lights, setup_slice));
    app.add_systems(
        FixedUpdate,
        (
            tick_sim_time,                      // increments SimTime.tick and updates dt
            input_to_domain_commands,           // Bevy input → Command messages
            sys_apply_commands_to_controller,   // Domain: controller → TransformState
            apply_transformstate_to_transforms, // Presenter bridge for transforms
        ),
    );
    app.add_systems(
        Update,
        (
            spawn_dragon_scene,
            ensure_domain_dragon,
            prepare_dragon_animation,
            kickoff_dragon_animation,
            cycle_dragon_animation,
            prune_dragon_extras,
        ),
    );

    app.run();
    Ok(())
}

// --- Startup setup ---
fn setup_camera_lights(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 4.0, 8.0).looking_at(Vec3::new(0.0, 1.5, 0.0), Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            illuminance: 30_000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.4, 0.0)),
    ));
}

fn setup_slice(mut commands: Commands, cfg: Res<SliceConfig>, assets: Res<AssetServer>) {
    if cfg.zone_picker {
        info!("Zone picker active. Use UI to load zones.");
        return;
    }

    // Zone temporarily disabled for bring-up clarity.
    // When re-enabling, spawn SceneRoot for `cfg.zone_scene` here.

    // Load GLTF with cameras/lights disabled to avoid extra scene clutter.
    let dragon_gltf: Handle<Gltf> =
        assets.load_with_settings(DEFAULT_DRAGON, |s: &mut bevy_gltf::GltfLoaderSettings| {
            s.load_cameras = false;
            s.load_lights = false;
            s.load_animations = true;
        });
    commands.insert_resource(DragonGltf(dragon_gltf));
    info!("Slice: requested dragon glTF={}", DEFAULT_DRAGON);
}

// --- Input mapping: Bevy input → domain `Command` messages ---
fn input_to_domain_commands(
    kb: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<bevy::input::mouse::MouseButton>>,
    mut ev_cmd: bevy::ecs::message::MessageWriter<Command>,
    mut mouse_motion: bevy::ecs::message::MessageReader<bevy::input::mouse::MouseMotion>,
    q_window: Query<&Window>,
) {
    let Ok(window) = q_window.single() else {
        return;
    };
    // Axes: WASD / arrows
    let mut x = 0.0;
    let mut y = 0.0;
    if kb.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        x -= 1.0;
    }
    if kb.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        x += 1.0;
    }
    if kb.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        y += 1.0;
    }
    if kb.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        y -= 1.0;
    }
    if x != 0.0 || y != 0.0 {
        ev_cmd.write(Command::MoveAxes { x, y });
    }

    // Flight ascend/descend
    if kb.any_pressed([KeyCode::Space]) {
        ev_cmd.write(Command::Ascend(1.0));
    }
    if kb.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
        ev_cmd.write(Command::Descend(1.0));
    }

    // Mouse look (when window focused)
    if window.focused {
        let mut dx = 0.0;
        let mut dy = 0.0;
        for m in mouse_motion.read() {
            dx += m.delta.x;
            dy += m.delta.y;
        }
        if dx != 0.0 || dy != 0.0 {
            ev_cmd.write(Command::LookDelta { dx, dy });
        }
    }

    // Actions
    if kb.just_pressed(KeyCode::KeyF) {
        ev_cmd.write(Command::Takeoff);
    }
    if kb.just_pressed(KeyCode::KeyL) {
        ev_cmd.write(Command::Land);
    }
    if mouse_buttons.just_pressed(bevy::input::mouse::MouseButton::Left) {
        ev_cmd.write(Command::AttackPrimary);
    }
}

// --- Presenter bridge: apply domain TransformState to Bevy Transform ---
fn apply_transformstate_to_transforms(
    mut q_xform: Query<&mut Transform, With<IsDragon>>,
    q_domain: Query<&TransformState>,
) {
    if let Ok(mut t) = q_xform.single_mut() {
        if let Ok(state) = q_domain.single() {
            t.translation = state.pos;
            let yaw = state.rot_yaw_pitch_roll.x;
            let pitch = state.rot_yaw_pitch_roll.y;
            t.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
        }
    }
}

// --- Debug / placeholder: spawn a domain dragon if not present ---
fn ensure_domain_dragon(
    mut commands: Commands,
    q_has_domain: Query<Entity, With<DragonController>>,
    q_visual: Query<Entity, With<IsDragon>>,
) {
    if q_has_domain.single().is_err() && q_visual.single().is_ok() {
        commands.spawn((DragonController::default(), TransformState::default()));
    }
}

// Spawn the dragon SceneRoot once the GLTF asset is available.
fn spawn_dragon_scene(
    mut commands: Commands,
    dragon: Res<DragonGltf>,
    gltfs: Res<Assets<Gltf>>,
    assets: Res<AssetServer>,
    mut spawned: Local<bool>,
) {
    if *spawned {
        return;
    }
    let Some(g) = gltfs.get(&dragon.0) else {
        return;
    };
    if let Some(first_scene) = g.scenes.get(0).cloned() {
        let dragon_xform = Transform::from_xyz(0.0, 1.5, 0.0).with_scale(Vec3::splat(1.0));
        commands.spawn((SceneRoot(first_scene), dragon_xform, IsDragon));
        info!("dragon: spawned Scene0");
        *spawned = true;
    } else {
        // Fallback: direct label
        let scene0 = assets.load(GltfAssetLabel::Scene(0).from_asset(DEFAULT_DRAGON));
        let dragon_xform = Transform::from_xyz(0.0, 1.5, 0.0).with_scale(Vec3::splat(1.0));
        commands.spawn((SceneRoot(scene0), dragon_xform, IsDragon));
        *spawned = true;
    }
}

#[derive(Resource, Clone)]
struct DragonGltf(Handle<Gltf>);

#[derive(Component, Clone)]
struct DragonAnimSeq {
    nodes: Vec<AnimationNodeIndex>,
    idx: usize,
}

/// Marker on the entity that owns the live AnimationPlayer we drive
#[derive(Component)]
struct DragonAnimController;

// (Removed) pick_clip helper; we now take the first two clips directly.

fn prepare_dragon_animation(
    mut commands: Commands,
    dragon: Res<DragonGltf>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    q_dragon: Query<(Entity, Option<&Children>), With<IsDragon>>,
    q_children: Query<&Children>,
    q_players: Query<Entity, With<AnimationPlayer>>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Some(g) = gltfs.get(&dragon.0) else {
        return;
    };
    // Build a graph with all clips and record node indices
    let mut seq_nodes: Vec<AnimationNodeIndex> = Vec::new();
    if g.animations.is_empty() {
        info!("dragon: no animations found in GLTF");
        *done = true;
        return;
    }
    let mut graph = AnimationGraph::new();
    for h in g.animations.iter() {
        let n = graph.add_clip(h.clone(), 1.0, graph.root);
        seq_nodes.push(n);
    }
    let handle = graphs.add(graph);
    // Find existing AnimationPlayer under the dragon root; otherwise use root
    let Some((root, kids)) = q_dragon.iter().next() else {
        return;
    };
    let mut stack: Vec<Entity> = Vec::new();
    if let Some(k) = kids {
        for child in k.iter() {
            stack.push(child.clone());
        }
    }
    let mut player_ent: Option<Entity> = None;
    while let Some(e) = stack.pop() {
        if q_players.get(e).is_ok() {
            player_ent = Some(e);
            break;
        }
        if let Ok(ch) = q_children.get(e) {
            for child in ch.iter() {
                stack.push(child.clone());
            }
        }
    }
    let target = player_ent.unwrap_or(root);
    let mut ecmd = commands.entity(target);
    if q_players.get(target).is_err() {
        ecmd.insert(AnimationPlayer::default());
    }
    ecmd.insert(AnimationGraphHandle(handle))
        .insert(DragonAnimSeq {
            nodes: seq_nodes,
            idx: 0,
        })
        .insert(DragonAnimController);
    info!(
        "dragon: driving AnimationPlayer on {:?} ({} clip(s))",
        target,
        g.animations.len()
    );
    *done = true;
}

fn kickoff_dragon_animation(
    mut q: Query<(&mut AnimationPlayer, &mut DragonAnimSeq), With<DragonAnimController>>,
) {
    if let Ok((mut player, seq)) = q.single_mut() {
        if let Some(&node) = seq.nodes.get(seq.idx) {
            if !player.is_playing_animation(node) {
                player.start(node);
                info!("dragon: playing animation node {:?}", node);
            }
        }
    }
}

fn cycle_dragon_animation(
    mut q: Query<(&mut AnimationPlayer, &mut DragonAnimSeq), With<DragonAnimController>>,
) {
    if let Ok((mut player, mut seq)) = q.single_mut() {
        if seq.nodes.is_empty() {
            return;
        }
        let cur = seq.nodes[seq.idx];
        let finished = player
            .animation(cur)
            .map(|a| a.is_finished())
            .unwrap_or(true);
        if finished {
            seq.idx = (seq.idx + 1) % seq.nodes.len();
            let next = seq.nodes[seq.idx];
            player.stop_all();
            player.start(next);
            info!("dragon: switched animation to node {:?}", next);
        }
    }
}

// One-shot prune pass to remove obvious helper cubes/planes from the dragon scene.
fn prune_dragon_extras(
    mut commands: Commands,
    mut done: Local<bool>,
    q_root: Query<(Entity, Option<&Children>), With<IsDragon>>,
    q_children: Query<&Children>,
    q_name_mesh: Query<(Option<&Name>, Option<&Mesh3d>)>,
) {
    if *done {
        return;
    }
    let Ok((_root, kids)) = q_root.single() else {
        return;
    };
    let mut stack: Vec<Entity> = Vec::new();
    if let Some(k) = kids {
        for c in k.iter() {
            stack.push(c);
        }
    }
    let mut removed = 0usize;
    while let Some(e) = stack.pop() {
        // enqueue children first
        if let Ok(children) = q_children.get(e) {
            for c in children.iter() {
                stack.push(c);
            }
        }
        // if it's a mesh and name looks like a helper, remove it
        if let Ok((maybe_name, maybe_mesh)) = q_name_mesh.get(e) {
            if maybe_mesh.is_some() {
                if let Some(name) = maybe_name {
                    let s = name.as_str().to_ascii_lowercase();
                    if s.contains("cube")
                        || s.contains("block")
                        || s.contains("plane")
                        || s.contains("grid")
                        || s.contains("floor")
                        || s.contains("helper")
                        || s.contains("light")
                        || s.contains("lamp")
                        || s.contains("sphere")
                        || s.contains("sun")
                        || s.contains("emiss")
                    {
                        commands.entity(e).despawn();
                        removed += 1;
                    }
                }
            }
        }
    }
    if removed > 0 {
        info!("pruned {} helper mesh(es) under dragon", removed);
    }
    *done = true;
}
