//! Bevy vertical slice runner (library entry) so other crates can launch the slice.
use anyhow::Result;
use bevy::gltf::GltfAssetLabel;
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::Mesh3d;
use bevy::prelude::Name;
use bevy::prelude::*;
use bevy::scene::{Scene, SceneRoot};
use bevy_animation::graph::AnimationNodeIndex;
use bevy_animation::prelude::{AnimationGraph, AnimationGraphHandle, AnimationPlayer};
use bevy_gltf::Gltf;

use roa_domain::{
    Command, DragonController, DragonTypeId, NpcKind, SimTime, SpawnNpc, TransformState,
    register_npc_domain, sys_apply_commands_to_controller, tick_sim_time,
};

const DEFAULT_ZONE: &str = "models/ruins.decompressed.gltf";
const DEFAULT_DRAGON: &str = "models/DragonProto_v2.glb";
const PUMPKIN_MODEL: &str = "models/Gourd.glb";
const ZOMBIE_MODEL: &str = "models/zombie.glb";

mod agar_dragon;
use agar_dragon::{AgarCamera, AgarDragonPlugin, PlayerDragon};

#[derive(Resource, Default, Clone)]
struct SliceConfig {
    zone_picker: bool,
    agar_mode: bool,
}

#[derive(Component)]
struct IsDragon;

#[derive(Component)]
struct IsZone;

#[derive(Resource, Clone)]
struct ZoneGltf(Handle<Gltf>);

#[derive(Resource, Clone)]
pub struct PumpkinScene(pub Handle<Scene>);

#[derive(Resource, Clone)]
pub struct ZombieScene(pub Handle<Scene>);

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

    let agar_mode_env = std::env::var("ROA_AGAR_MODE")
        .ok()
        .map(|v| v != "0")
        .unwrap_or(true);

    if !agar_mode_env {
        // Domain events/resources
        app.add_message::<Command>();
        app.insert_resource(SimTime::default());
        // Domain NPC events
        // Register domain NPC message storage on the world
        register_npc_domain(app.world_mut());
    }

    // Config
    let zone_picker_env = std::env::var("ROA_ZONE_PICKER")
        .ok()
        .map(|v| v == "1")
        .unwrap_or(false);
    let _zone_scene = zone_override.unwrap_or_else(|| DEFAULT_ZONE.to_string());
    app.insert_resource(SliceConfig {
        zone_picker: zone_picker || zone_picker_env,
        agar_mode: agar_mode_env,
    });

    // Schedules: Sim (FixedUpdate) and Present (Update/PostUpdate)
    app.add_systems(Startup, (setup_camera_lights, setup_slice));

    if !agar_mode_env {
        app.add_systems(
            FixedUpdate,
            (
                tick_sim_time,                      // increments SimTime.tick and updates dt
                input_to_domain_commands,           // Bevy input → Command messages
                sys_apply_commands_to_controller,   // Domain: controller → TransformState
                apply_transformstate_to_transforms, // Presenter bridge for transforms
            ),
        );
    }

    if agar_mode_env {
        app.add_systems(
            Update,
            (
                spawn_zone_scene,
                spawn_dragon_scene,
                prepare_dragon_animation,
                kickoff_dragon_animation,
                cycle_dragon_animation,
                prune_dragon_extras,
                apply_player_tint_after_spawn,
            ),
        );
        app.add_plugins(AgarDragonPlugin);
    } else {
        app.add_systems(
            Update,
            (
                spawn_zone_scene,
                spawn_dragon_scene,
                ensure_domain_dragon,
                prepare_dragon_animation,
                kickoff_dragon_animation,
                cycle_dragon_animation,
                prune_dragon_extras,
                apply_player_tint_after_spawn,
            ),
        );

        // NPC visuals & spawner
        app.add_plugins((
            npc_registry::NpcVisualRegistryPlugin,
            npc_spawn::NpcSpawnPlugin,
        ))
        .add_systems(Startup, demo_spawn_multiple_dragons);
    }

    app.run();
    Ok(())
}

// --- Startup setup ---
fn setup_camera_lights(mut commands: Commands, cfg: Res<SliceConfig>) {
    let mut camera_transform = if cfg.agar_mode {
        Transform::from_xyz(0.0, 28.0, 0.01)
    } else {
        Transform::from_xyz(0.0, 4.0, 8.0)
    };
    let target = if cfg.agar_mode {
        Vec3::new(0.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.5, 0.0)
    };
    camera_transform.look_at(target, Vec3::Y);
    let mut camera = commands.spawn((Camera3d::default(), camera_transform));
    if cfg.agar_mode {
        camera.insert(AgarCamera);
    }
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            illuminance: 30_000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.4, 0.0)),
    ));
}

fn demo_spawn_multiple_dragons(mut ev: bevy::ecs::message::MessageWriter<SpawnNpc>) {
    // Spawn green / red / blue variants using per-instance tint colors
    let offs = [(-8.0, 2.5, 0.0), (0.0, 3.0, 0.0), (8.0, 2.5, 0.0)];
    let tints = [
        glam::Vec3::new(0.1, 0.95, 0.2),   // green
        glam::Vec3::new(0.95, 0.15, 0.15), // red
        glam::Vec3::new(0.15, 0.4, 1.0),   // blue
    ];
    for (i, (x, y, z)) in offs.into_iter().enumerate() {
        let yaw = 0.0 + i as f32 * 0.7;
        ev.write(SpawnNpc {
            kind: NpcKind::Dragon(DragonTypeId("proto_v2")),
            pos: glam::Vec3::new(x, y, z),
            yaw,
            tint: Some(tints[i]),
        });
    }
}

