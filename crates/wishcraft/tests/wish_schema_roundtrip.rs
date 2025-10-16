//! Schema YAML/JSON round-trip and required fields.

use pretty_assertions::assert_eq;
use wishcraft::schema::Wish;

#[test]
fn yaml_roundtrip_preserves_core_fields() {
    let yaml = r#"
title: "Stabilize the Western Sea Lanes"
objective: "Reduce pirate attacks by 40% over 14 days without increasing naval casualties."
scope: { region: "Western Sea", duration_days: 14 }
invariants: ["No increase in civilian deaths", "Trade price index delta <= 1%"]
budget: { chrono_sand: 3, genie_slots: 4, gold_cap: 10000 }
tools: ["openai.codex.v2025.plan"]
plan: ["Map corridors", "Coordinate convoy windows", "Broker ceasefire"]
safety_tests: ["Sim displacement effects", "Stress test storms"]
rollback: ["Dissolve oaths", "Revert patrol routes", "Publish notice"]
"#;

    let wish: Wish = serde_yaml::from_str(yaml).expect("parse");
    assert_eq!(wish.title, "Stabilize the Western Sea Lanes");
    assert!(wish.plan.len() >= 3);
    assert!(wish.rollback.len() >= 1);

    // JSON roundtrip
    let json = serde_json::to_string(&wish).unwrap();
    let wish2: Wish = serde_json::from_str(&json).unwrap();
    assert_eq!(wish, wish2);
}
