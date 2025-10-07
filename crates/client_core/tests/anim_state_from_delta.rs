#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnimState {
    Idle,
    Jog,
    Death,
}

fn pick_anim(prev: [f32; 3], cur: [f32; 3], dt: f32, alive: bool) -> AnimState {
    if !alive {
        return AnimState::Death;
    }
    let dx = cur[0] - prev[0];
    let dz = cur[2] - prev[2];
    let dist = (dx * dx + dz * dz).sqrt();
    let speed = if dt > 1e-6 { dist / dt } else { 0.0 };
    if speed < 0.2 {
        AnimState::Idle
    } else {
        AnimState::Jog
    }
}

#[test]
fn anim_idle_then_jog_then_death() {
    let prev = [0.0, 0.6, 0.0];
    let cur_idle = [0.05, 0.6, 0.0]; // ~0.05m in 1s ⇒ idle
    let cur_jog = [1.0, 0.6, 0.0]; // 1m in 1s ⇒ jog
    assert_eq!(pick_anim(prev, cur_idle, 1.0, true), AnimState::Idle);
    assert_eq!(pick_anim(prev, cur_jog, 1.0, true), AnimState::Jog);
    // Death overrides movement
    assert_eq!(pick_anim(prev, cur_jog, 1.0, false), AnimState::Death);
}
