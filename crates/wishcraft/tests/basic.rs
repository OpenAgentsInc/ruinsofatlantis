use wishcraft::{Budget, Scope, Wish, estimate_heat, lint_wish, score_wish};

struct AllowAll;
impl wishcraft::conduit::ConduitRegistry for AllowAll {
    fn get(&self, id: &str) -> Option<wishcraft::conduit::ConduitDescriptor> {
        Some(wishcraft::conduit::ConduitDescriptor {
            id: id.to_string(),
            label: id.to_string(),
            disposition: wishcraft::conduit::Disposition::Literalist,
            domains: vec![],
            scopes: vec![],
            cost_profile: Default::default(),
            risk_class: wishcraft::conduit::RiskClass::Low,
            determinism: wishcraft::conduit::Determinism::Deterministic,
            latency_class: wishcraft::conduit::Latency::Instant,
            permissions: vec![],
            limits: Default::default(),
            audit_fields: vec![],
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
