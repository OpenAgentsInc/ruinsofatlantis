use bevy::prelude::*;
use roa_domain::{DragonTypeId, NpcKind};
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub enum ClipMode {
    RotateAll,
    IdleOnly,
    Named(&'static [&'static str]),
}

#[derive(Clone)]
pub struct NpcVisualArchetype {
    pub gltf_path: String,
    pub scene_index: usize,
    pub scale: Vec3,
    pub clip_mode: ClipMode,
}

#[derive(Resource, Default)]
pub struct NpcVisualRegistry {
    pub by_slug: HashMap<String, NpcVisualArchetype>,
}

impl NpcVisualRegistry {
    pub fn insert(&mut self, slug: impl Into<String>, a: NpcVisualArchetype) {
        self.by_slug.insert(slug.into(), a);
    }
    pub fn get(&self, kind: &NpcKind) -> Option<&NpcVisualArchetype> {
        match kind {
            NpcKind::Dragon(DragonTypeId(t)) => self.by_slug.get(&format!("dragon:{t}")),
        }
    }
}

pub fn register_default_dragons(mut reg: ResMut<NpcVisualRegistry>) {
    reg.insert(
        "dragon:proto_v2",
        NpcVisualArchetype {
            gltf_path: "models/DragonProto_v2.glb".into(),
            scene_index: 0,
            scale: Vec3::splat(1.0),
            clip_mode: ClipMode::RotateAll,
        },
    );
}

pub struct NpcVisualRegistryPlugin;
impl Plugin for NpcVisualRegistryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NpcVisualRegistry>()
            .add_systems(Startup, register_default_dragons);
    }
}
