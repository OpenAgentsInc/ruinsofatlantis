use glam::{Quat, Vec3};
use ruinsofatlantis::ecs::{RenderKind, Transform, World};

#[test]
fn transform_default_matrix_is_identity() {
    let t = Transform::default();
    let m = t.matrix();
    assert_eq!(m.to_cols_array()[0], 1.0);
    assert_eq!(m.to_cols_array()[5], 1.0);
    assert_eq!(m.to_cols_array()[10], 1.0);
    assert_eq!(m.to_cols_array()[15], 1.0);
}

#[test]
fn transform_scale_translation() {
    let t = Transform {
        translation: Vec3::new(1.0, 2.0, 3.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::new(2.0, 3.0, 4.0),
    };
    let m = t.matrix();
    let cols = m.to_cols_array();
    // Scale on diagonal
    assert_eq!(cols[0], 2.0);
    assert_eq!(cols[5], 3.0);
    assert_eq!(cols[10], 4.0);
    // Translation in last column
    assert!((cols[12] - 1.0).abs() < 1e-6);
    assert!((cols[13] - 2.0).abs() < 1e-6);
    assert!((cols[14] - 3.0).abs() < 1e-6);
}

#[test]
fn world_spawn_increments_ids_and_stores_components() {
    let mut w = World::new();
    let e1 = w.spawn(Transform::default(), RenderKind::Wizard);
    let e2 = w.spawn(Transform::default(), RenderKind::Ruins);
    assert_ne!(e1, e2);
    assert_eq!(w.ids.len(), 2);
    assert!(matches!(w.kinds[0], RenderKind::Wizard));
    assert!(matches!(w.kinds[1], RenderKind::Ruins));
}

#[test]
fn world_indices_align() {
    let mut w = World::new();
    for _ in 0..5 {
        w.spawn(Transform::default(), RenderKind::Wizard);
    }
    assert_eq!(w.ids.len(), 5);
    assert_eq!(w.transforms.len(), 5);
    assert_eq!(w.kinds.len(), 5);
}
