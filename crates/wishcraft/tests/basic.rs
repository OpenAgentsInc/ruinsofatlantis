use wishcraft::{
    estimate_heat, lint_wish,
    schema::{Budget, Scope, Wish},
    score_wish,
};

struct AllowAll;
impl wishcraft::GenieRegistry for AllowAll {
    fn get(&self, id: &str) -> Option<wishcraft::GenieCapability> {
        Some(wishcraft::GenieCapability {
            id: id.to_string(),
            persona: wishcraft::Persona::Literalist,
            allowed: true,
        })
    }
}

fn demo_wish() -> Wish {
    Wish {
        title: "Demo".into(),
        objective: "Reduce incidents by 10% in 7 days".into(),
        scope: Scope {
            region: "demo".into(),
            duration_days: 7,
        },
        invariants: vec!["No civilian harm".into()],
        budget: Budget {
            chrono_sand: 1,
            genie_slots: 1,
            gold_cap: 100,
        },
        tools: vec!["Weather.StormOracle".into()],
        plan: vec!["Map corridors".into(), "Coordinate windows".into()],
        safety_tests: vec!["Stress test".into()],
        rollback: vec!["Revert routes".into()],
        tier: Some(wishcraft::Tier::Micro),
        meta: Default::default(),
    }
}

#[test]
fn lint_score_heat_basics() {
    let w = demo_wish();
    let lint = lint_wish(&w, &AllowAll);
    assert!(lint.ok(), "lint errors: {:?}", lint.errors);
    let s = score_wish(&w);
    assert!(s.clarity >= 60 && s.safety >= 50 && s.reversibility >= 50);
    let h = estimate_heat(&w, 1.0);
    assert!(h.total > 0.0);
}
