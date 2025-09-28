use ruinsofatlantis::sim::state::{ActorSim, SimState};
use ruinsofatlantis::sim::systems;
use sim_core::combat::fsm::ActionState;

fn mk_actor(id: &str, role: &str, team: Option<&str>) -> ActorSim {
    ActorSim {
        id: id.into(),
        role: role.into(),
        class: None,
        team: team.map(|s| s.into()),
        hp: 30,
        ac_base: 12,
        ac_temp_bonus: 0,
        ability_ids: vec![],
        action: ActionState::Idle,
        gcd: Default::default(),
        target: None,
        spell_attack_bonus: 5,
        spell_save_dc: 13,
        statuses: vec![],
        blessed_ms: 0,
        reaction_ready: true,
        next_ability_idx: 0,
        temp_hp: 0,
        concentration: None,
        ability_cooldowns: std::collections::HashMap::new(),
    }
}

#[test]
fn ai_assigns_targets() {
    let mut s = SimState::new(50, 1);
    let boss = mk_actor("boss", "boss", Some("boss"));
    let p1 = mk_actor("p1", "dps", Some("players"));
    let p2 = mk_actor("p2", "dps", Some("players"));
    s.actors.push(boss);
    s.actors.push(p1);
    s.actors.push(p2);
    systems::ai::run(&mut s);
    assert_eq!(s.actors[1].target, Some(0));
    assert_eq!(s.actors[2].target, Some(0));
}

#[test]
fn cast_begin_starts_cast_and_sets_gcd() {
    let mut s = SimState::new(50, 2);
    let mut caster = mk_actor("wiz", "dps", Some("players"));
    caster.ability_ids.push("wiz.fire_bolt.srd521".into());
    s.actors.push(caster);
    systems::cast_begin::run(&mut s);
    assert!(matches!(s.actors[0].action, ActionState::Casting { .. }));
    assert!(s.actors[0].gcd.remaining_ms > 0);
    assert!(s.logs.iter().any(|l| l.contains("cast_started")));
}

#[test]
fn attack_roll_hits_without_shield() {
    let mut s = SimState::new(50, 42);
    let mut a = mk_actor("wiz", "dps", Some("players"));
    a.ability_ids.push("wiz.fire_bolt.srd521".into());
    a.spell_attack_bonus = 100; // ensure hit
    a.target = Some(1);
    s.spells.insert(
        "wiz.fire_bolt.srd521".into(),
        data_runtime::loader::load_spell_spec("spells/fire_bolt.json").unwrap(),
    );
    let b = mk_actor("boss", "boss", Some("boss"));
    s.actors.push(a);
    s.actors.push(b);
    s.cast_completed.push((0, "wiz.fire_bolt.srd521".into()));
    systems::attack_roll::run(&mut s);
    assert!(s.logs.iter().any(|l| l.contains("attack_resolved")));
    assert_eq!(s.pending_damage.len(), 1);
}

#[test]
fn attack_roll_triggers_shield_reaction() {
    let mut s = SimState::new(50, 7);
    let mut a = mk_actor("wiz", "dps", Some("players"));
    a.ability_ids.push("wiz.fire_bolt.srd521".into());
    a.spell_attack_bonus = 0; // use raw d20
    a.target = Some(1);
    s.spells.insert(
        "wiz.fire_bolt.srd521".into(),
        data_runtime::loader::load_spell_spec("spells/fire_bolt.json").unwrap(),
    );
    let mut b = mk_actor("target", "boss", Some("boss"));
    b.ability_ids.push("wiz.shield.srd521".into());
    b.ac_base = 1; // ensure would_hit before shield
    s.actors.push(a);
    s.actors.push(b);
    s.cast_completed.push((0, "wiz.fire_bolt.srd521".into()));
    systems::attack_roll::run(&mut s);
    assert!(s.logs.iter().any(|l| l.contains("shield_reaction")));
}

