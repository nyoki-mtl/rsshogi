use super::helpers::{apply_usi_move, move_from_usi};
use super::*;

const fn is_promoted_piece(piece: Piece) -> bool {
    matches!(
        piece.piece_type(),
        PieceType::PRO_PAWN
            | PieceType::PRO_LANCE
            | PieceType::PRO_KNIGHT
            | PieceType::PRO_SILVER
            | PieceType::HORSE
            | PieceType::DRAGON
    )
}
use crate::types::{Color, Move, Move32, Piece, PieceType, Square};

fn assert_apply_and_undo_delta_roundtrip(pos: &mut Position, mv: Move32) {
    let initial_sfen = pos.to_sfen(None);
    let initial_key = pos.key();
    let gives_check = pos.gives_check_move32(mv);

    let apply_delta = pos.apply_move32_with_delta(mv, gives_check);
    let undo_delta = pos.undo_move32_with_delta(mv).expect("undo with delta");

    assert_eq!(undo_delta, apply_delta, "undo delta should match original forward delta");
    assert_eq!(pos.to_sfen(None), initial_sfen, "position should restore after undo");
    assert_eq!(pos.key(), initial_key, "zobrist should restore after undo");
}

#[test]
// 通常手のapply/undoで局面が完全に往復するか検証
fn test_undo_normal_move() {
    let mut pos = crate::board::hirate_position();

    let initial_sfen = pos.to_sfen(None);
    let initial_zobrist = pos.key();
    let initial_ply = pos.game_ply();
    let initial_side = pos.turn();

    let from = Square::from_file_rank(crate::types::File::FILE_7, crate::types::Rank::RANK_7);
    let to = Square::from_file_rank(crate::types::File::FILE_7, crate::types::Rank::RANK_6);
    let piece = pos.piece_on(from);
    let mv = Move32::normal(from, to, piece);
    pos.apply_move32(mv);

    assert_ne!(pos.to_sfen(None), initial_sfen);
    assert_ne!(pos.key(), initial_zobrist);
    assert_eq!(pos.game_ply(), initial_ply + 1);
    assert_eq!(pos.turn(), initial_side.flip());

    pos.undo_move32(mv).unwrap();

    assert_eq!(pos.to_sfen(None), initial_sfen);
    assert_eq!(pos.key(), initial_zobrist);
    assert_eq!(pos.game_ply(), initial_ply);
    assert_eq!(pos.turn(), initial_side);
}

#[test]
// Moveの通常手でapply/undoが往復するか検証
fn test_undo_move_normal_move() {
    let mut pos = crate::board::hirate_position();

    let initial_sfen = pos.to_sfen(None);
    let initial_zobrist = pos.key();
    let initial_ply = pos.game_ply();
    let initial_side = pos.turn();

    let mv16 = Move::from_usi("7g7f").unwrap();
    assert!(pos.is_legal_move(mv16));

    pos.apply_move(mv16);
    pos.undo_move(mv16).unwrap();

    assert_eq!(pos.to_sfen(None), initial_sfen);
    assert_eq!(pos.key(), initial_zobrist);
    assert_eq!(pos.game_ply(), initial_ply);
    assert_eq!(pos.turn(), initial_side);
}

#[test]
// 駒取りを含む手のundoで持ち駒や盤面が復元されるか確認
fn test_undo_capture_move() {
    let mut pos = crate::board::hirate_position();

    apply_usi_move(&mut pos, "7g7f");
    apply_usi_move(&mut pos, "3c3d");
    apply_usi_move(&mut pos, "3g3f");
    apply_usi_move(&mut pos, "3d3e");

    let initial_sfen = pos.to_sfen(None);
    let initial_zobrist = pos.key();
    let initial_black_hand = pos.hand(Color::BLACK);

    let capture_from = Square::from_usi("3f").unwrap();
    let capture_to = Square::from_usi("3e").unwrap();
    let mv = Move32::normal(capture_from, capture_to, pos.piece_on(capture_from));
    pos.apply_move32(mv);

    assert_ne!(pos.hand(Color::BLACK), initial_black_hand);

    pos.undo_move32(mv).unwrap();

    assert_eq!(pos.to_sfen(None), initial_sfen);
    assert_eq!(pos.key(), initial_zobrist);
    assert_eq!(pos.hand(Color::BLACK), initial_black_hand);
}

