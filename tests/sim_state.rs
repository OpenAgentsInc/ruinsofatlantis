use ruinsofatlantis::sim::state::SimState;

#[test]
fn new_state_defaults() {
    let s = SimState::new(50, 123);
    assert_eq!(s.tick_ms, 50);
    assert!(s.actors.is_empty());
    assert!(s.spells.is_empty());
}

#[test]
fn load_spell_resolution_heuristics() {
    let s = SimState::new(50, 1);
    let spec = s.load_spell("wiz.fire_bolt.srd521").expect("load");
    assert_eq!(spec.name, "Fire Bolt");
    let spec2 = s.load_spell("cleric.bless.srd521").expect("load");
    assert_eq!(spec2.name, "Bless");
}

#[test]
fn load_defaults_class_and_monster() {
    let s = SimState::new(50, 1);
    let (ac, atk, dc) = s.load_class_defaults("wizard").expect("class");
    assert_eq!(ac, 12);
    assert!(atk >= 0);
    assert!(dc >= 0);
    let (ac2, hp) = s.load_monster_defaults("boss_aboleth").expect("monster");
    assert_eq!(ac2, 17);
    assert!(hp > 0);
}

#[test]
fn target_ac_and_allies() {
    use sim_core::combat::fsm::ActionState;
    let mut s = SimState::new(50, 1);
    s.actors.push(ruinsofatlantis::sim::state::ActorSim {
        id: "a".into(),
        role: "dps".into(),
        class: None,
        team: Some("players".into()),
        hp: 10,
        ac_base: 12,
        ac_temp_bonus: 0,
        ability_ids: vec![],
        action: ActionState::Idle,
        gcd: Default::default(),
        target: Some(1),
        char_level: 1,
        spell_attack_bonus: 0,
        spell_save_dc: 10,
        statuses: vec![],
        blessed_ms: 0,
        reaction_ready: true,
        next_ability_idx: 0,
        temp_hp: 0,
        concentration: None,
        ability_cooldowns: std::collections::HashMap::new(),
    });
    s.actors.push(ruinsofatlantis::sim::state::ActorSim {
        id: "b".into(),
        role: "boss".into(),
        class: None,
        team: Some("boss".into()),
        hp: 20,
        ac_base: 15,
        ac_temp_bonus: 0,
        ability_ids: vec![],
        action: ActionState::Idle,
        gcd: Default::default(),
        target: None,
        char_level: 1,
        spell_attack_bonus: 0,
        spell_save_dc: 10,
        statuses: vec![],
        blessed_ms: 0,
        reaction_ready: true,
        next_ability_idx: 0,
        temp_hp: 0,
        concentration: None,
        ability_cooldowns: std::collections::HashMap::new(),
    });
    assert_eq!(s.target_ac(0), Some(15));
    assert!(!s.are_allies(0, 1));
}

#[test]
fn roll_dice_str_parses() {
    let mut s = SimState::new(50, 123);
    let v = s.roll_dice_str("2d6");
    assert!((2..=12).contains(&v));
}

#[test]
fn roll_dice_str_supports_ndm_plus_k() {
    let mut s = SimState::new(50, 999);
    let v = s.roll_dice_str("2d6+3");
    assert!((5..=15).contains(&v));
}

#[test]
fn actor_alive_checks_bounds() {
    let s = SimState::new(50, 1);
    assert!(!s.actor_alive(99));
}

#[test]
fn builtin_specs_present() {
    let basic = SimState::builtin_basic_attack_spec();
    assert_eq!(basic.name, "Basic Attack");
    let tent = SimState::builtin_boss_tentacle_spec();
    assert_eq!(tent.name, "Tentacle");
}
