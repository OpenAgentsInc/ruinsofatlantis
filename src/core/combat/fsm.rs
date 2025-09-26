//! Ability/action finite state machine scaffolding.
//! Tracks casts, channels, recovery, GCD, and reaction windows.

use crate::core::data::ids::Id;

/// Simple action FSM handling cast/channel/recovery and a separate GCD budget.

#[derive(Debug, Clone)]
pub enum ActionState {
    Idle,
    Casting { ability: Id, remaining_ms: u32 },
    Channeling { ability: Id, remaining_ms: u32 },
    Recovery { remaining_ms: u32 },
}

#[derive(Debug, Clone, Default)]
pub struct Gcd {
    pub remaining_ms: u32,
}

#[derive(Debug, Clone)]
pub struct ReactionWindow {
    pub remaining_ms: u32,
}

impl Default for ActionState {
    fn default() -> Self {
        ActionState::Idle
    }
}

impl ActionState {
    /// Advance timers by `dt_ms`, returning the updated state and any completion flag.
    pub fn tick(self, dt_ms: u32) -> (Self, Option<ActionDone>) {
        match self {
            ActionState::Idle => (self, None),
            ActionState::Casting {
                ability,
                remaining_ms,
            } => {
                let new = remaining_ms.saturating_sub(dt_ms);
                if new == 0 {
                    (
                        ActionState::Recovery { remaining_ms: 0 },
                        Some(ActionDone::CastCompleted { ability }),
                    )
                } else {
                    (
                        ActionState::Casting {
                            ability,
                            remaining_ms: new,
                        },
                        None,
                    )
                }
            }
            ActionState::Channeling {
                ability,
                remaining_ms,
            } => {
                let new = remaining_ms.saturating_sub(dt_ms);
                if new == 0 {
                    (
                        ActionState::Recovery { remaining_ms: 0 },
                        Some(ActionDone::ChannelCompleted { ability }),
                    )
                } else {
                    (
                        ActionState::Channeling {
                            ability,
                            remaining_ms: new,
                        },
                        None,
                    )
                }
            }
            ActionState::Recovery { remaining_ms } => {
                let new = remaining_ms.saturating_sub(dt_ms);
                if new == 0 {
                    (ActionState::Idle, None)
                } else {
                    (ActionState::Recovery { remaining_ms: new }, None)
                }
            }
        }
    }

    /// Begin a cast if idle and GCD allows.
    pub fn try_start_cast(
        self,
        ability: Id,
        cast_time_ms: u32,
        gcd: &mut Gcd,
        gcd_ms: u32,
    ) -> Result<Self, StartError> {
        if gcd.remaining_ms > 0 {
            return Err(StartError::GcdLocked);
        }
        match self {
            ActionState::Idle => {
                gcd.remaining_ms = gcd_ms;
                Ok(ActionState::Casting {
                    ability,
                    remaining_ms: cast_time_ms,
                })
            }
            _ => Err(StartError::Busy),
        }
    }
}

impl Gcd {
    pub fn tick(&mut self, dt_ms: u32) {
        self.remaining_ms = self.remaining_ms.saturating_sub(dt_ms);
    }
    pub fn ready(&self) -> bool {
        self.remaining_ms == 0
    }
}

impl ReactionWindow {
    pub fn tick(&mut self, dt_ms: u32) {
        self.remaining_ms = self.remaining_ms.saturating_sub(dt_ms);
    }
    pub fn open(&self) -> bool {
        self.remaining_ms > 0
    }
}

#[derive(Debug, Clone)]
pub enum ActionDone {
    CastCompleted { ability: Id },
    ChannelCompleted { ability: Id },
}

#[derive(Debug, Clone)]
pub enum StartError {
    Busy,
    GcdLocked,
}