#[test]
// 駒打ちのundoで持ち駒と盤面が元通りになるか検証
fn test_undo_drop_move() {
    let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPP1PPPP/1B5R1/LNSGKGSNL b P 1";
    let mut pos = crate::board::position_from_sfen(sfen).unwrap();

    let initial_sfen = pos.to_sfen(None);
    let initial_zobrist = pos.key();
    let initial_black_hand = pos.hand(Color::BLACK);

    let to = Square::from_file_rank(crate::types::File::FILE_5, crate::types::Rank::RANK_5);
    let mv = Move32::drop(PieceType::PAWN, to, Color::BLACK);
    pos.apply_move32(mv);

    assert_ne!(pos.hand(Color::BLACK), initial_black_hand);
    assert_eq!(pos.piece_on(to).piece_type(), PieceType::PAWN);

    pos.undo_move32(mv).unwrap();

    assert_eq!(pos.to_sfen(None), initial_sfen);
    assert_eq!(pos.key(), initial_zobrist);
    assert_eq!(pos.hand(Color::BLACK), initial_black_hand);
    assert_eq!(pos.piece_on(to), Piece::NONE);
}

#[test]
// 成りを含む手のundoで成と非成が適切に戻るか確認
fn test_undo_promote_move() {
    let sfen = "4k4/9/4P4/9/9/9/9/9/4K4 b - 1";
    let mut pos = crate::board::position_from_sfen(sfen).unwrap();

    let initial_sfen = pos.to_sfen(None);
    let initial_zobrist = pos.key();

    let from = Square::from_usi("5c").unwrap();
    let to = Square::from_usi("5b").unwrap();
    let piece = pos.piece_on(from);
    let mv = Move32::promotion(from, to, piece);
    assert!(pos.is_legal_move32(mv));
    pos.apply_move32(mv);

    assert!(is_promoted_piece(pos.piece_on(to)));

    pos.undo_move32(mv).unwrap();

    assert_eq!(pos.to_sfen(None), initial_sfen);
    assert_eq!(pos.key(), initial_zobrist);
    assert_eq!(pos.piece_on(from).piece_type(), PieceType::PAWN);
    assert!(!is_promoted_piece(pos.piece_on(from)));
}

#[test]
// Moveの駒打ちと成りでapply/undoが往復するか検証
fn test_undo_move_drop_and_promote() {
    let drop_sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPP1PPPP/1B5R1/LNSGKGSNL b P 1";
    let mut drop_pos = crate::board::position_from_sfen(drop_sfen).unwrap();
    let drop_initial_sfen = drop_pos.to_sfen(None);
    let drop_initial_zobrist = drop_pos.key();
    let drop_mv16 = Move::from_usi("P*5e").unwrap();
    assert!(drop_pos.is_legal_move(drop_mv16));

    drop_pos.apply_move(drop_mv16);
    drop_pos.undo_move(drop_mv16).unwrap();

    assert_eq!(drop_pos.to_sfen(None), drop_initial_sfen);
    assert_eq!(drop_pos.key(), drop_initial_zobrist);

    let promote_sfen = "4k4/9/4P4/9/9/9/9/9/4K4 b - 1";
    let mut promote_pos = crate::board::position_from_sfen(promote_sfen).unwrap();
    let promote_initial_sfen = promote_pos.to_sfen(None);
    let promote_initial_zobrist = promote_pos.key();
    let promote_mv16 = Move::from_usi("5c5b+").unwrap();
    assert!(promote_pos.is_legal_move(promote_mv16));

    promote_pos.apply_move(promote_mv16);
    promote_pos.undo_move(promote_mv16).unwrap();

    assert_eq!(promote_pos.to_sfen(None), promote_initial_sfen);
    assert_eq!(promote_pos.key(), promote_initial_zobrist);
}

