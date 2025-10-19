//! Minimal character/dragon controller components and systems.
use bevy_ecs::prelude::*;
use bevy_reflect::Reflect;

use crate::Command;

#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct DragonController {
    pub speed_fwd: f32,
    pub speed_strafe: f32,
    pub speed_up: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for DragonController {
    fn default() -> Self {
        Self {
            speed_fwd: 12.0,
            speed_strafe: 8.0,
            speed_up: 8.0,
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

#[derive(Component, Reflect, Debug, Default, Clone, Copy)]
pub struct TransformState {
    pub pos: glam::Vec3,
    /// yaw (x), pitch (y), roll (z) radians
    pub rot_yaw_pitch_roll: glam::Vec3,
}

/// Apply high-level input commands to the controller and transform state.
pub fn sys_apply_commands_to_controller(
    mut q: Query<(&mut DragonController, &mut TransformState)>,
    mut ev: MessageReader<Command>,
    sim: Res<crate::SimTime>,
) {
    let dt = sim.dt;
    for (mut ctrl, mut tf) in q.iter_mut() {
        for e in ev.read() {
            match *e {
                Command::MoveAxes { x, y } => {
                    let yaw = ctrl.yaw;
                    let fwd = glam::Vec3::new(yaw.sin(), 0.0, -yaw.cos());
                    let right = glam::Vec3::new(fwd.z, 0.0, -fwd.x);
                    tf.pos += (fwd * y * ctrl.speed_fwd + right * x * ctrl.speed_strafe) * dt;
                }
                Command::LookDelta { dx, dy } => {
                    ctrl.yaw += dx * 0.002;
                    ctrl.pitch = (ctrl.pitch + dy * 0.002).clamp(-1.2, 1.2);
                    tf.rot_yaw_pitch_roll = glam::vec3(ctrl.yaw, ctrl.pitch, 0.0);
                }
                Command::Ascend(a) => {
                    tf.pos.y += a * ctrl.speed_up * dt;
                }
                Command::Descend(a) => {
                    tf.pos.y -= a * ctrl.speed_up * dt;
                }
                Command::Takeoff => {}
                Command::Land => {}
                Command::AttackPrimary => {}
            }
        }
    }
}
