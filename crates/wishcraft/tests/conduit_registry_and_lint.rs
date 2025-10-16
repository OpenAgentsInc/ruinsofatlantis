//! Contract tests for Conduit registry + lint integration.

use pretty_assertions::assert_eq;
use wishcraft::conduit::{ConduitDescriptor, ConduitRegistry, Disposition};
use wishcraft::schema::{Budget, Scope, Tier, Wish};

struct AllowOpenAIPlan;
impl ConduitRegistry for AllowOpenAIPlan {
    fn get(&self, id: &str) -> Option<ConduitDescriptor> {
        if id == "openai.codex.v2025.plan" {
            Some(ConduitDescriptor {
                id: "openai.codex.v2025.plan".into(),
                label: "OpenAI Codex (Plan Builder)".into(),
                disposition: Disposition::Literalist,
                ..Default::default()
            })
        } else {
            None
        }
    }
}

fn demo_wish(tool: &str) -> Wish {
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
            gold_cap: 0,
        },
        tools: vec![tool.into()],
        plan: vec!["Generate plan".into()],
        safety_tests: vec!["Simulate effect".into()],
        rollback: vec!["Revert routes".into()],
        tier: Some(Tier::Micro),
        meta: Default::default(),
    }
}

#[test]
fn lint_allows_present_conduit() {
    let w = demo_wish("openai.codex.v2025.plan");
    let rep = wishcraft::lint_wish(&w, &AllowOpenAIPlan);
    assert!(rep.ok(), "expected ok lint, got errors: {:?}", rep.errors);
}

struct EmptyRegistry;
impl ConduitRegistry for EmptyRegistry {
    fn get(&self, _id: &str) -> Option<ConduitDescriptor> {
        None
    }
}

#[test]
fn lint_rejects_unknown_conduit() {
    let w = demo_wish("no.such.conduit");
    let rep = wishcraft::lint_wish(&w, &EmptyRegistry);
    assert!(!rep.ok(), "expected lint failure");
    assert!(
        rep.errors
            .iter()
            .any(|e| e.contains("disallowed") || e.contains("unknown")),
        "expected 'unknown/not allowed' error, got: {:?}",
        rep.errors
    );
}

#[test]
fn ambiguity_warning_triggers_on_pronouns() {
    let mut w = demo_wish("openai.codex.v2025.plan");
    w.objective = "Fix it".into(); // intentionally ambiguous
    let rep = wishcraft::lint_wish(&w, &AllowOpenAIPlan);
    assert!(rep.ok(), "warnings should not fail lint");
    assert!(
        rep.warnings
            .iter()
            .any(|w| w.to_lowercase().contains("ambiguous"))
    );
}
