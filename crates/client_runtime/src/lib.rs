//! client_runtime: thin client-side scene inputs and updates.
//!
//! This crate decouples controller + collision updates from the renderer.
//! The renderer consumes `SceneInputs` to update the player transform and
//! camera, without owning input semantics or collision policy.

use client_core::controller::PlayerController;
use client_core::input::InputState;
use collision_static::{Aabb, Capsule, StaticIndex};
use glam::Vec3;

#[derive(Debug, Clone)]
pub struct SceneInputs {
    controller: PlayerController,
    input: InputState,
}

impl SceneInputs {
    pub fn new(initial_pos: Vec3) -> Self {
        Self {
            controller: PlayerController::new(initial_pos),
            input: InputState::default(),
        }
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
