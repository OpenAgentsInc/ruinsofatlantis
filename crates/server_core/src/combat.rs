//! Faction rules and hostility helpers.

use crate::actor::Team;

#[inline]
pub fn are_hostile(a: Team, b: Team) -> bool {
    use Team::*;
    matches!(
        (a, b),
        (Pc, Undead) | (Undead, Pc) | (Wizards, Undead) | (Undead, Wizards)
    )
}

#[derive(Default, Debug, Clone, Copy)]
pub struct FactionState {
    pub pc_vs_wizards_hostile: bool,
}

impl FactionState {
    pub fn effective_hostile(&self, a: Team, b: Team) -> bool {
        use Team::*;
        match (a, b) {
            (Pc, Wizards) | (Wizards, Pc) => self.pc_vs_wizards_hostile,
            _ => are_hostile(a, b),
        }
    }
}
