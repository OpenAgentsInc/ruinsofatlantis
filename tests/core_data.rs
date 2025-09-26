use ruinsofatlantis::core::data::loader;
use ruinsofatlantis::core::data::scenario;
use ruinsofatlantis::core::data::spell::{Spell, SpellSpec};

#[test]
fn read_json_fire_bolt() {
    let s = loader::read_json("spells/fire_bolt.json").expect("json");
    assert!(s.contains("Fire Bolt"));
}

#[test]
fn load_spell_spec_fire_bolt() {
    let spec = loader::load_spell_spec("spells/fire_bolt.json").expect("spec");
    assert_eq!(spec.name, "Fire Bolt");
    assert_eq!(spec.level, 0);
    assert_eq!(spec.school.to_ascii_lowercase(), "evocation");
    assert!(spec.attack.is_some());
    assert!(spec.damage.is_some());
}

#[test]
fn load_spell_spec_bless() {
    let spec = loader::load_spell_spec("spells/bless.json").expect("spec");
    assert_eq!(spec.name, "Bless");
    assert_eq!(spec.level, 1);
    assert!(spec.attack.is_none());
    assert!(spec.damage.is_none());
}

#[test]
fn load_spell_spec_shield() {
    let spec = loader::load_spell_spec("spells/shield.json").expect("spec");
    assert_eq!(spec.name, "Shield");
    assert_eq!(spec.level, 1);
    assert!(spec.attack.is_none());
}

#[test]
fn load_spell_spec_grease() {
    let spec = loader::load_spell_spec("spells/grease.json").expect("spec");
    assert_eq!(spec.name, "Grease");
    assert!(spec.save.is_some());
}

#[test]
fn load_class_wizard_defaults() {
    let spec = loader::load_class_spec("classes/wizard.json").expect("class");
    assert_eq!(spec.id, "wizard");
    assert!(spec.save_mods.get("dex").copied().unwrap_or_default() >= 0);
}

#[test]
fn load_monster_boss_defaults() {
    let spec = loader::load_monster_spec("monsters/boss_aboleth.json").expect("monster");
    assert_eq!(spec.id, "boss_aboleth");
    assert!(spec.hp > 0);
}

#[test]
fn scenario_yaml_example_loads() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = root.join("data/scenarios/example.yaml");
    let scn = scenario::load_yaml(&path).expect("yaml");
    assert_eq!(scn.name, "Aboleth Demo");
    assert_eq!(scn.tick_ms, 50);
    assert!(scn.actors.iter().any(|a| a.role == "boss"));
}

#[test]
fn spell_is_cantrip_helper() {
    use ruinsofatlantis::core::data::ids::Id;
    let s = Spell {
        id: Id("test".into()),
        name: "X".into(),
        level: 0,
        school: "evoc".into(),
    };
    assert!(s.is_cantrip());
}

#[test]
fn spell_spec_roundtrip_subset() {
    // Ensure SpellSpec has expected required fields and serde defaults work
    let txt = r#"{
        "id":"x.y",
        "name":"N",
        "school":"illusion",
        "level":1,
        "cast_time_s":1.0,
        "gcd_s":1.0,
        "cooldown_s":0.0,
        "can_move_while_casting":false,
        "targeting":"unit",
        "requires_line_of_sight":true,
        "range_ft":30,
        "minimum_range_ft":0,
        "firing_arc_deg":180
    }"#;
    let spec: SpellSpec = serde_json::from_str(txt).expect("serde");
    assert_eq!(spec.name, "N");
    assert!(spec.classes.is_empty());
    assert!(spec.tags.is_empty());
    assert!(spec.attack.is_none());
    assert!(spec.damage.is_none());
}
