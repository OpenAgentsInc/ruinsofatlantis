//! Ability schema (umbrella over spells and non-spell actions).
//! Minimal placeholder; extend as sim/client/server integrate.

use super::ids::Id;

#[derive(Debug, Clone)]
pub struct AbilityRef {
    pub id: Id,
}
