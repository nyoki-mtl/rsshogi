use super::helpers::move_from_usi;
use crate::types::Move;

#[test]
fn test_pseudo_legal_knight_promotion_boundaries() {
    let cases = [
        ("4k4/9/1N7/9/9/9/9/9/4K4 b - 1", "8c7a", false),
        ("4k4/9/9/1N7/9/9/9/9/4K4 b - 1", "8d7b", false),
        ("4k4/9/9/9/1N7/9/9/9/4K4 b - 1", "8e7c", true),
        ("4k4/9/9/9/9/1n7/9/9/4K4 w - 1", "8f7h", false),
        ("4k4/9/9/9/9/9/1n7/9/4K4 w - 1", "8g7i", false),
        ("4k4/9/9/9/1n7/9/9/9/4K4 w - 1", "8e7g", true),
    ];

    for (sfen, usi, unpromoted_is_legal) in cases {
        let pos = crate::board::position_from_sfen(sfen).expect("valid SFEN");
        for promote in [false, true] {
            let usi = if promote { format!("{usi}+") } else { usi.to_owned() };
            let mv = Move::from_usi(&usi).expect("valid move encoding");
            let mv32 = pos.move32_from_move(mv);
            let expected = promote || unpromoted_is_legal;
            for generate_all in [false, true] {
                assert_eq!(
                    pos.is_pseudo_legal_move(mv, generate_all),
                    expected,
                    "Move: {sfen}, {usi}, generate_all={generate_all}"
                );
                assert_eq!(
                    pos.is_pseudo_legal_move32(mv32, generate_all),
                    expected,
                    "Move32: {sfen}, {usi}, generate_all={generate_all}"
                );
                assert_eq!(
                    pos.is_pseudo_legal_move32(mv32, generate_all)
                        && pos.is_legal_promote(mv32)
                        && pos.is_legal_after_pseudo_move32(mv32),
                    expected,
                    "split legality: {sfen}, {usi}, generate_all={generate_all}"
                );
            }
            assert_eq!(pos.is_legal_move(mv), expected, "{sfen}, {usi}");
            assert_eq!(pos.is_legal_move32(mv32), expected, "{sfen}, {usi}");
        }
    }
}

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
