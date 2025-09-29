//! client_runtime: thin client-side scene inputs and updates.
//!
//! This crate decouples controller + collision updates from the renderer.
//! The renderer consumes `SceneInputs` to update the player transform and
//! camera, without owning input semantics or collision policy.

use client_core::controller::PlayerController;
use client_core::input::InputState;
use collision_static::{Aabb, Capsule, StaticIndex};
use std::collections::HashMap;
use glam::Vec3;

#[derive(Debug, Clone)]
pub struct SceneInputs {
    controller: PlayerController,
    input: InputState,
    ability: AbilityState,
}

impl SceneInputs {
    pub fn new(initial_pos: Vec3) -> Self {
        Self { controller: PlayerController::new(initial_pos), input: InputState::default(), ability: AbilityState::default() }
    }
    pub fn apply_input(&mut self, input: &InputState) {
        self.input = *input;
    }
    pub fn pos(&self) -> Vec3 {
        self.controller.pos
    }
    pub fn yaw(&self) -> f32 {
        self.controller.yaw
    }

    /// Advance the client controller and resolve against static colliders (capsule slide).
    /// The Y component is left untouched; caller may project to terrain height separately.
    pub fn update(&mut self, dt: f32, cam_forward: Vec3, static_index: Option<&StaticIndex>) {
        self.controller.update(&self.input, dt, cam_forward);
        if let Some(idx) = static_index {
            let cap = Capsule {
                p0: Vec3::new(
                    self.controller.pos.x,
                    self.controller.pos.y + 0.4,
                    self.controller.pos.z,
                ),
                p1: Vec3::new(
                    self.controller.pos.x,
                    self.controller.pos.y + 1.8,
                    self.controller.pos.z,
                ),
                radius: 0.4,
            };
            let aabb = Aabb {
                min: Vec3::new(
                    cap.p0.x.min(cap.p1.x) - cap.radius,
                    cap.p0.y.min(cap.p1.y) - cap.radius,
                    cap.p0.z.min(cap.p1.z) - cap.radius,
                ),
                max: Vec3::new(
                    cap.p0.x.max(cap.p1.x) + cap.radius,
                    cap.p0.y.max(cap.p1.y) + cap.radius,
                    cap.p0.z.max(cap.p1.z) + cap.radius,
                ),
            };
            let _ = aabb;
            let resolved = collision_static::resolve_slide(
                self.controller.pos,
                self.controller.pos,
                &cap,
                idx,
                0.25,
                4,
            );
            self.controller.pos = resolved;
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct AbilityState {
    cooldown_end_s: HashMap<String, f32>,
}

impl AbilityState {
    pub fn can_cast(&self, id: &str, now_s: f32) -> bool {
        self.cooldown_end_s.get(id).copied().unwrap_or(0.0) <= now_s
    }
    pub fn start_cooldown(&mut self, id: &str, now_s: f32, cooldown_s: f32) {
        let end = now_s + cooldown_s.max(0.0);
        self.cooldown_end_s.insert(id.to_string(), end);
    }
    pub fn cooldown_frac(&self, id: &str, now_s: f32, cooldown_s: f32) -> f32 {
        let end = self.cooldown_end_s.get(id).copied().unwrap_or(0.0);
        if end <= now_s || cooldown_s <= 0.0 { 0.0 } else { ((end - now_s) / cooldown_s).clamp(0.0, 1.0) }
    }
}

impl SceneInputs {
    pub fn can_cast(&self, id: &str, now_s: f32) -> bool { self.ability.can_cast(id, now_s) }
    pub fn start_cooldown(&mut self, id: &str, now_s: f32, cooldown_s: f32) { self.ability.start_cooldown(id, now_s, cooldown_s); }
    pub fn cooldown_frac(&self, id: &str, now_s: f32, cooldown_s: f32) -> f32 { self.ability.cooldown_frac(id, now_s, cooldown_s) }
}
