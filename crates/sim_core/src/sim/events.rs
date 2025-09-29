#[derive(Debug, Clone)]
pub enum SimEvent {
    CastStarted {
        actor: String,
        ability: String,
        cast_ms: u32,
        gcd_ms: u32,
    },
    CastCompleted {
        actor: String,
        ability: String,
    },
    AllyImmunity {
        actor: String,
        target: String,
        ability: String,
    },
    ShieldReaction {
        target: String,
        new_ac: i32,
    },
    AttackResolved {
        actor: String,
        ability: String,
        roll: i32,
        bonus: i32,
        total: i32,
        target_ac: i32,
        hit: bool,
    },
    SaveResolved {
        caster: String,
        target: String,
        ability: String,
        save: String,
        total: i32,
        dc: i32,
        success: bool,
    },
    ConditionApplied {
        target: String,
        condition: String,
        duration_ms: u32,
    },
    TempHpAbsorb {
        target: String,
        absorbed: i32,
        thp_now: i32,
    },
    DamageApplied {
        caster: String,
        target: String,
        ability: String,
        amount: i32,
        hp_before: i32,
        hp_after: i32,
    },
    ConcentrationCheck {
        target: String,
        roll: i32,
        dc: i32,
        keep: bool,
    },
    ConcentrationBroken {
        target: String,
        ability: String,
    },
    BlessApplied {
        caster: String,
        duration_ms: u32,
    },
}
