//! Ability/action finite state machine scaffolding.
//! Tracks casts, channels, recovery, GCD, and reaction windows.

use crate::core::data::ids::Id;

#[derive(Debug, Clone)]
pub enum ActionState {
    Idle,
    Casting { ability: Id, remaining_ms: u32 },
    Channeling { ability: Id, remaining_ms: u32 },
    Recovery { remaining_ms: u32 },
}

#[derive(Debug, Clone, Default)]
pub struct Gcd { pub remaining_ms: u32 }

#[derive(Debug, Clone)]
pub struct ReactionWindow { pub remaining_ms: u32 }

