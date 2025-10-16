//! Heat must scale with tier and conduit risk/determinism.
//! These are relative assertions to keep balance flexible.

use wishcraft::conduit::{ConduitDescriptor, ConduitRegistry, Determinism, Disposition, RiskClass};
use wishcraft::heat::estimate_heat;
use wishcraft::schema::{Budget, Scope, Tier, Wish};

struct Registry {
    low: ConduitDescriptor,
    med: ConduitDescriptor,
    high: ConduitDescriptor,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            low: ConduitDescriptor {
                id: "low.safe".into(),
                label: "Low".into(),
                risk_class: RiskClass::Low,
                determinism: Determinism::Deterministic,
                ..Default::default()
            },
            med: ConduitDescriptor {
                id: "openai.codex.v2025.plan".into(),
                label: "Plan".into(),
                risk_class: RiskClass::Medium,
                determinism: Determinism::Stochastic,
                disposition: Disposition::Literalist,
                ..Default::default()
            },
            high: ConduitDescriptor {
                id: "apply.pr".into(),
                label: "Apply PR".into(),
                risk_class: RiskClass::High,
                determinism: Determinism::Stochastic,
                ..Default::default()
            },
        }
    }
}
impl ConduitRegistry for Registry {
    fn get(&self, id: &str) -> Option<ConduitDescriptor> {
        match id {
            "low.safe" => Some(self.low.clone()),
            "openai.codex.v2025.plan" => Some(self.med.clone()),
            "apply.pr" => Some(self.high.clone()),
            _ => None,
        }
    }
}

fn wish(tool: &str, tier: Tier) -> Wish {
    Wish {
        title: "H".into(),
        objective: "Numbered objective 10%".into(),
        scope: Scope {
            region: "demo".into(),
            duration_days: 7,
        },
        invariants: vec!["No civilian harm".into()],
        budget: Budget {
            chrono_sand: 1,
            genie_slots: 1,
            gold_cap: 0,
        },
        tools: vec![tool.into()],
        plan: vec!["step1".into(), "step2".into()],
        safety_tests: vec!["sim".into()],
        rollback: vec!["revert".into()],
        tier: Some(tier),
        meta: Default::default(),
    }
}

#[test]
fn heat_orders_by_risk_class_and_determinism() {
    let _reg = Registry::default();
    let low = estimate_heat(&wish("low.safe", Tier::Micro), 1.0);
    let med = estimate_heat(&wish("openai.codex.v2025.plan", Tier::Micro), 1.0);
    let high = estimate_heat(&wish("apply.pr", Tier::Micro), 1.0);
    assert!(
        low.total < med.total && med.total < high.total,
        "expected Low < Medium < High, got: low={} med={} high={}",
        low.total,
        med.total,
        high.total
    );
}

#[test]
fn macro_tier_increases_heat_vs_micro() {
    let _reg = Registry::default();
    let micro = estimate_heat(&wish("openai.codex.v2025.plan", Tier::Micro), 1.0);
    let macro_ = estimate_heat(&wish("openai.codex.v2025.plan", Tier::Macro), 1.0);
    assert!(
        macro_.total > micro.total,
        "Macro tier should increase Heat"
    );
}