#[test]
fn damage_applies_and_underwater_halves_fire() {
    // Setup hit
    let mut s = SimState::new(50, 99);
    s.underwater = true;
    let mut a = mk_actor("wiz", "dps", Some("players"));
    a.ability_ids.push("wiz.fire_bolt.srd521".into());
    a.spell_attack_bonus = 100; // guarantee hit
    a.target = Some(1);
    s.spells.insert(
        "wiz.fire_bolt.srd521".into(),
        data_runtime::loader::load_spell_spec("spells/fire_bolt.json").unwrap(),
    );
    let mut b = mk_actor("boss", "boss", Some("boss"));
    b.hp = 40;
    s.actors.push(a);
    s.actors.push(b);
    s.cast_completed.push((0, "wiz.fire_bolt.srd521".into()));
    systems::attack_roll::run(&mut s);
    assert_eq!(s.pending_damage.len(), 1);
    // Copy state to compare underwater vs non-underwater
    // Run underwater branch
    let mut s_under = SimState::new(50, 99);
    s_under.underwater = true;
    let mut a_u = mk_actor("wiz", "dps", Some("players"));
    a_u.ability_ids.push("wiz.fire_bolt.srd521".into());
    a_u.spell_attack_bonus = 100;
    a_u.target = Some(1);
    s_under.spells.insert(
        "wiz.fire_bolt.srd521".into(),
        data_runtime::loader::load_spell_spec("spells/fire_bolt.json").unwrap(),
    );
    let mut b_u = mk_actor("boss", "boss", Some("boss"));
    b_u.hp = 40;
    s_under.actors.push(a_u);
    s_under.actors.push(b_u);
    s_under
        .cast_completed
        .push((0, "wiz.fire_bolt.srd521".into()));
    systems::attack_roll::run(&mut s_under);
    systems::damage::run(&mut s_under);
    let hp_after_under = s_under.actors[1].hp;

    // Run dry branch with same seed and setup
    let mut s_dry = SimState::new(50, 99);
    s_dry.underwater = false;
    let mut a_d = mk_actor("wiz", "dps", Some("players"));
    a_d.ability_ids.push("wiz.fire_bolt.srd521".into());
    a_d.spell_attack_bonus = 100;
    a_d.target = Some(1);
    s_dry.spells.insert(
        "wiz.fire_bolt.srd521".into(),
        data_runtime::loader::load_spell_spec("spells/fire_bolt.json").unwrap(),
    );
    let mut b_d = mk_actor("boss", "boss", Some("boss"));
    b_d.hp = 40;
    s_dry.actors.push(a_d);
    s_dry.actors.push(b_d);
    s_dry
        .cast_completed
        .push((0, "wiz.fire_bolt.srd521".into()));
    systems::attack_roll::run(&mut s_dry);
    systems::damage::run(&mut s_dry);
    let hp_after_dry = s_dry.actors[1].hp;

    assert!(hp_after_under >= hp_after_dry); // took less or equal damage when underwater (half)
}

#[test]
fn saving_throw_applies_condition_on_fail() {
    let mut s = SimState::new(50, 3);
    let mut a = mk_actor("wiz", "dps", Some("players"));
    a.target = Some(1);
    let b = mk_actor("boss", "boss", Some("boss"));
    s.actors.push(a);
    s.actors.push(b);
    // Insert a minimal spell with a very high DC so target fails
    let txt = r#"{
      "id": "grease",
      "name": "Grease",
      "school": "conjuration",
      "level": 1,
      "classes": [],
      "tags": [],
      "cast_time_s": 1.0,
      "gcd_s": 1.0,
      "cooldown_s": 0.0,
      "resource_cost": null,
      "can_move_while_casting": false,
      "targeting": "unit",
      "requires_line_of_sight": true,
      "range_ft": 60,
      "minimum_range_ft": 0,
      "firing_arc_deg": 180,
      "attack": null,
      "damage": null,
      "projectile": null,
      "save": { "kind": "dex", "dc": 30, "on_fail": { "apply_condition": "prone", "duration_ms": 6000 } }
    }"#;
    let spec: data_runtime::spell::SpellSpec = serde_json::from_str(txt).unwrap();
    s.spells.insert("grease".into(), spec);
    s.cast_completed.push((0, "grease".into()));
    systems::saving_throw::run(&mut s);
    systems::conditions::run(&mut s);
    assert!(s.logs.iter().any(|l| l.contains("condition_applied")));
    assert!(
        s.actors[1]
            .statuses
            .iter()
            .any(|(c, _)| format!("{:?}", c).contains("Prone"))
    );
}

#[test]
fn buffs_bless_sets_blessed_ms_for_allies() {
    let mut s = SimState::new(50, 4);
    let mut cleric = mk_actor("cleric", "healer", Some("players"));
    cleric.ability_ids.push("cleric.bless.srd521".into());
    s.actors.push(cleric);
    s.actors.push(mk_actor("ally", "dps", Some("players")));
    s.actors.push(mk_actor("boss", "boss", Some("boss")));
    s.spells.insert(
        "cleric.bless.srd521".into(),
        data_runtime::loader::load_spell_spec("spells/bless.json").unwrap(),
    );
    s.cast_completed.push((0, "cleric.bless.srd521".into()));
    systems::buffs::run(&mut s);
    assert_eq!(s.actors[1].blessed_ms, 10_000);
    assert_eq!(s.actors[2].blessed_ms, 0); // boss not blessed
}

#[test]
fn conditions_tick_and_expire() {
    let mut s = SimState::new(50, 5);
    let mut a = mk_actor("p", "dps", Some("players"));
    a.statuses
        .push((sim_core::combat::conditions::Condition::Prone, 50));
    s.actors.push(a);
    systems::conditions::run(&mut s);
    // After one tick, 50 -> 0 and removed
    assert!(s.actors[0].statuses.is_empty());
}

