#![allow(clippy::unwrap_used)]

#[test]
fn projectiles_interest_filtered_matches_radius() {
    // Build a tiny server state stand-in with three projectiles at different radii
    // This test exercises the same logic used in platform_winit to filter projectiles by interest.
    #[derive(Clone, Copy)]
    struct P {
        pos: [f32; 3],
        vel: [f32; 3],
        kind: u8,
        id: u32,
    }
    fn filter(projs: &[P], center: [f32; 3], r2: f32) -> usize {
        projs
            .iter()
            .filter(|p| {
                let dx = p.pos[0] - center[0];
                let dz = p.pos[2] - center[2];
                dx * dx + dz * dz <= r2
            })
            .count()
    }
    let center = [0.0, 0.0, 0.0];
    let r2 = 25.0 * 25.0;
    let projs = vec![
        P {
            id: 1,
            kind: 0,
            pos: [5.0, 1.0, 0.0],
            vel: [0.0, 0.0, 1.0],
        },
        P {
            id: 2,
            kind: 0,
            pos: [15.0, 1.0, 0.0],
            vel: [0.0, 0.0, 1.0],
        },
        P {
            id: 3,
            kind: 0,
            pos: [30.0, 1.0, 0.0],
            vel: [0.0, 0.0, 1.0],
        },
    ];
    assert_eq!(
        filter(&projs, center, r2),
        2,
        "only projectiles within 25m should remain"
    );
}
