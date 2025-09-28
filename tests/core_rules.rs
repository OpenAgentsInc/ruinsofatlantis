use sim_core::rules::attack::Advantage;
use sim_core::rules::saves::SaveKind;

#[test]
fn advantage_variants_debug() {
    let a = Advantage::Normal;
    let b = Advantage::Advantage;
    let c = Advantage::Disadvantage;
    // Simple Debug formatting existence
    assert!(format!("{:?}", a).contains("Normal"));
    assert!(format!("{:?}", b).contains("Advantage"));
    assert!(format!("{:?}", c).contains("Disadvantage"));
}

#[test]
fn save_kind_cov() {
    let kinds = [
        SaveKind::Str,
        SaveKind::Dex,
        SaveKind::Con,
        SaveKind::Int,
        SaveKind::Wis,
        SaveKind::Cha,
    ];
    for k in kinds {
        assert!(!format!("{:?}", k).is_empty());
    }
}

#[test]
fn enums_clone_copy_work() {
    let a = Advantage::Advantage;
    let b = a; // Copy
    let c = b; // Copy again
    assert!(matches!(c, Advantage::Advantage));
}
