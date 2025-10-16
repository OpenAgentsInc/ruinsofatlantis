use crate::{schema::Wish, score::score_wish};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatBreakdown {
    pub total: f32,
    pub scope_factor: f32,
    pub novelty: f32,
    pub speed: f32,
    pub reversibility_penalty: f32,
    pub clarity_penalty: f32,
    pub chain_len_factor: f32,
}

impl HeatBreakdown {
    pub fn new(total: f32) -> Self {
        Self {
            total,
            scope_factor: 1.0,
            novelty: 1.0,
            speed: 1.0,
            reversibility_penalty: 1.0,
            clarity_penalty: 1.0,
            chain_len_factor: 1.0,
        }
    }
}

pub fn estimate_heat(w: &Wish, novelty: f32) -> HeatBreakdown {
    // Default parameters per docs; keep simple for now.
    let scope_factor = match w.tier {
        Some(crate::schema::Tier::Micro) | None => 0.5,
        Some(crate::schema::Tier::Meso) => 1.0,
        Some(crate::schema::Tier::Macro) => 2.0,
    };

    let scores = score_wish(w);
    let clarity_penalty = if scores.clarity >= 80 {
        0.8
    } else if scores.clarity >= 60 {
        1.0
    } else {
        1.2
    };
    let reversibility_penalty = if scores.reversibility >= 80 {
        0.85
    } else if scores.reversibility >= 60 {
        1.0
    } else {
        1.2
    };
    let chain_len_factor = (w.plan.len() as f32).max(1.0).min(5.0) / 3.0; // ~0.33..1.67
    let speed = 1.0; // placeholder: instant vs scheduled
    let base = 10.0; // arbitrary base unit
    let total = base
        * scope_factor
        * novelty
        * speed
        * reversibility_penalty
        * clarity_penalty
        * chain_len_factor;
    HeatBreakdown {
        total,
        scope_factor,
        novelty,
        speed,
        reversibility_penalty,
        clarity_penalty,
        chain_len_factor,
    }
}