#[test]
// 複数手を進めたあと逆順にundoして初期局面に戻れるか検証
fn test_undo_multiple_moves_in_reverse_order() {
    let mut pos = crate::board::hirate_position();

    let initial_sfen = pos.to_sfen(None);
    let initial_zobrist = pos.key();

    let from1 = Square::from_file_rank(crate::types::File::FILE_7, crate::types::Rank::RANK_7);
    let to1 = Square::from_file_rank(crate::types::File::FILE_7, crate::types::Rank::RANK_6);
    let piece1 = pos.piece_on(from1);
    let mv1 = Move32::normal(from1, to1, piece1);
    pos.apply_move32(mv1);

    let from2 = Square::from_file_rank(crate::types::File::FILE_3, crate::types::Rank::RANK_3);
    let to2 = Square::from_file_rank(crate::types::File::FILE_3, crate::types::Rank::RANK_4);
    let piece2 = pos.piece_on(from2);
    let mv2 = Move32::normal(from2, to2, piece2);
    pos.apply_move32(mv2);

    let from3 = Square::from_file_rank(crate::types::File::FILE_2, crate::types::Rank::RANK_7);
    let to3 = Square::from_file_rank(crate::types::File::FILE_2, crate::types::Rank::RANK_6);
    let piece3 = pos.piece_on(from3);
    let mv3 = Move32::normal(from3, to3, piece3);
    pos.apply_move32(mv3);

    for mv in [mv1, mv2, mv3].iter().rev() {
        pos.undo_move32(*mv).unwrap();
    }

    assert_eq!(pos.to_sfen(None), initial_sfen);
    assert_eq!(pos.key(), initial_zobrist);
}

#[test]
// スタック不足時にStackUnderflowが返るか確認
fn test_undo_stack_underflow_returns_error() {
    let mut pos = crate::board::hirate_position();

    let from = Square::from_file_rank(crate::types::File::FILE_7, crate::types::Rank::RANK_7);
    let to = Square::from_file_rank(crate::types::File::FILE_7, crate::types::Rank::RANK_6);
    let piece = pos.piece_on(from);
    let mv = Move32::normal(from, to, piece);
    let result = pos.undo_move32(mv);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), MoveError::StackUnderflow);
}

#[test]
// undoでplies_from_nullなどの状態が復元されるか検証
fn test_undo_restores_repetition_state() {
    let mut pos = crate::board::hirate_position();

    let mv1 = move_from_usi(&pos, "7g7f");
    pos.apply_move32(mv1);
    let plies_after_first = pos.state_stack().current().plies_from_null;

    let mv2 = move_from_usi(&pos, "8c8d");
    pos.apply_move32(mv2);

    pos.undo_move32(mv2).unwrap();

    assert_eq!(pos.state_stack().current().plies_from_null, plies_after_first);
}

#[test]
fn test_apply_move32_with_delta_normal_move() {
    let mut pos = crate::board::hirate_position();
    let mv = move_from_usi(&pos, "7g7f");
    let gives_check = pos.gives_check_move32(mv);

    let delta = pos.apply_move32_with_delta(mv, gives_check);

    assert_eq!(
        delta,
        MoveDelta32::Board {
            color: Color::BLACK,
            from: Square::from_usi("7g").unwrap(),
            to: Square::from_usi("7f").unwrap(),
            moved_piece_before: Piece::B_PAWN,
            moved_piece_after: Piece::B_PAWN,
            captured: None,
        }
    );
}

#[test]
fn test_apply_move32_with_delta_capture_move() {
    let mut pos = crate::board::hirate_position();

    apply_usi_move(&mut pos, "7g7f");
    apply_usi_move(&mut pos, "3c3d");
    apply_usi_move(&mut pos, "3g3f");
    apply_usi_move(&mut pos, "3d3e");

    let mv = move_from_usi(&pos, "3f3e");
    let gives_check = pos.gives_check_move32(mv);

    let delta = pos.apply_move32_with_delta(mv, gives_check);

    assert_eq!(
        delta,
        MoveDelta32::Board {
            color: Color::BLACK,
            from: Square::from_usi("3f").unwrap(),
            to: Square::from_usi("3e").unwrap(),
            moved_piece_before: Piece::B_PAWN,
            moved_piece_after: Piece::B_PAWN,
            captured: Some(CapturedPieceDelta { piece: Piece::W_PAWN, hand_count_before: 0 }),
        }
    );
}

