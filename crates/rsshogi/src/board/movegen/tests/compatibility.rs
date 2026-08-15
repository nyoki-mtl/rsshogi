use super::{
    CapturePlusProAll, Captures, Legal, MoveList, QuietChecks, Quiets, QuietsProMinus,
    QuietsProMinusAll, generate_moves,
};

fn moves<T: super::MoveGenType>(sfen: &str) -> Vec<String> {
    let pos = crate::board::position_from_sfen(sfen).expect("valid SFEN");
    let mut list = MoveList::new();
    generate_moves::<T>(&pos, &mut list);
    list.iter().map(|mv| mv.to_usi()).collect()
}

#[test]
fn quiet_checks_include_checking_drops() {
    let generated = moves::<QuietChecks>("4k4/9/9/9/9/9/9/9/4K4 b R 1");
    assert!(generated.iter().any(|mv| mv == "R*5b"));
}

#[test]
fn legal_keeps_lance_non_promotion_on_third_rank() {
    let generated = moves::<Legal>("4k4/9/4p4/4L4/9/9/9/9/4K4 b - 1");
    assert!(generated.iter().any(|mv| mv == "5d5c"));
    assert!(generated.iter().any(|mv| mv == "5d5c+"));
}

#[test]
fn captures_keep_silver_promotion_and_non_promotion() {
    let generated = moves::<Captures>("4k4/9/3p5/4S4/9/9/9/9/4K4 b - 1");
    assert!(generated.iter().any(|mv| mv == "5d6c"));
    assert!(generated.iter().any(|mv| mv == "5d6c+"));
}

#[test]
fn quiets_pro_minus_excludes_quiet_pawn_promotions() {
    let sfen = "4k4/9/9/4P4/9/9/9/9/4K4 b - 1";
    let quiets = moves::<Quiets>(sfen);
    let minus = moves::<QuietsProMinus>(sfen);
    let minus_all = moves::<QuietsProMinusAll>(sfen);

    assert!(quiets.iter().any(|mv| mv == "5d5c+"));
    assert!(!minus.iter().any(|mv| mv == "5d5c+"));
    assert!(minus_all.iter().any(|mv| mv == "5d5c"));
    assert!(!minus_all.iter().any(|mv| mv == "5d5c+"));
}

#[test]
fn capture_plus_pro_all_keeps_paired_pawn_non_promotion() {
    let generated = moves::<CapturePlusProAll>("4k4/9/9/4P4/9/9/9/9/4K4 b - 1");
    assert!(generated.iter().any(|mv| mv == "5d5c"));
    assert!(generated.iter().any(|mv| mv == "5d5c+"));
}

#[test]
fn non_evasion_modes_do_not_apply_evasion_targets_while_in_check() {
    let generated = moves::<Quiets>("k3r4/9/9/9/9/9/9/9/4K3G b - 1");
    assert!(generated.iter().any(|mv| mv == "1i1h"));
}
