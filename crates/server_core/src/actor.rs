//! Authoritative actor store and basic types (pre-ECS bridge).

use glam::Vec3;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActorId(pub u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ActorKind {
    Wizard,
    Zombie,
    Boss,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Team {
    Pc,
    Wizards,
    Undead,
    Neutral,
}

/// Preferred terminology in docs and new code paths.
/// Alias `Team` to `Faction` to clarify that this is a component-level
/// allegiance marker, not a separate entity or system. Code may gradually
/// migrate to `Faction` while keeping `Team` for compatibility.
pub use Team as Faction;

#[derive(Copy, Clone, Debug)]
pub struct Health {
    pub hp: i32,
    pub max: i32,
}
impl Health {
    #[inline]
    pub fn alive(&self) -> bool {
        self.hp > 0
    }
    #[inline]
    pub fn clamp(&mut self) {
        if self.hp > self.max {
            self.hp = self.max;
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Transform {
    pub pos: Vec3,
    pub yaw: f32,
    pub radius: f32,
}

#[derive(Copy, Clone, Debug)]
pub struct Actor {
    pub id: ActorId,
    pub kind: ActorKind,
    pub team: Team,
    pub tr: Transform,
    pub hp: Health,
}

// Legacy ActorStore removed; ECS world is authoritative.
