//! Input state used by the basic controller.

#[derive(Default, Debug, Clone, Copy)]
pub struct InputState {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub run: bool, // Shift
}

impl InputState {
    pub fn clear(&mut self) { *self = Self::default(); }
}

