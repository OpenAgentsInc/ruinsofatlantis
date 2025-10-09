#![allow(clippy::unwrap_used)]

#[test]
fn wizard_models_follow_replication_positions() {
    // Minimal renderer-like struct exercise: create two wizard models, set replication with
    // PC at (0,0,0) and one NPC at (10,0,0). After running render path once, models should
    // translate near those positions.
    let mut r = crate::gfx::Renderer::test_minimal();
    // Two wizard instance slots
    r.wizard_models = vec![
        glam::Mat4::from_translation(glam::vec3(-5.0, 0.6, 0.0)),
        glam::Mat4::from_translation(glam::vec3(-6.0, 0.6, 0.0)),
    ];
    r.wizard_instances_cpu = vec![
        crate::gfx::types::InstanceSkin {
            model: r.wizard_models[0].to_cols_array_2d(),
            ..Default::default()
        },
        crate::gfx::types::InstanceSkin {
            model: r.wizard_models[1].to_cols_array_2d(),
            ..Default::default()
        },
    ];
    r.pc_index = 0;
    // Replication: PC and one NPC wizard
    r.repl_buf.wizards = vec![
        client_core::replication::WizardView {
            id: 1,
            kind: 0,
            pos: glam::vec3(0.0, 0.6, 0.0),
            yaw: 0.0,
            hp: 100,
            max: 100,
            is_pc: true,
        },
        client_core::replication::WizardView {
            id: 2,
            kind: 0,
            pos: glam::vec3(10.0, 0.6, 0.0),
            yaw: 0.0,
            hp: 100,
            max: 100,
            is_pc: false,
        },
    ];
    // Run the portion that syncs wizard models (call render() which drives update)
    // Use a very small frame to avoid GPU, focusing on CPU path; the test helper should make this no-op GPU-wise.
    r.last_time = 0.0; // deterministic
    // Call the internal sync function indirectly by invoking the render tick
    // Note: in this test harness, we call a reduced path that executes update without GPU submits.
    crate::gfx::renderer::update::tests::drive_update_minimal(&mut r);

    let a = r.wizard_models[0].to_cols_array();
    let b = r.wizard_models[1].to_cols_array();
    let pa = glam::vec3(a[12], a[13], a[14]);
    let pb = glam::vec3(b[12], b[13], b[14]);
    assert!((pa - glam::vec3(0.0, 0.6, 0.0)).length() < 0.1);
    assert!((pb - glam::vec3(10.0, 0.6, 0.0)).length() < 0.1);
}
