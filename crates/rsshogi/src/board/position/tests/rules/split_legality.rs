use super::helpers::move_from_usi;

#[test]
fn test_is_legal_after_pseudo_move32_accepts_normal_safe_move() {
    let pos = crate::board::hirate_position();
    let mv = move_from_usi(&pos, "7g7f");

    assert!(pos.is_pseudo_legal_move32(mv, true));
    assert!(pos.is_legal_after_pseudo_move32(mv));
}

#[test]
fn test_is_legal_after_pseudo_move32_rejects_king_move_into_check() {
    let sfen = "1+P3Skn1/4+B1g2/4p1sp1/6p1p/LppL1P1PK/2PbP1P2/1P1+p5/3r2s2/5G1NL b RGSNL4Pgn 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid SFEN");
    let mv = move_from_usi(&pos, "1e2d");

    assert!(pos.is_pseudo_legal_move32(mv, true));
    assert!(!pos.is_legal_after_pseudo_move32(mv));
}

#[test]
fn test_is_legal_after_pseudo_move32_rejects_pinned_piece_off_pin_line() {
    let sfen = "1+P3Skn1/4+B1g2/4p1sp1/6p1p/LppL1P1P1/2PbP1Ps1/1P1+p5/3r2G1K/5G1NL b RSNL4Pgn 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid SFEN");
    let mv = move_from_usi(&pos, "3h3g");

    assert!(pos.is_pseudo_legal_move32(mv, true));
    assert!(!pos.is_legal_after_pseudo_move32(mv));
}
