use net_core::snapshot::SnapshotEncode;

#[test]
fn client_parses_hud_toast_not_enough_mana() {
    use net_core::snapshot::{HUD_TOAST_VERSION, HudToastMsg};
    let toast = HudToastMsg {
        v: HUD_TOAST_VERSION,
        code: 1,
    };
    let mut buf = Vec::new();
    toast.encode(&mut buf);
    let mut framed = Vec::new();
    net_core::frame::write_msg(&mut framed, &buf);

    let mut repl = client_core::replication::ReplicationBuffer::default();
    assert!(repl.apply_message(&framed));
    assert_eq!(repl.toasts, vec![1u8]);
}
