//! Portable NPC types, kinds, and spawn events (no Bevy meta-crate).
use bevy_ecs::prelude::*;
use bevy_reflect::Reflect;

#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct Npc {
    pub radius: f32,
    pub speed_mps: f32,
    pub damage: i32,
    pub attack_cooldown_s: f32,
}

impl Default for Npc {
    fn default() -> Self {
        Self {
            radius: 1.0,
            speed_mps: 5.0,
            damage: 5,
            attack_cooldown_s: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect)]
pub enum NpcKind {
    Dragon(DragonTypeId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect)]
pub struct DragonTypeId(pub &'static str); // e.g., "proto_v2", "red_wyvern"

#[derive(Event, Message, Debug, Clone, Reflect)]
pub struct SpawnNpc {
    pub kind: NpcKind,
    pub pos: glam::Vec3,
    pub yaw: f32,
    pub tint: Option<glam::Vec3>,
}

pub fn register_npc_domain(app: &mut bevy_ecs::prelude::World) {
    // Register message storage for SpawnNpc in a World-only context
    if !app.contains_resource::<bevy_ecs::message::Messages<SpawnNpc>>() {
        app.insert_resource(bevy_ecs::message::Messages::<SpawnNpc>::default());
    }
}
