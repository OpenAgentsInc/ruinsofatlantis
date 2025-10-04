//! Map button presses to `InputCommand` events when in appropriate mode/profile.

use crate::facade::controller::{ControllerState, InputQueue};
use ecs_core::components::{ControllerMode, InputCommand};

#[derive(Default, Clone, Copy, Debug)]
pub struct ButtonSnapshot {
    pub lmb_pressed: bool,
    pub rmb_pressed: bool,
    pub q_pressed: bool,
    pub e_pressed: bool,
    pub r_pressed: bool,
    pub shift_pressed: bool,
    pub tab_pressed: bool,
}

#[derive(Clone, Debug)]
pub struct Bindings {
    pub lmb: InputCommand,
    pub rmb: InputCommand,
    pub q: InputCommand,
    pub e: InputCommand,
    pub r: InputCommand,
    pub shift: InputCommand,
    pub tab: InputCommand,
}

impl Default for Bindings {
    fn default() -> Self {
        Self {
            lmb: InputCommand::AtWillLMB,
            rmb: InputCommand::AtWillRMB,
            q: InputCommand::EncounterQ,
            e: InputCommand::EncounterE,
            r: InputCommand::EncounterR,
            shift: InputCommand::Dodge,
            tab: InputCommand::ClassMechanic,
        }
    }
}

pub fn handle_buttons(
    binds: &Bindings,
    state: &ControllerState,
    input: &ButtonSnapshot,
    out: &mut InputQueue,
) {
    let in_action = matches!(state.mode, ControllerMode::Mouselook);
    if !in_action {
        return;
    }
    if input.lmb_pressed {
        out.push(binds.lmb.clone());
    }
    if input.rmb_pressed {
        out.push(binds.rmb.clone());
    }
    if input.q_pressed {
        out.push(binds.q.clone());
    }
    if input.e_pressed {
        out.push(binds.e.clone());
    }
    if input.r_pressed {
        out.push(binds.r.clone());
    }
    if input.shift_pressed {
        out.push(binds.shift.clone());
    }
    if input.tab_pressed {
        out.push(binds.tab.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pressed_buttons_emit_commands_in_mouselook() {
        let binds = Bindings::default();
        let mut state = ControllerState::default();
        state.profile = InputProfile::ActionCombat;
        state.mode = ControllerMode::Mouselook;
        let input = ButtonSnapshot {
            lmb_pressed: true,
            q_pressed: true,
            shift_pressed: true,
            ..Default::default()
        };
        let mut out = InputQueue::default();
        handle_buttons(&binds, &state, &input, &mut out);
        let cmds: Vec<_> = out.drain().collect();
        assert!(cmds.contains(&InputCommand::AtWillLMB));
        assert!(cmds.contains(&InputCommand::EncounterQ));
        assert!(cmds.contains(&InputCommand::Dodge));
    }
}
