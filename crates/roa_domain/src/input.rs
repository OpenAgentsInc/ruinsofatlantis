//! Input command events for the domain.
use bevy_ecs::prelude::*;
use bevy_reflect::Reflect;

#[derive(Message, Debug, Clone, Reflect)]
pub enum Command {
    /// Local-space axes: x = strafe right (+), y = forward (+)
    MoveAxes {
        x: f32,
        y: f32,
    },
    /// Mouse look deltas in pixels
    LookDelta {
        dx: f32,
        dy: f32,
    },
    /// Flight controls
    Ascend(f32),
    Descend(f32),
    Takeoff,
    Land,
    /// Primary attack intent
    AttackPrimary,
}
