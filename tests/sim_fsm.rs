use ruinsofatlantis::core::combat::fsm::{ActionState, Gcd, ReactionWindow, StartError};
use ruinsofatlantis::core::data::ids::Id;

#[test]
fn default_state_is_idle() {
    let s: ActionState = Default::default();
    assert!(matches!(s, ActionState::Idle));
}

#[test]
fn tick_idle_noop() {
    let s = ActionState::Idle;
    let (next, done) = s.tick(100);
    assert!(matches!(next, ActionState::Idle));
    assert!(done.is_none());
}

#[test]
fn casting_ticks_down_and_completes() {
    let s = ActionState::Casting {
        ability: Id("a".into()),
        remaining_ms: 150,
    };
    let (s, done) = s.tick(100);
    assert!(done.is_none());
    let (s, done) = s.tick(100);
    assert!(matches!(s, ActionState::Recovery { .. }));
    assert!(done.is_some());
}

#[test]
fn channeling_ticks_down_and_completes() {
    let s = ActionState::Channeling {
        ability: Id("a".into()),
        remaining_ms: 50,
    };
    let (_s, done) = s.tick(60);
    assert!(done.is_some());
}

#[test]
fn recovery_ticks_to_idle() {
    let s = ActionState::Recovery { remaining_ms: 30 };
    let (s, _) = s.tick(20);
    let (s, _) = s.tick(20);
    assert!(matches!(s, ActionState::Idle));
}

#[test]
fn try_start_cast_sets_gcd_and_enters_casting() {
    let mut gcd = Gcd::default();
    let s = ActionState::Idle;
    let s = s
        .try_start_cast(Id("x".into()), 200, &mut gcd, 100)
        .expect("start");
    assert!(matches!(s, ActionState::Casting { .. }));
    assert_eq!(gcd.remaining_ms, 100);
}

#[test]
fn try_start_cast_fails_if_busy() {
    let mut gcd = Gcd::default();
    let s = ActionState::Recovery { remaining_ms: 10 };
    let err = s
        .try_start_cast(Id("x".into()), 100, &mut gcd, 30)
        .unwrap_err();
    match err {
        StartError::Busy => {}
        _ => panic!("unexpected"),
    }
}

#[test]
fn try_start_cast_fails_if_gcd_locked() {
    let mut gcd = Gcd { remaining_ms: 10 };
    let s = ActionState::Idle;
    let err = s
        .try_start_cast(Id("x".into()), 100, &mut gcd, 30)
        .unwrap_err();
    match err {
        StartError::GcdLocked => {}
        _ => panic!("unexpected"),
    }
}

#[test]
fn gcd_tick_and_ready() {
    let mut g = Gcd { remaining_ms: 30 };
    g.tick(10);
    assert!(!g.ready());
    g.tick(20);
    assert!(g.ready());
}

#[test]
fn reaction_window_tick_and_open() {
    let mut r = ReactionWindow { remaining_ms: 5 };
    assert!(r.open());
    r.tick(5);
    assert!(!r.open());
}
