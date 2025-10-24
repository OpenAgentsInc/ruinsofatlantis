use bevy::prelude::*;
use bevy::scene::SceneRoot;
use bevy::ui::{Node, PositionType, Val};

use crate::{PumpkinScene, ZombieScene};

pub struct AgarDragonPlugin;

#[derive(Component)]
pub struct PlayerDragon;

#[derive(Component)]
pub struct AgarCamera;

#[derive(Component, Default)]
struct PlayerState {
    xp: f32,
    level: u32,
}

#[derive(Component)]
struct Pumpkin;

#[derive(Component)]
struct Zombie {
    speed: f32,
}

#[derive(Resource)]
struct PumpkinField {
    spawned: bool,
    positions: Vec<Vec3>,
    next_index: usize,
    spawn_count: usize,
}

#[derive(Resource)]
struct ZombieField {
    spawned: bool,
    positions: Vec<Vec3>,
    next_index: usize,
    spawn_count: usize,
}

#[derive(Resource)]
struct GameState {
    status: GameStatus,
    message: String,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum GameStatus {
    #[default]
    Playing,
    GameOver,
}

#[derive(Component)]
struct HudRoot;

#[derive(Component)]
struct HudText;

const PLAYER_HEIGHT: f32 = 1.8;
const PLAYER_BASE_SCALE: f32 = 1.0;
const PLAYER_SCALE_STEP: f32 = 0.25;
const PLAYER_BASE_SPEED: f32 = 7.0;
const PLAYER_SPEED_STEP: f32 = 1.5;
const PLAYER_COLLISION_RADIUS: f32 = 1.4;
const FIELD_HALF_EXTENT: f32 = 24.0;
const CAMERA_HEIGHT: f32 = 26.0;
const INITIAL_PUMPKINS: usize = 8;
const INITIAL_ZOMBIES: usize = 3;
const ZOMBIE_BASE_SPEED: f32 = 4.0;
const ZOMBIE_COLLISION_RADIUS: f32 = 1.6;
const ZOMBIE_LEVEL_REQUIREMENT: u32 = 3;
const ZOMBIE_KO_XP: f32 = 15.0;
const PUMPKIN_POINTS: [(f32, f32, f32); 12] = [
    (-16.0, 0.4, -12.0),
    (-10.0, 0.4, -6.0),
    (-6.0, 0.4, -14.0),
    (0.0, 0.4, -10.0),
    (6.0, 0.4, -6.0),
    (12.0, 0.4, -12.0),
    (-14.0, 0.4, 4.0),
    (-8.0, 0.4, 10.0),
    (0.0, 0.4, 12.0),
    (8.0, 0.4, 6.0),
    (14.0, 0.4, 0.0),
    (4.0, 0.4, -2.0),
];

const ZOMBIE_POINTS: [(f32, f32, f32); 6] = [
    (-20.0, 0.5, 18.0),
    (18.0, 0.5, 20.0),
    (-18.0, 0.5, -18.0),
    (15.0, 0.5, -15.0),
    (0.0, 0.5, 18.0),
    (20.0, 0.5, 0.0),
];

impl Default for PumpkinField {
    fn default() -> Self {
        Self {
            spawned: false,
            positions: PUMPKIN_POINTS
                .iter()
                .map(|&(x, y, z)| Vec3::new(x, y, z))
                .collect(),
            next_index: INITIAL_PUMPKINS % PUMPKIN_POINTS.len(),
            spawn_count: INITIAL_PUMPKINS.min(PUMPKIN_POINTS.len()),
        }
    }
}

impl PumpkinField {
    fn next_position(&mut self) -> Vec3 {
        if self.positions.is_empty() {
            return Vec3::ZERO;
        }
        let pos = self.positions[self.next_index];
        self.next_index = (self.next_index + 1) % self.positions.len();
        pos
    }

    fn reset(&mut self) {
        self.next_index = self.spawn_count % self.positions.len();
    }
}

impl Default for ZombieField {
    fn default() -> Self {
        Self {
            spawned: false,
            positions: ZOMBIE_POINTS
                .iter()
                .map(|&(x, y, z)| Vec3::new(x, y, z))
                .collect(),
            next_index: INITIAL_ZOMBIES % ZOMBIE_POINTS.len(),
            spawn_count: INITIAL_ZOMBIES.min(ZOMBIE_POINTS.len()),
        }
    }
}

impl ZombieField {
    fn next_position(&mut self) -> Vec3 {
        if self.positions.is_empty() {
            return Vec3::ZERO;
        }
        let pos = self.positions[self.next_index];
        self.next_index = (self.next_index + 1) % self.positions.len();
        pos
    }

