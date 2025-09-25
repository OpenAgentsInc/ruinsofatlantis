//! Typed events for the simulation bus.

#[derive(Debug, Clone)]
pub enum Event {
    CastStarted,
    CastCompleted,
    ProjectileSpawned,
    HitResolved,
    DamageApplied,
    ObjectIgnited,
}

