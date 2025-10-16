use crate::schema::Wish;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scores {
    pub clarity: u8,
    pub safety: u8,
    pub reversibility: u8,
}

fn clamp01(x: i32) -> i32 {
    x.max(0).min(100)
}

pub fn score_wish(w: &Wish) -> Scores {
    // Extremely lightweight heuristics for a skeleton implementation.
    let mut clarity = 50;
    if w.objective.chars().any(|c| c.is_ascii_digit()) {
        clarity += 15;
    }
    if w.plan.len() >= 2 {
        clarity += 10;
    }
    if w.objective.len() > 120 {
        clarity -= 10; // long, likely rambling
    }

    let mut safety = 50;
    safety += (w.invariants.len() as i32 * 4).min(20);
    if w.invariants
        .iter()
        .any(|s| s.to_ascii_lowercase().contains("civilian"))
    {
        safety += 5;
    }

    let mut reversibility = 50;
    reversibility += (w.rollback.len() as i32 * 5).min(25);

    Scores {
        clarity: clamp01(clarity) as u8,
        safety: clamp01(safety) as u8,
        reversibility: clamp01(reversibility) as u8,
    }
}
