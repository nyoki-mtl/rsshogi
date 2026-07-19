use crate::board::{MoveList, NonEvasionsAll, generate_moves, test_support::move_from_usi_expect};
use crate::types::Color;

#[test]
fn test_king_move_to_safe_square_is_legal() {
    // 玉が安全なマスへ移動する手は合法
    let sfen = "1+P3Skn1/4+B1g2/4p1sp1/6p1p/LppL1P1PK/2PbP1P2/1P1+p5/3r2s2/5G1NL b RGSNL4Pgn 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid SFEN");

    // 1e -> 1d への移動（安全なマス）
    let mv = move_from_usi_expect(&pos, "1e1d");
    assert!(pos.is_legal_move32(mv), "King move to safe square should be legal");
}

#[test]
fn test_king_move_to_attacked_square_is_illegal() {
    // 玉が敵の利きにさらされるマスへ移動する手は不合法
    let sfen = "1+P3Skn1/4+B1g2/4p1sp1/6p1p/LppL1P1PK/2PbP1P2/1P1+p5/3r2s2/5G1NL b RGSNL4Pgn 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid SFEN");

    // 1e -> 2d への移動（相手の駒が利いている）
    let mv = move_from_usi_expect(&pos, "1e2d");
    assert!(!pos.is_legal_move32(mv), "King move to attacked square should be illegal");
}

#[test]
fn test_pinned_piece_move_along_pin_line_is_legal() {
    // ピンされた駒がピン方向に沿って移動する手は合法
    let sfen = "1+P3Skn1/4+B1g2/4p1sp1/6p1p/LppL1P1P1/2PbP1Ps1/1P1+p5/3r2G1K/5G1NL b RSNL4Pgn 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid SFEN");

    // 金が3h -> 4h へ移動（ピン方向に沿っている）
    let mv = move_from_usi_expect(&pos, "3h4h");
    assert!(pos.is_legal_move32(mv), "Pinned piece move along pin line should be legal");
}

#[test]
fn test_pinned_piece_move_off_pin_line_is_illegal() {
    // ピンされた駒がピン方向から外れて移動する手は不合法
    let sfen = "1+P3Skn1/4+B1g2/4p1sp1/6p1p/LppL1P1P1/2PbP1Ps1/1P1+p5/3r2G1K/5G1NL b RSNL4Pgn 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid SFEN");

    // 金が3h -> 3g へ移動（ピン方向から外れる）
    let mv = move_from_usi_expect(&pos, "3h3g");
    assert!(!pos.is_legal_move32(mv), "Pinned piece move off pin line should be illegal");
}

#[test]
fn test_normal_piece_move_is_legal() {
    // ピンされていない駒の通常移動は合法
    let pos = crate::board::hirate_position();

    // 7七の歩を7六へ移動
    let mv = move_from_usi_expect(&pos, "7g7f");
    // 初期局面では合法手生成がまだ完成していないので、基本チェックのみ
    assert!(pos.is_legal_move32(mv), "Normal piece move should be legal");
}

#[test]
fn test_generated_moves_roundtrip_and_preserve_pin_cache() {
    // 盤面の合法手生成が apply/undo 循環に耐えることを検証
    const PINNED_SCENARIO_SFEN: &str =
        "1+P3Skn1/4+B1g2/4p1sp1/6p1p/LppL1P1P1/2PbP1Ps1/1P1+p5/3r2G1K/5G1NL b RSNL4Pgn 1";

    let mut pos = crate::board::position_from_sfen(PINNED_SCENARIO_SFEN).expect("valid SFEN");

    let pinned_before = pos.pinned_pieces(Color::BLACK);
    assert!(!pinned_before.is_empty(), "Scenario must contain pinned pieces");

    let zobrist_before = pos.key();
    let depth_before = pos.state_stack().depth();

    let mut moves = MoveList::new();
    generate_moves::<NonEvasionsAll>(&pos, &mut moves);
    assert!(!moves.is_empty(), "Scenario should yield legal moves");

    let legal_moves: Vec<_> = moves.iter().copied().filter(|mv| pos.is_legal_move(*mv)).collect();
    assert!(!legal_moves.is_empty(), "Scenario should yield legal moves");

    for mv in legal_moves {
        pos.apply_move(mv);
        assert_eq!(
            pos.state_stack().depth(),
            depth_before + 1,
            "StateStack depth should increase after apply_move32"
        );

        pos.undo_move(mv).expect("undo move");
        assert_eq!(
            pos.state_stack().depth(),
            depth_before,
            "StateStack depth should restore after undo_move32"
        );
        assert_eq!(pos.key(), zobrist_before, "Zobrist key should return to initial state");
    }

    assert_eq!(
        pos.pinned_pieces(Color::BLACK),
        pinned_before,
        "Pinned pieces cache should survive apply/undo roundtrip"
    );

    let illegal = move_from_usi_expect(&pos, "3h3g");
    assert!(
        !pos.is_legal_move32(illegal),
        "Pinned piece deviating from pin line must remain illegal after roundtrip"
    );
}