    fn reset(&mut self) {
        self.next_index = self.spawn_count % self.positions.len();
    }
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            status: GameStatus::Playing,
            message: default_instructions().to_string(),
        }
    }
}

fn default_instructions() -> &'static str {
    "Collect pumpkins to grow. Reach level 3 to chomp zombies. Press R to reset after defeat."
}

impl GameState {
    fn is_playing(&self) -> bool {
        self.status == GameStatus::Playing
    }

    fn set_game_over(&mut self, message: impl Into<String>) {
        self.status = GameStatus::GameOver;
        self.message = message.into();
    }

    fn set_message(&mut self, message: impl Into<String>) {
        if self.is_playing() {
            self.message = message.into();
        }
    }

    fn reset(&mut self) {
        self.status = GameStatus::Playing;
        self.message = default_instructions().to_string();
    }
}

impl PlayerState {
    fn add_xp(&mut self, amount: f32) -> bool {
        self.xp += amount;
        let new_level = level_for_xp(self.xp);
        let leveled = new_level != self.level;
        self.level = new_level;
        leveled
    }

    fn level(&self) -> u32 {
        self.level
    }

    fn xp(&self) -> f32 {
        self.xp
    }

    fn reset(&mut self) {
        self.xp = 0.0;
        self.level = 1;
    }
}

impl Plugin for AgarDragonPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GameState::default())
            .init_resource::<PumpkinField>()
            .init_resource::<ZombieField>()
            .add_systems(Startup, setup_hud)
            .add_systems(
                Update,
                (
                    handle_restart,
                    ensure_player_state,
                    spawn_pumpkin_field,
                    spawn_zombies,
                    drive_player_movement,
                    collect_pumpkins,
                    update_zombies,
                    update_camera_follow,
                    update_hud,
                ),
            );
    }
}

fn ensure_player_state(
    mut commands: Commands,
    mut q: Query<(Entity, &mut Transform), (With<PlayerDragon>, Without<PlayerState>)>,
) {
    for (entity, mut transform) in q.iter_mut() {
        transform.translation.y = PLAYER_HEIGHT;
        transform.scale = Vec3::splat(PLAYER_BASE_SCALE);
        commands
            .entity(entity)
            .insert(PlayerState { xp: 0.0, level: 1 });
    }
}

fn setup_hud(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = asset_server.load("fonts/NotoSans-Regular.ttf");
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(18.0),
            left: Val::Px(18.0),
            ..default()
        },
        Text::new(format!("Level: 1  XP: 0\n{}", default_instructions())),
        TextFont::from(font).with_font_size(24.0),
        TextColor(Color::WHITE.into()),
        HudRoot,
        HudText,
    ));
}

fn drive_player_movement(
    time: Res<Time>,
    kb: Res<ButtonInput<KeyCode>>,
    game: Res<GameState>,
    mut q: Query<(&mut Transform, &PlayerState), With<PlayerDragon>>,
) {
    if !game.is_playing() {
        return;
    }
    let Ok((mut transform, state)) = q.single_mut() else {
        return;
    };

    let mut dir = Vec3::ZERO;
    if kb.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        dir.z -= 1.0;
    }
    if kb.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        dir.z += 1.0;
    }
    if kb.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        dir.x -= 1.0;
    }
    if kb.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        dir.x += 1.0;
    }

    if dir.length_squared() > 0.0 {
        let dir = dir.normalize();
        let speed =
            PLAYER_BASE_SPEED + PLAYER_SPEED_STEP * ((state.level.saturating_sub(1)) as f32);
        let delta = dir * speed * time.delta_secs();
        transform.translation += Vec3::new(delta.x, 0.0, delta.z);
        transform.translation.x = transform
            .translation
            .x
            .clamp(-FIELD_HALF_EXTENT, FIELD_HALF_EXTENT);
        transform.translation.z = transform
            .translation
            .z
            .clamp(-FIELD_HALF_EXTENT, FIELD_HALF_EXTENT);
    }

    transform.translation.y = PLAYER_HEIGHT;
}

