//! Dice helpers; deterministic at call sites via injected RNG.

pub enum CritRule {
    None,
    Nat20DoubleDice,
}