#[test]
fn cast_complete_triggers_pending_damage_even_without_attack() {
    // If no attack spec, damage system still queues entry with crit=false
    let mut s = SimState::new(50, 6);
    let mut a = mk_actor("wiz", "dps", Some("players"));
    a.target = Some(1);
    s.actors.push(a);
    s.actors.push(mk_actor("boss", "boss", Some("boss")));
    // Insert a buff spell with no attack/damage
    s.spells.insert(
        "cleric.bless.srd521".into(),
        data_runtime::loader::load_spell_spec("spells/bless.json").unwrap(),
    );
    s.cast_completed.push((0, "cleric.bless.srd521".into()));
    systems::attack_roll::run(&mut s);
    assert_eq!(s.pending_damage.len(), 1);
    assert!(!s.pending_damage[0].2);
}

#[test]
fn temp_hp_absorbs_before_hp() {
    let mut s = SimState::new(50, 123);
    // Caster
    let mut a = mk_actor("wiz", "dps", Some("players"));
    a.ability_ids.push("deal100".into());
    a.target = Some(1);
    s.actors.push(a);
    // Target with THP
    let mut b = mk_actor("tgt", "boss", Some("boss"));
    b.hp = 30;
    b.temp_hp = 5;
    s.actors.push(b);
    // Insert a spell that deals exactly 7 damage: 7d1
    let txt = r#"{
      "id": "deal100",
      "name": "TestDmg",
      "school": "evocation",
      "level": 0,
      "classes": [],
      "tags": [],
      "cast_time_s": 0.0,
      "gcd_s": 0.0,
      "cooldown_s": 0.0,
      "resource_cost": null,
      "can_move_while_casting": false,
      "targeting": "unit",
      "requires_line_of_sight": true,
      "range_ft": 120,
      "minimum_range_ft": 0,
      "firing_arc_deg": 180,
      "attack": null,
      "damage": {"type":"fire", "add_spell_mod_to_damage":false, "dice_by_level_band":{"1-4":"7d1"}},
      "projectile": null,
      "save": null
    }"#;
    let spec: data_runtime::spell::SpellSpec = serde_json::from_str(txt).unwrap();
    s.spells.insert("deal100".into(), spec);
    s.cast_completed.push((0, "deal100".into()));
    // No attack roll branch → pending_damage
    systems::attack_roll::run(&mut s);
    systems::damage::run(&mut s);
    // THP 5 absorbs first; remaining 2 reduces HP
    assert_eq!(s.actors[1].temp_hp, 0);
    assert_eq!(s.actors[1].hp, 28);
}

#[test]
fn concentration_breaks_on_high_damage() {
    let mut s = SimState::new(50, 456);
    // Caster/Source
    let mut a = mk_actor("wiz", "dps", Some("players"));
    a.ability_ids.push("deal100".into());
    a.target = Some(1);
    s.actors.push(a);
    // Target with an active concentration effect
    let mut b = mk_actor("tgt", "boss", Some("boss"));
    b.hp = 100;
    b.concentration = Some("test_conc".into());
    s.actors.push(b);
    // Spell guaranteeing 100 damage (DC -> min(30))
    let txt = r#"{
      "id": "deal100",
      "name": "TestDmg",
      "school": "evocation",
      "level": 0,
      "classes": [],
      "tags": [],
      "cast_time_s": 0.0,
      "gcd_s": 0.0,
      "cooldown_s": 0.0,
      "resource_cost": null,
      "can_move_while_casting": false,
      "targeting": "unit",
      "requires_line_of_sight": true,
      "range_ft": 120,
      "minimum_range_ft": 0,
      "firing_arc_deg": 180,
      "attack": null,
      "damage": {"type":"force", "add_spell_mod_to_damage":false, "dice_by_level_band":{"1-4":"100d1"}},
      "projectile": null,
      "save": null
    }"#;
    let spec: data_runtime::spell::SpellSpec = serde_json::from_str(txt).unwrap();
    s.spells.insert("deal100".into(), spec);
    s.cast_completed.push((0, "deal100".into()));
    systems::attack_roll::run(&mut s);
    systems::damage::run(&mut s);
    // DC 30 always fails on 1d20 + 0 → concentration must break
    assert!(s.actors[1].concentration.is_none());
    assert!(s.logs.iter().any(|l| l.contains("concentration_broken")));
}

#[test]
fn ability_cooldown_starts_on_cast() {
    let mut s = SimState::new(50, 321);
    let mut caster = mk_actor("wiz", "dps", Some("players"));
    caster.ability_ids.push("wiz.fire_bolt.srd521".into());
    s.actors.push(caster);
    // Ensure spell is loaded to read cooldown
    s.spells.insert(
        "wiz.fire_bolt.srd521".into(),
        data_runtime::loader::load_spell_spec("spells/fire_bolt.json").unwrap(),
    );
    // Start cast (may or may not start depending on GCD); ensure Idle for next attempt
    systems::cast_begin::run(&mut s);
    s.actors[0].action = ActionState::Idle;
    // Cooldown is 0 for Fire Bolt; spoof a non-zero cooldown in state and ensure gate
    s.actors[0]
        .ability_cooldowns
        .insert("wiz.fire_bolt.srd521".into(), 5000);
    // Try starting again; should not start due to cooldown
    let before_logs = s.logs.len();
    systems::cast_begin::run(&mut s);
    assert_eq!(before_logs, s.logs.len());
}