#[test]
fn test_apply_move32_with_delta_drop_move() {
    let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPP1PPPP/1B5R1/LNSGKGSNL b P 1";
    let mut pos = crate::board::position_from_sfen(sfen).unwrap();
    let mv = Move32::drop(PieceType::PAWN, Square::from_usi("5e").unwrap(), Color::BLACK);
    let gives_check = pos.gives_check_move32(mv);

    let delta = pos.apply_move32_with_delta(mv, gives_check);

    assert_eq!(
        delta,
        MoveDelta32::Drop {
            color: Color::BLACK,
            piece_type: PieceType::PAWN,
            to: Square::from_usi("5e").unwrap(),
            hand_count_before: 1,
        }
    );
}

#[test]
fn test_apply_move32_with_delta_promotion_move() {
    let sfen = "4k4/9/4P4/9/9/9/9/9/4K4 b - 1";
    let mut pos = crate::board::position_from_sfen(sfen).unwrap();
    let mv = Move32::promotion(
        Square::from_usi("5c").unwrap(),
        Square::from_usi("5b").unwrap(),
        Piece::B_PAWN,
    );
    let gives_check = pos.gives_check_move32(mv);

    let delta = pos.apply_move32_with_delta(mv, gives_check);

    assert_eq!(
        delta,
        MoveDelta32::Board {
            color: Color::BLACK,
            from: Square::from_usi("5c").unwrap(),
            to: Square::from_usi("5b").unwrap(),
            moved_piece_before: Piece::B_PAWN,
            moved_piece_after: Piece::B_PRO_PAWN,
            captured: None,
        }
    );
}

#[test]
fn test_apply_move32_with_facts_normal_move() {
    let mut pos = crate::board::hirate_position();
    let mv = move_from_usi(&pos, "7g7f");
    let gives_check = pos.gives_check_move32(mv);

    let facts = pos.apply_move32_with_facts(mv, gives_check);

    assert_eq!(facts.delta, pos.undo_move32_with_delta(mv).unwrap());
    let _ = pos.apply_move32_with_delta(mv, gives_check);
    assert_eq!(facts.mv, mv);
    assert_eq!(facts.side_moved, Color::BLACK);
    assert_eq!(facts.side_to_move_after, Color::WHITE);
    assert_eq!(facts.from, Some(Square::from_usi("7g").unwrap()));
    assert_eq!(facts.to, Square::from_usi("7f").unwrap());
    assert_eq!(facts.moved_piece_before, Piece::B_PAWN);
    assert_eq!(facts.moved_piece_after, Piece::B_PAWN);
    assert_eq!(facts.captured_piece, Piece::NONE);
    assert!(!facts.is_capture());
    assert!(!facts.is_drop());
    assert!(!facts.promoted);
    assert!(!facts.moved_king);
    assert_eq!(facts.gives_check, gives_check);
    assert_eq!(facts.board_key_after, pos.board_key());
    assert_eq!(facts.hand_key_after, pos.hand_key());
    assert_eq!(facts.key_after, pos.key());
    assert_eq!(facts.partial_keys_after, pos.partial_keys());
    assert_eq!(facts.search_stack_site().moved_piece, Piece::B_PAWN);
    assert_eq!(facts.search_stack_site().to, Square::from_usi("7f").unwrap());
}

#[test]
fn test_apply_move32_with_facts_capture_move() {
    let mut pos = crate::board::hirate_position();

    apply_usi_move(&mut pos, "7g7f");
    apply_usi_move(&mut pos, "3c3d");
    apply_usi_move(&mut pos, "3g3f");
    apply_usi_move(&mut pos, "3d3e");

    let mv = move_from_usi(&pos, "3f3e");
    let gives_check = pos.gives_check_move32(mv);
    let facts = pos.apply_move32_with_facts(mv, gives_check);

    assert_eq!(facts.captured_piece, Piece::W_PAWN);
    assert_eq!(facts.captured_hand_count_before, Some(0));
    assert!(facts.is_capture());
    assert_eq!(facts.moved_piece_after, Piece::B_PAWN);
    assert_eq!(facts.key_after, pos.key());
    assert_eq!(facts.partial_keys_after, pos.partial_keys());
}

