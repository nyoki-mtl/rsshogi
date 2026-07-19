//! ピン情報キャッシュの apply/undo 整合性を検証する統合テスト

use rsshogi::board::{self, MoveList, NonEvasionsAll, Position, generate_moves};
use rsshogi::types::Color;

const PINNED_SCENARIO_SFEN: &str =
    "1+P3Skn1/4+B1g2/4p1sp1/6p1p/LppL1P1P1/2PbP1Ps1/1P1+p5/3r2G1K/5G1NL b RSNL4Pgn 1";

fn assert_cache_matches(pos: &Position, color: Color) {
    let cached = pos.blockers_for_king(color) & pos.bitboards().color_pieces(color);
    assert_eq!(
        pos.pinned_pieces(color),
        cached,
        "Pinned cache for {color} must match Position::pinned_pieces"
    );
}

#[test]
fn test_pinned_cache_matches_position_after_apply_undo() {
    let mut pos = board::position_from_sfen(PINNED_SCENARIO_SFEN).expect("valid SFEN");

    let mut moves = MoveList::new();
    generate_moves::<NonEvasionsAll>(&pos, &mut moves);
    assert!(!moves.is_empty(), "Scenario should yield legal moves");

    let depth_before = pos.state_stack_depth();
    let pinned_black_before = pos.pinned_pieces(Color::BLACK);
    let pinned_white_before = pos.pinned_pieces(Color::WHITE);

    assert_cache_matches(&pos, Color::BLACK);
    assert_cache_matches(&pos, Color::WHITE);

    for &mv in moves.iter() {
        let mv_move = pos.move32_from_move(mv);
        if !mv_move.is_normal() || !pos.is_legal_move32(mv_move) {
            continue;
        }
        pos.apply_move32(mv_move);
        assert_cache_matches(&pos, Color::BLACK);
        assert_cache_matches(&pos, Color::WHITE);

        pos.undo_move32(mv_move).expect("undo move");
        assert_eq!(
            pos.state_stack_depth(),
            depth_before,
            "StateStack depth should restore after undo_move32"
        );
        assert_cache_matches(&pos, Color::BLACK);
        assert_cache_matches(&pos, Color::WHITE);
        assert_eq!(pos.pinned_pieces(Color::BLACK), pinned_black_before);
        assert_eq!(pos.pinned_pieces(Color::WHITE), pinned_white_before);
    }
}