fn spawn_pumpkin_field(
    mut commands: Commands,
    pumpkin_scene: Option<Res<PumpkinScene>>,
    mut field: ResMut<PumpkinField>,
) {
    if field.spawned {
        return;
    }
    let Some(scene) = pumpkin_scene else {
        return;
    };
    let handle = scene.0.clone();
    let total = field.positions.len();
    if total == 0 {
        return;
    }
    let mut spawn_count = field.spawn_count.min(total);
    if spawn_count == 0 {
        spawn_count = total;
    }
    for i in 0..spawn_count {
        let pos = field.positions[i];
        commands.spawn((
            SceneRoot(handle.clone()),
            Transform::from_translation(pos).with_scale(Vec3::splat(0.9)),
            Pumpkin,
        ));
    }
    field.spawn_count = spawn_count;
    field.next_index = spawn_count % field.positions.len();
    field.spawned = true;
}

fn collect_pumpkins(
    mut pumpkins: Query<&mut Transform, With<Pumpkin>>,
    mut player: Query<(&mut Transform, &mut PlayerState), With<PlayerDragon>>,
    mut field: ResMut<PumpkinField>,
    mut game: ResMut<GameState>,
) {
    if !game.is_playing() {
        return;
    }
    let Ok((mut player_transform, mut state)) = player.single_mut() else {
        return;
    };
    let player_pos = player_transform.translation;
    let eat_radius = PLAYER_COLLISION_RADIUS + 0.6 * ((state.level.saturating_sub(1)) as f32);
    let eat_radius_sq = eat_radius * eat_radius;

    for mut pumpkin_transform in pumpkins.iter_mut() {
        let delta = pumpkin_transform.translation - player_pos;
        let dist_sq = delta.x * delta.x + delta.z * delta.z;
        if dist_sq <= eat_radius_sq {
            let next_pos = field.next_position();
            pumpkin_transform.translation = next_pos;

            if state.add_xp(5.0) {
                let scale = scale_for_level(state.level());
                player_transform.scale = Vec3::splat(scale);
                if state.level() >= ZOMBIE_LEVEL_REQUIREMENT {
                    game.set_message("Zombies are on the menu!");
                } else {
                    game.set_message(format!("Level {} reached!", state.level()));
                }
            }
        }
    }

    player_transform.translation.y = PLAYER_HEIGHT;
}

fn update_camera_follow(
    mut cameras: Query<&mut Transform, With<AgarCamera>>,
    player: Query<&Transform, With<PlayerDragon>>,
) {
    let Ok(mut camera_transform) = cameras.single_mut() else {
        return;
    };
    let Ok(player_transform) = player.single() else {
        return;
    };
    let mut target = player_transform.translation;
    target.y = 0.0;
    camera_transform.translation.x = player_transform.translation.x;
    camera_transform.translation.z = player_transform.translation.z;
    camera_transform.translation.y = CAMERA_HEIGHT;
    camera_transform.look_at(player_transform.translation, Vec3::Y);
}

fn spawn_zombies(
    mut commands: Commands,
    zombie_scene: Option<Res<ZombieScene>>,
    mut field: ResMut<ZombieField>,
) {
    if field.spawned {
        return;
    }
    let Some(scene) = zombie_scene else {
        return;
    };
    let handle = scene.0.clone();
    let total = field.positions.len();
    if total == 0 {
        return;
    }
    let mut spawn_count = field.spawn_count.min(total);
    if spawn_count == 0 {
        spawn_count = total;
    }
    for i in 0..spawn_count {
        let pos = field.positions[i];
        commands.spawn((
            SceneRoot(handle.clone()),
            Transform::from_translation(pos).with_scale(Vec3::splat(1.0)),
            Zombie {
                speed: ZOMBIE_BASE_SPEED,
            },
        ));
    }
    field.spawn_count = spawn_count;
    field.next_index = spawn_count % field.positions.len();
    field.spawned = true;
}