fn setup_slice(mut commands: Commands, cfg: Res<SliceConfig>, assets: Res<AssetServer>) {
    if cfg.zone_picker {
        info!("Zone picker active. Use UI to load zones.");
        return;
    }

    // Load the CC demo zone geometry so we have a surface to play on.
    let zone_gltf: Handle<Gltf> =
        assets.load_with_settings(DEFAULT_ZONE, |s: &mut bevy_gltf::GltfLoaderSettings| {
            s.load_cameras = false;
            s.load_lights = true;
            s.load_animations = false;
        });
    commands.insert_resource(ZoneGltf(zone_gltf));
    info!("Slice: requested zone glTF={}", DEFAULT_ZONE);

    let pumpkin_scene: Handle<Scene> =
        assets.load(GltfAssetLabel::Scene(0).from_asset(PUMPKIN_MODEL));
    commands.insert_resource(PumpkinScene(pumpkin_scene));
    info!("Slice: requested pumpkin scene={}", PUMPKIN_MODEL);

    let zombie_scene: Handle<Scene> =
        assets.load(GltfAssetLabel::Scene(0).from_asset(ZOMBIE_MODEL));
    commands.insert_resource(ZombieScene(zombie_scene));
    info!("Slice: requested zombie scene={}", ZOMBIE_MODEL);

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

fn spawn_zone_scene(
    mut commands: Commands,
    zone: Option<Res<ZoneGltf>>,
    gltfs: Res<Assets<Gltf>>,
    assets: Res<AssetServer>,
    mut spawned: Local<bool>,
) {
    if *spawned {
        return;
    }
    let Some(zone) = zone else {
        return;
    };
    let Some(gltf) = gltfs.get(&zone.0) else {
        return;
    };
    if let Some(scene) = gltf.scenes.get(0).cloned() {
        commands.spawn((SceneRoot(scene), Transform::default(), IsZone));
        info!("zone: spawned Scene0 for {}", DEFAULT_ZONE);
        *spawned = true;
    } else {
        let scene0 = assets.load(GltfAssetLabel::Scene(0).from_asset(DEFAULT_ZONE));
        commands.spawn((SceneRoot(scene0), Transform::default(), IsZone));
        *spawned = true;
    }
}

// Spawn the dragon SceneRoot once the GLTF asset is available.
fn spawn_dragon_scene(
    mut commands: Commands,
    dragon: Res<DragonGltf>,
    gltfs: Res<Assets<Gltf>>,
    assets: Res<AssetServer>,
    mut spawned: Local<bool>,
    cfg: Res<SliceConfig>,
) {
    if *spawned {
        return;
    }
    let Some(g) = gltfs.get(&dragon.0) else {
        return;
    };
    if let Some(first_scene) = g.scenes.get(0).cloned() {
        let dragon_xform = Transform::from_xyz(0.0, 1.5, 0.0).with_scale(Vec3::splat(1.0));
        let mut entity = commands.spawn((SceneRoot(first_scene), dragon_xform, IsDragon));
        if cfg.agar_mode {
            entity.insert(PlayerDragon);
        }
        info!("dragon: spawned Scene0");
        *spawned = true;
    } else {
        // Fallback: direct label
        let scene0 = assets.load(GltfAssetLabel::Scene(0).from_asset(DEFAULT_DRAGON));
        let dragon_xform = Transform::from_xyz(0.0, 1.5, 0.0).with_scale(Vec3::splat(1.0));
        let mut entity = commands.spawn((SceneRoot(scene0), dragon_xform, IsDragon));
        if cfg.agar_mode {
            entity.insert(PlayerDragon);
        }
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
mod npc_registry;
mod npc_spawn;

// --- Player dragon tint ---
#[derive(Component)]
struct PlayerTintApplied;

fn apply_player_tint_after_spawn(
    mut commands: Commands,
    q_root: Query<(Entity, Option<&Children>), (With<IsDragon>, Without<PlayerTintApplied>)>,
    q_children: Query<&Children>,
    mut q_mat: Query<&mut MeshMaterial3d<StandardMaterial>>, // mesh material handle on mesh
    mut materials: ResMut<Assets<StandardMaterial>>,
    q_mesh: Query<&Mesh3d>,
) {
    let Ok((root, kids)) = q_root.single() else {
        return;
    };
    let tint = Color::srgb(0.15, 0.4, 1.0); // blue
    let mut stack: Vec<Entity> = Vec::new();
    if let Some(k) = kids {
        for child in k.iter() {
            stack.push(child);
        }
    }
    let mut painted = false;
    while let Some(e) = stack.pop() {
        if let Ok(ch) = q_children.get(e) {
            for c in ch.iter() {
                stack.push(c);
            }
        }
        if let Ok(mut mh) = q_mat.get_mut(e) {
            // Duplicate and tint material
            let handle = mh.0.clone();
            let new_mat = if let Some(orig) = materials.get(&handle).cloned() {
                let mut m = orig;
                m.base_color_texture = None;
                m.base_color = tint;
                m.unlit = true;
                m.emissive = tint.into();
                m
            } else {
                StandardMaterial {
                    base_color_texture: None,
                    base_color: tint,
                    unlit: true,
                    emissive: tint.into(),
                    ..Default::default()
                }
            };
            let new = materials.add(new_mat);
            *mh = MeshMaterial3d(new);
            painted = true;
        } else if q_mesh.get(e).is_ok() {
            // Attach a fresh tinted material if none present
            let new = materials.add(StandardMaterial {
                base_color_texture: None,
                base_color: tint,
                unlit: true,
                emissive: tint.into(),
                ..Default::default()
            });
            commands.entity(e).insert(MeshMaterial3d(new));
            painted = true;
        }
    }
    if painted {
        commands.entity(root).insert(PlayerTintApplied);
    }
}