#[test]
fn test_apply_move32_with_facts_drop_move() {
    let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPP1PPPP/1B5R1/LNSGKGSNL b P 1";
    let mut pos = crate::board::position_from_sfen(sfen).unwrap();
    let mv = Move32::drop(PieceType::PAWN, Square::from_usi("5e").unwrap(), Color::BLACK);
    let gives_check = pos.gives_check_move32(mv);

    let facts = pos.apply_move32_with_facts(mv, gives_check);

    assert_eq!(facts.from, None);
    assert_eq!(facts.to, Square::from_usi("5e").unwrap());
    assert_eq!(facts.moved_piece_before, Piece::NONE);
    assert_eq!(facts.moved_piece_after, Piece::B_PAWN);
    assert_eq!(facts.dropped_piece_type, Some(PieceType::PAWN));
    assert_eq!(facts.drop_hand_count_before, Some(1));
    assert!(facts.is_drop());
    assert_eq!(facts.key_after, pos.key());
    assert_eq!(facts.partial_keys_after, pos.partial_keys());
}

#[test]
fn test_apply_move32_with_facts_promotion_move() {
    let sfen = "4k4/9/4P4/9/9/9/9/9/4K4 b - 1";
    let mut pos = crate::board::position_from_sfen(sfen).unwrap();
    let mv = Move32::promotion(
        Square::from_usi("5c").unwrap(),
        Square::from_usi("5b").unwrap(),
        Piece::B_PAWN,
    );
    let gives_check = pos.gives_check_move32(mv);

    let facts = pos.apply_move32_with_facts(mv, gives_check);

    assert_eq!(facts.moved_piece_before, Piece::B_PAWN);
    assert_eq!(facts.moved_piece_after, Piece::B_PRO_PAWN);
    assert!(facts.promoted);
    assert!(!facts.moved_king);
    assert_eq!(facts.key_after, pos.key());
    assert_eq!(facts.partial_keys_after, pos.partial_keys());
}

#[test]
fn test_apply_move32_with_facts_king_move() {
    let sfen = "4k4/9/9/9/9/9/9/9/4K4 b - 1";
    let mut pos = crate::board::position_from_sfen(sfen).unwrap();
    let mv = Move32::normal(
        Square::from_usi("5i").unwrap(),
        Square::from_usi("5h").unwrap(),
        Piece::B_KING,
    );
    let gives_check = pos.gives_check_move32(mv);

    let facts = pos.apply_move32_with_facts(mv, gives_check);

    assert_eq!(facts.moved_piece_before, Piece::B_KING);
    assert_eq!(facts.moved_piece_after, Piece::B_KING);
    assert!(facts.moved_king);
    assert_eq!(facts.search_stack_site().moved_piece, Piece::B_KING);
    assert_eq!(facts.search_stack_site().to, Square::from_usi("5h").unwrap());
    assert_eq!(facts.key_after, pos.key());
    assert_eq!(facts.partial_keys_after, pos.partial_keys());
}

#[test]
fn test_undo_move32_with_delta_matches_apply_delta_normal_move() {
    let mut pos = crate::board::hirate_position();
    let mv = move_from_usi(&pos, "7g7f");
    assert_apply_and_undo_delta_roundtrip(&mut pos, mv);
}

#[test]
fn test_undo_move32_with_delta_matches_apply_delta_capture_move() {
    let mut pos = crate::board::hirate_position();

    apply_usi_move(&mut pos, "7g7f");
    apply_usi_move(&mut pos, "3c3d");
    apply_usi_move(&mut pos, "3g3f");
    apply_usi_move(&mut pos, "3d3e");

    let mv = move_from_usi(&pos, "3f3e");
    assert_apply_and_undo_delta_roundtrip(&mut pos, mv);
}

#[test]
fn test_undo_move32_with_delta_matches_apply_delta_drop_move() {
    let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPP1PPPP/1B5R1/LNSGKGSNL b P 1";
    let mut pos = crate::board::position_from_sfen(sfen).unwrap();
    let mv = Move32::drop(PieceType::PAWN, Square::from_usi("5e").unwrap(), Color::BLACK);
    assert_apply_and_undo_delta_roundtrip(&mut pos, mv);
}

#[test]
fn test_undo_move32_with_delta_matches_apply_delta_promotion_move() {
    let sfen = "4k4/9/4P4/9/9/9/9/9/4K4 b - 1";
    let mut pos = crate::board::position_from_sfen(sfen).unwrap();
    let mv = Move32::promotion(
        Square::from_usi("5c").unwrap(),
        Square::from_usi("5b").unwrap(),
        Piece::B_PAWN,
    );
    assert_apply_and_undo_delta_roundtrip(&mut pos, mv);
}
