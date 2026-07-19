use crate::types::{Color, SQ_15, SQ_38, SQ_46};

#[test]
fn test_pinned_pieces_no_pins() {
    // 初期局面ではピンされている駒はない
    let pos = crate::board::hirate_position();

    let black_pinned = pos.pinned_pieces(Color::BLACK);
    assert!(black_pinned.is_empty(), "Initial position should have no pinned pieces for BLACK");

    let white_pinned = pos.pinned_pieces(Color::WHITE);
    assert!(white_pinned.is_empty(), "Initial position should have no pinned pieces for WHITE");
}

#[test]
fn test_pinned_by_rook_horizontally() {
    // 飛車による横方向のピン
    let sfen = "1+P3Skn1/4+B1g2/4p1sp1/6p1p/LppL1P1P1/2PbP1Ps1/1P1+p5/3r2G1K/5G1NL b RSNL4Pgn 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid SFEN");

    let pinned = pos.pinned_pieces(Color::BLACK);
    assert!(pinned.test(SQ_38), "Gold at 3八 should be pinned by rook horizontally");
    assert_eq!(pinned.count(), 1, "Only one piece should be pinned");
}

#[test]
fn test_pinned_by_rook_vertically() {
    // 飛車による縦方向のピン
    let sfen = "1+P3Skn1/4+B1g1r/4p1sp1/6p2/LppL1P1PS/2PbP1P2/1P1+p5/3p2G1K/5G1NL b RNL4Pgsn 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid SFEN");

    let pinned = pos.pinned_pieces(Color::BLACK);
    assert!(pinned.test(SQ_15), "Silver at 1五 should be pinned by rook vertically");
    assert_eq!(pinned.count(), 1, "Only one piece should be pinned");
}

#[test]
fn test_pinned_by_bishop_diagonally() {
    // 角による斜め方向のピン
    let sfen = "1+P3Skn1/4+B1g2/2b1p1sp1/6p1p/LppL1P1P1/2P1PSP2/1P1+p5/3p2GK1/5G1NL b RNL3Prgsn 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid SFEN");

    let pinned = pos.pinned_pieces(Color::BLACK);
    assert!(pinned.test(SQ_46), "Silver at 4六 should be pinned by bishop diagonally");
    assert_eq!(pinned.count(), 1, "Only one piece should be pinned");
}

#[test]
fn test_pinned_pieces_blocked_by_enemy() {
    // ピンの直線上に敵駒がいる場合、ピンは成立しない
    let sfen = "1+P3Skn1/4+B1g2/2b1p1sp1/3s2p1p/Lpp2P1P1/2P1PSP2/1P1+p5/3p2GK1/5G1NL b RNL3Prgnl 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid SFEN");

    let pinned = pos.pinned_pieces(Color::BLACK);
    assert!(
        pinned.is_empty(),
        "Silver at 4六 should not be pinned when enemy piece blocks the line"
    );
}

#[test]
fn test_no_pin_when_friendly_piece_blocks() {
    // ピンの直線上に味方の別の駒がいる場合、玉に最も近い駒のみがピンされる
    let sfen =
        "1+P3Skn1/4+B1g2/2b1p1sp1/3P2p1p/Lpp2P1P1/2P1PSP2/1P1+p5/3p2GK1/5G1NL b RNL2Prgsnl 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid SFEN");

    let pinned = pos.pinned_pieces(Color::BLACK);
    assert!(
        pinned.is_empty(),
        "Silver at 4六 should not be pinned when friendly piece blocks the line"
    );
}
