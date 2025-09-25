//! Spell schema (SRD-derived). Keep minimal; loaders will fill from JSON.

use super::ids::Id;

#[derive(Debug, Clone)]
pub struct Spell {
    pub id: Id,
    pub name: String,
    pub level: u8,
    pub school: String,
}

impl Spell {
    pub fn is_cantrip(&self) -> bool { self.level == 0 }
}

