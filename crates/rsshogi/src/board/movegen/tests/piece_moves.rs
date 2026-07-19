use super::{NonEvasionsAll, generate_moves};
use crate::board::MoveList;
use crate::types::{Color, Move, PieceType, Square};

#[test]
fn test_pawn_moves_startpos() {
    let pos = crate::board::hirate_position();
    let mut moves = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut moves);

    let pawn_moves_count = moves
        .iter()
        .filter(|&m| {
            !m.is_drop()
                && pos.piece_on(m.from_sq()).piece_type() == PieceType::PAWN
                && pos.piece_on(m.from_sq()).color() == Color::BLACK
        })
        .count();

    assert_eq!(pawn_moves_count, 9, "初期局面で先手の歩の移動手は9手");
}

#[test]
fn test_pawn_promotion_choices_are_generated() {
    let sfen = "9/9/9/P8/9/9/9/9/9 b - 1";
    let pos = crate::board::position_from_sfen(sfen).unwrap();
    let mut moves = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut moves);

    let pawn_moves: Vec<Move> = moves
        .iter()
        .filter(|&m| !m.is_drop() && pos.piece_on(m.from_sq()).piece_type() == PieceType::PAWN)
        .copied()
        .collect();

    assert_eq!(pawn_moves.len(), 2, "敵陣進入時は成り・不成りの2手");
    assert!(pawn_moves.iter().any(|m| m.is_promotion()), "成り手が含まれる");
    assert!(pawn_moves.iter().any(|m| !m.is_promotion()), "不成り手が含まれる");
}

#[test]
fn test_pawn_must_promote_on_last_rank() {
    let sfen = "9/P8/9/9/9/9/9/9/9 b - 1";
    let pos = crate::board::position_from_sfen(sfen).unwrap();
    let mut moves = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut moves);

    let pawn_moves: Vec<Move> = moves
        .iter()
        .filter(|&m| !m.is_drop() && pos.piece_on(m.from_sq()).piece_type() == PieceType::PAWN)
        .copied()
        .collect();

    assert_eq!(pawn_moves.len(), 1, "1段目への進出は成りのみ生成");
    let move_to_top = pawn_moves.first().expect("at least one pawn move");
    assert!(move_to_top.is_promotion(), "不成り手が生成されない");
    assert_eq!(move_to_top.to_sq(), "9a".parse::<Square>().unwrap());
}

#[test]
fn test_knight_moves_include_promotion_options() {
    let sfen = "9/9/9/9/4N4/9/9/9/9 b - 1";
    let pos = crate::board::position_from_sfen(sfen).unwrap();
    let mut moves = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut moves);

    let knight_moves_count = moves
        .iter()
        .filter(|&m| !m.is_drop() && pos.piece_on(m.from_sq()).piece_type() == PieceType::KNIGHT)
        .count();

    assert_eq!(knight_moves_count, 4, "桂馬の移動手は4手（2方向×成り不成り）");
}

#[test]
fn test_knight_must_promote_near_last_ranks() {
    let sfen = "9/9/1N7/9/9/9/9/9/9 b - 1";
    let pos = crate::board::position_from_sfen(sfen).unwrap();
    let mut moves = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut moves);

    let knight_moves: Vec<Move> = moves
        .iter()
        .filter(|&m| !m.is_drop() && pos.piece_on(m.from_sq()).piece_type() == PieceType::KNIGHT)
        .copied()
        .collect();

    assert_eq!(knight_moves.len(), 2, "桂の2方向への跳びは成りのみ生成される");
    assert!(knight_moves.iter().all(|m| m.is_promotion()));
    let destinations: Vec<Square> = knight_moves.iter().map(|m| m.to_sq()).collect();
    assert!(destinations.contains(&"7a".parse::<Square>().unwrap()));
    assert!(destinations.contains(&"9a".parse::<Square>().unwrap()));
}

#[test]
fn test_silver_moves_have_expected_count() {
    let sfen = "9/9/9/9/4S4/9/9/9/9 b - 1";
    let pos = crate::board::position_from_sfen(sfen).unwrap();
    let mut moves = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut moves);

    let silver_moves_count = moves
        .iter()
        .filter(|&m| !m.is_drop() && pos.piece_on(m.from_sq()).piece_type() == PieceType::SILVER)
        .count();

    assert_eq!(silver_moves_count, 5, "銀の移動手は5手");
}

#[test]
fn test_gold_moves_have_expected_count() {
    let sfen = "9/9/9/9/4G4/9/9/9/9 b - 1";
    let pos = crate::board::position_from_sfen(sfen).unwrap();
    let mut moves = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut moves);

    let gold_moves_count = moves
        .iter()
        .filter(|&m| !m.is_drop() && pos.piece_on(m.from_sq()).piece_type() == PieceType::GOLD)
        .count();

    assert_eq!(gold_moves_count, 6, "金の移動手は6手");
}

#[test]
fn test_king_moves_have_expected_count() {
    let sfen = "9/9/9/9/4K4/9/9/9/9 b - 1";
    let pos = crate::board::position_from_sfen(sfen).unwrap();
    let mut moves = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut moves);

    let king_moves_count = moves
        .iter()
        .filter(|&m| !m.is_drop() && pos.piece_on(m.from_sq()).piece_type() == PieceType::KING)
        .count();

    assert_eq!(king_moves_count, 8, "玉の移動手は8手");
}
