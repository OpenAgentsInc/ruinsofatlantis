// Bevy vertical slice entrypoint (ADR-0003):
// - Loads a default zone and dragon at startup.
// - Zone picker is hidden by default; enable with --zone-picker or ROA_ZONE_PICKER=1.
// - Input mapping → domain commands → domain systems (FixedUpdate schedule).

use anyhow::Result;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::scene::SceneRoot;
use clap::Parser;

use roa_domain::{
    Command, DragonController, SimTime, TransformState, sys_apply_commands_to_controller,
    tick_sim_time,
};

const DEFAULT_ZONE: &str = "zones/valewind/valewind.glb";
const DEFAULT_DRAGON: &str = "anims/converted/RedDragon2021.glb";

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

#[derive(Resource, Default)]
struct SliceConfig {
    zone_scene: String,
    zone_picker: bool,
}

#[derive(Component)]
struct IsDragon;

fn main() -> Result<()> {
    let args = Cli::parse();

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
    let zone_scene = args
        .zone
        .clone()
        .unwrap_or_else(|| DEFAULT_ZONE.to_string());
    let zone_picker_env = std::env::var("ROA_ZONE_PICKER")
        .ok()
        .map(|v| v == "1")
        .unwrap_or(false);
    app.insert_resource(SliceConfig {
        zone_scene,
        zone_picker: args.zone_picker || zone_picker_env,
    });

    // Schedules: Sim (FixedUpdate) and Present (Update/PostUpdate)
    app.add_systems(Startup, (setup_camera_lights, setup_slice));
    app.add_systems(
        FixedUpdate,
        (
            tick_sim_time,                      // increments SimTime.tick and updates dt
            input_to_domain_commands,           // Bevy input → Command events
            sys_apply_commands_to_controller,   // Domain: controller → TransformState
            apply_transformstate_to_transforms, // Presenter bridge for transforms
        ),
    );
    app.add_systems(Update, (ensure_domain_dragon,));

    app.run();
    Ok(())
}

// --- Startup setup ---
fn setup_camera_lights(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 12.5, 22.0).looking_at(Vec3::new(0.0, 5.0, 0.0), Vec3::Y),
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

    // Auto-load zone scene root (supports either raw `path#Scene` or path only)
    let zone_handle: Handle<Scene> = if cfg.zone_scene.contains('#') {
        assets.load(cfg.zone_scene.clone())
    } else {
        assets.load(GltfAssetLabel::Scene(0).from_asset(cfg.zone_scene.clone()))
    };
    commands.spawn(SceneRoot(zone_handle));

    // Auto-load dragon scene root and tag it
    let dragon_scene0 = assets.load(GltfAssetLabel::Scene(0).from_asset(DEFAULT_DRAGON));
    commands.spawn((SceneRoot(dragon_scene0), IsDragon));

    info!(
        "Slice: auto-loaded zone={} dragon={}",
        cfg.zone_scene, DEFAULT_DRAGON
    );
}

// --- Input mapping: Bevy input → domain `Command` events ---
fn input_to_domain_commands(
    kb: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<bevy::input::mouse::MouseButton>>,
    mut ev_cmd: MessageWriter<Command>,
    mut mouse_motion: MessageReader<bevy::input::mouse::MouseMotion>,
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
