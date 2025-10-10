//! client_runtime: thin client-side scene inputs and updates.
//!
//! This crate decouples controller + collision updates from the renderer.
//! The renderer consumes `SceneInputs` to update the player transform and
//! camera, without owning input semantics or collision policy.

use client_core::controller::PlayerController;
use client_core::input::InputState;
use collision_static::{Aabb, Capsule, StaticIndex};
use glam::Vec3;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct PlayerCameraRig {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub lift: f32,
    pub look_height: f32,
}
impl Default for PlayerCameraRig {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.2,
            distance: 8.5,
            lift: 3.5,
            look_height: 1.6,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SceneInputs {
    controller: PlayerController,
    input: InputState,
    ability: AbilityState,
    cam_rig: PlayerCameraRig,
}

impl SceneInputs {
    pub fn new(initial_pos: Vec3) -> Self {
        Self {
            controller: PlayerController::new(initial_pos),
            input: InputState::default(),
            ability: AbilityState::default(),
            cam_rig: PlayerCameraRig::default(),
        }
    }
    pub fn apply_input(&mut self, input: &InputState) {
        self.input = *input;
    }
    pub fn toggle_autorun(&mut self) {
        self.controller.toggle_autorun();
    }
    pub fn cancel_autorun(&mut self) {
        self.controller.cancel_autorun();
    }
    pub fn toggle_walk(&mut self) {
        self.controller.toggle_walk();
    }
    pub fn pos(&self) -> Vec3 {
        self.controller.pos
    }
    pub fn yaw(&self) -> f32 {
        self.controller.yaw
    }
    /// Explicitly set the controller yaw. Used by mouse-look so renderer and
    /// controller remain in sync when the player drags to rotate.
    pub fn set_yaw(&mut self, yaw: f32) {
        self.controller.yaw = yaw;
    }

    // --- Camera rig helpers (orbit yaw/pitch/distance updates) ---
    pub fn rig_add_yaw(&mut self, dyaw: f32) {
        let mut y = self.cam_rig.yaw + dyaw;
        while y > std::f32::consts::PI {
            y -= std::f32::consts::TAU;
        }
        while y < -std::f32::consts::PI {
            y += std::f32::consts::TAU;
        }
        self.cam_rig.yaw = y;
    }
    pub fn rig_set_yaw(&mut self, yaw: f32) {
        self.cam_rig.yaw = yaw;
    }
    pub fn rig_apply_mouse_orbit(
        &mut self,
        dx: f32,
        dy: f32,
        sens_rad: f32,
        pitch_min: f32,
        pitch_max: f32,
    ) {
        self.rig_add_yaw(-dx * sens_rad);
        let p = (self.cam_rig.pitch + dy * sens_rad).clamp(pitch_min, pitch_max);
        self.cam_rig.pitch = p;
    }
    pub fn rig_zoom(&mut self, step: f32) {
        let d = (self.cam_rig.distance - step).clamp(1.6, 25.0);
        self.cam_rig.distance = d;
    }
    pub fn rig_values(&self) -> (f32, f32, f32, f32, f32) {
        (
            self.cam_rig.yaw,
            self.cam_rig.pitch,
            self.cam_rig.distance,
            self.cam_rig.lift,
            self.cam_rig.look_height,
        )
    }
    pub fn rig_yaw(&self) -> f32 {
        self.cam_rig.yaw
    }
    pub fn rig_pitch(&self) -> f32 {
        self.cam_rig.pitch
    }
    pub fn rig_distance(&self) -> f32 {
        self.cam_rig.distance
    }

    /// True while the controller is off the ground (jumping/falling).
    pub fn airborne(&self) -> bool {
        self.controller.airborne()
    }

    /// Provide the latest ground height from terrain/collision to the controller.
    pub fn set_ground_height(&mut self, h: f32) {
        self.controller.set_ground_height(h);
    }

    /// Advance the client controller and resolve against static colliders (capsule slide).
    /// The Y component is left untouched; caller may project to terrain height separately.
    pub fn update(&mut self, dt: f32, cam_forward: Vec3, static_index: Option<&StaticIndex>) {
        self.controller.update(&self.input, dt, cam_forward);
        if let Some(idx) = static_index {
            let cap = Capsule {
                // WoW-like humanoid: height 1.9m, radius 0.35m → segment from 0.35 to 1.55
                p0: Vec3::new(
                    self.controller.pos.x,
                    self.controller.pos.y + 0.35,
                    self.controller.pos.z,
                ),
                p1: Vec3::new(
                    self.controller.pos.x,
                    self.controller.pos.y + 1.55,
                    self.controller.pos.z,
                ),
                radius: 0.35,
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
                0.45, // step offset (meters)
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
        if end <= now_s || cooldown_s <= 0.0 {
            0.0
        } else {
            ((end - now_s) / cooldown_s).clamp(0.0, 1.0)
        }
    }
}

impl SceneInputs {
    pub fn can_cast(&self, id: &str, now_s: f32) -> bool {
        self.ability.can_cast(id, now_s)
    }
    pub fn start_cooldown(&mut self, id: &str, now_s: f32, cooldown_s: f32) {
        self.ability.start_cooldown(id, now_s, cooldown_s);
    }
    pub fn cooldown_frac(&self, id: &str, now_s: f32, cooldown_s: f32) -> f32 {
        self.ability.cooldown_frac(id, now_s, cooldown_s)
    }
}
