use bevy::prelude::*;
use bevy_animation::graph::{AnimationGraph, AnimationNodeIndex};
use bevy_animation::prelude::{AnimationGraphHandle, AnimationPlayer};
use bevy_gltf::{Gltf, GltfAssetLabel};
use roa_domain::{Npc, SpawnNpc};

use crate::npc_registry::{ClipMode, NpcVisualRegistry};

#[derive(Component)]
pub struct NpcRoot;
#[derive(Component)]
pub struct DragonAnimSeq {
    pub nodes: Vec<AnimationNodeIndex>,
    pub idx: usize,
}
#[derive(Component)]
pub struct DragonAnimController;

pub fn sys_spawn_npc_requests(
    mut commands: Commands,
    assets: Res<AssetServer>,
    reg: Res<NpcVisualRegistry>,
    mut ev_spawn: bevy::ecs::message::MessageReader<SpawnNpc>,
) {
    for req in ev_spawn.read() {
        let Some(arch) = reg.get(&req.kind) else {
            continue;
        };
        let path = arch.gltf_path.clone();
        let scene = assets.load(GltfAssetLabel::Scene(arch.scene_index).from_asset(path));
        let mut e = commands.spawn((
            NpcRoot,
            Transform::from_translation(Vec3::new(req.pos.x, req.pos.y, req.pos.z))
                .with_rotation(Quat::from_rotation_y(req.yaw))
                .with_scale(arch.scale),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
        ));
        e.insert(Npc {
            radius: 1.5,
            speed_mps: 8.0,
            damage: 10,
            attack_cooldown_s: 1.0,
        });
        e.with_children(|c| {
            c.spawn(SceneRoot(scene));
        });
    }
}

pub fn sys_prepare_npc_animation(
    mut commands: Commands,
    reg: Res<NpcVisualRegistry>,
    gltfs: Res<Assets<Gltf>>,
    assets: Res<AssetServer>,
    q_root: Query<(Entity, &Children, &Transform), (With<NpcRoot>, Without<AnimationGraphHandle>)>,
    q_children: Query<&Children>,
    q_players: Query<Entity, With<AnimationPlayer>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    for (root, kids, _xf) in q_root.iter() {
        // For now, assume proto_v2 archetype
        let arch = reg.by_slug.get("dragon:proto_v2");
        if arch.is_none() {
            continue;
        }
        let arch = arch.unwrap();
        let Some(gltf) = gltfs.get(&assets.load::<Gltf>(arch.gltf_path.clone())) else {
            continue;
        };

        let mut graph = AnimationGraph::new();
        let mut seq = Vec::<AnimationNodeIndex>::new();
        match arch.clip_mode {
            ClipMode::RotateAll => {
                for h in gltf.animations.iter() {
                    seq.push(graph.add_clip(h.clone(), 1.0, graph.root));
                }
            }
            ClipMode::IdleOnly => {
                if let Some(h) = gltf.animations.get(0) {
                    seq.push(graph.add_clip(h.clone(), 1.0, graph.root));
                }
            }
            ClipMode::Named(_names) => {
                for h in gltf.animations.iter() {
                    seq.push(graph.add_clip(h.clone(), 1.0, graph.root));
                }
            }
        }
        let ghandle = graphs.add(graph);

        // Find a GLTF-provided AnimationPlayer under this root
        let mut stack: Vec<Entity> = Vec::new();
        for child in kids.iter() {
            stack.push(child);
        }
        let mut player_ent: Option<Entity> = None;
        while let Some(e) = stack.pop() {
            if q_players.get(e).is_ok() {
                player_ent = Some(e);
                break;
            }
            if let Ok(ch) = q_children.get(e) {
                for c in ch.iter() {
                    stack.push(c);
                }
            }
        }
        let target = player_ent.unwrap_or(root);
        let mut ecmd = commands.entity(target);
        if q_players.get(target).is_err() {
            ecmd.insert(AnimationPlayer::default());
        }
        ecmd.insert(AnimationGraphHandle(ghandle))
            .insert(DragonAnimSeq { nodes: seq, idx: 0 })
            .insert(DragonAnimController);
    }
}

pub fn sys_cycle_animation(
    mut q: Query<(&mut AnimationPlayer, &mut DragonAnimSeq), With<DragonAnimController>>,
) {
    for (mut player, mut seq) in &mut q {
        if seq.nodes.is_empty() {
            continue;
        }
        let cur = seq.nodes[seq.idx];
        if player
            .animation(cur)
            .map(|a| a.is_finished())
            .unwrap_or(true)
        {
            seq.idx = (seq.idx + 1) % seq.nodes.len();
            let next = seq.nodes[seq.idx];
            player.stop_all();
            player.start(next);
        }
    }
}

pub struct NpcSpawnPlugin;
impl Plugin for NpcSpawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, sys_spawn_npc_requests)
            .add_systems(Update, (sys_prepare_npc_animation, sys_cycle_animation));
    }
}