fn update_zombies(
    time: Res<Time>,
    mut zombies: Query<(&mut Transform, &Zombie)>,
    mut player: Query<(&mut Transform, &mut PlayerState), With<PlayerDragon>>,
    mut field: ResMut<ZombieField>,
    mut game: ResMut<GameState>,
) {
    if !game.is_playing() {
        return;
    }
    let Ok((mut player_transform, mut state)) = player.single_mut() else {
        return;
    };
    let player_pos = player_transform.translation;

    for (mut transform, zombie) in zombies.iter_mut() {
        let to_player = player_pos - transform.translation;
        let mut planar = Vec3::new(to_player.x, 0.0, to_player.z);
        if planar.length_squared() > f32::EPSILON {
            planar = planar.normalize();
            let delta = planar * zombie.speed * time.delta_secs();
            transform.translation += Vec3::new(delta.x, 0.0, delta.z);
        }
        transform.translation.x = transform
            .translation
            .x
            .clamp(-FIELD_HALF_EXTENT, FIELD_HALF_EXTENT);
        transform.translation.z = transform
            .translation
            .z
            .clamp(-FIELD_HALF_EXTENT, FIELD_HALF_EXTENT);
        transform.translation.y = transform.translation.y.max(0.0);

        let diff = transform.translation - player_pos;
        let dist_sq = diff.x * diff.x + diff.z * diff.z;
        if dist_sq <= ZOMBIE_COLLISION_RADIUS * ZOMBIE_COLLISION_RADIUS {
            if state.level() < ZOMBIE_LEVEL_REQUIREMENT {
                if game.is_playing() {
                    game.set_game_over("Zombie chomped you! Press R to restart.");
                }
                return;
            }

            let next = field.next_position();
            transform.translation = next;
            transform.translation.y = next.y;

            if state.add_xp(ZOMBIE_KO_XP) {
                let scale = scale_for_level(state.level());
                player_transform.scale = Vec3::splat(scale);
                game.set_message("Level up from zombie feast!");
            } else {
                game.set_message("You devoured a zombie!");
            }
        }
    }
}

fn handle_restart(
    kb: Res<ButtonInput<KeyCode>>,
    mut game: ResMut<GameState>,
    mut player: Query<(&mut Transform, &mut PlayerState), With<PlayerDragon>>,
    mut pumpkins: Query<&mut Transform, With<Pumpkin>>,
    mut pumpkin_field: ResMut<PumpkinField>,
    mut zombies: Query<&mut Transform, With<Zombie>>,
    mut zombie_field: ResMut<ZombieField>,
) {
    if game.status != GameStatus::GameOver {
        return;
    }
    if !kb.just_pressed(KeyCode::KeyR)
        && !kb.just_pressed(KeyCode::Enter)
        && !kb.just_pressed(KeyCode::Space)
    {
        return;
    }

    if let Ok((mut transform, mut state)) = player.single_mut() {
        transform.translation = Vec3::new(0.0, PLAYER_HEIGHT, 0.0);
        transform.scale = Vec3::splat(PLAYER_BASE_SCALE);
        state.reset();
    }

    if pumpkin_field.spawned {
        let mut positions = pumpkin_field.positions.iter().cycle();
        for (i, mut transform) in pumpkins.iter_mut().enumerate() {
            if pumpkin_field.spawn_count == 0 {
                break;
            }
            if i >= pumpkin_field.spawn_count {
                break;
            }
            if let Some(pos) = positions.next() {
                transform.translation = *pos;
            }
        }
        pumpkin_field.reset();
    }

    if zombie_field.spawned {
        let mut positions = zombie_field.positions.iter().cycle();
        for (i, mut transform) in zombies.iter_mut().enumerate() {
            if zombie_field.spawn_count == 0 {
                break;
            }
            if i >= zombie_field.spawn_count {
                break;
            }
            if let Some(pos) = positions.next() {
                transform.translation = *pos;
            }
        }
        zombie_field.reset();
    }

    game.reset();
}

fn update_hud(
    mut hud: Query<&mut Text, With<HudText>>,
    player: Query<&PlayerState, With<PlayerDragon>>,
    game: Res<GameState>,
) {
    let Ok(mut text) = hud.single_mut() else {
        return;
    };
    let (level, xp) = match player.single() {
        Ok(state) => (state.level(), state.xp()),
        Err(_) => (1, 0.0),
    };
    let status_prefix = if game.status == GameStatus::GameOver {
        "GAME OVER"
    } else {
        ""
    };
    let mut message = String::new();
    if !status_prefix.is_empty() {
        message.push_str(status_prefix);
        message.push('\n');
    }
    message.push_str(game.message.as_str());
    let content = format!("Level: {}  XP: {:.0}\n{}", level, xp, message);
    if text.as_str() != content {
        text.clear();
        text.push_str(&content);
    }
}

fn level_for_xp(xp: f32) -> u32 {
    if xp >= 25.0 {
        3
    } else if xp >= 10.0 {
        2
    } else {
        1
    }
}

fn scale_for_level(level: u32) -> f32 {
    PLAYER_BASE_SCALE + PLAYER_SCALE_STEP * (level.saturating_sub(1) as f32)
}
