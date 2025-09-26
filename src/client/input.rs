//! Input state and helpers for keyboard control.
//!
//! This captures a small set of keys (WASD + Shift) used by the basic
//! character controller. Platform code should update this state on events,
//! and game code can read it each frame.

#[derive(Default, Debug, Clone, Copy)]
pub struct InputState {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub run: bool, // Shift
}

impl InputState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

