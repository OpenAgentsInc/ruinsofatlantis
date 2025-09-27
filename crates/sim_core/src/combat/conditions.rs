//! Conditions scaffolding (blinded, charmed, grappled, etc.).

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Condition {
    Blinded,
    Charmed,
    Frightened,
    Grappled,
    Incapacitated,
    Invisible,
    Paralyzed,
    Petrified,
    Poisoned,
    Prone,
    Restrained,
    Stunned,
    Unconscious,
}
