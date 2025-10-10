use super::*;

#[test]
fn scene_roundtrip_preserves_spawns() {
    // Build a small scene doc in memory
    let doc = SceneDoc {
        version: "1.0.0".into(),
        seed: 0,
        layers: vec![],
        instances: vec![],
        logic: SceneLogic {
            triggers: vec![],
            spawns: vec![SpawnMarker {
                id: "m0001".into(),
                kind: "tree.default".into(),
                pos: [1.0, 0.0, -2.0],
                yaw_deg: 270.0,
                tags: vec!["wave1".into()],
            }],
            waypoints: vec![],
            links: vec![],
        },
    };
    let s = serde_json::to_string_pretty(&doc).expect("serialize");
    let parsed: SceneDoc = serde_json::from_str(&s).expect("parse");
    assert_eq!(parsed.logic.spawns.len(), 1);
    assert_eq!(parsed.logic.spawns[0].kind, "tree.default");
    assert!((parsed.logic.spawns[0].pos[0] - 1.0).abs() < 1e-6);
}
