use super::helpers::apply_usi_move;
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
use crate::types::{
    Color, File, Hand, MOVE_NONE, Move, Piece, PieceType, Rank, SQ_51, SQ_55, SQ_59, Square,
};

/// `PackedPiece` の基本的なエンコードとデコード動作を検証
#[test]
fn test_packed_piece_encoding_roundtrip() {
    assert_eq!(PackedPiece::EMPTY.raw(), 0);
    assert!(PackedPiece::EMPTY.is_empty());

    let black_pawn = PackedPiece::new(PieceType::PAWN, Color::BLACK, false);
    assert_eq!(black_pawn.piece_type(), PieceType::PAWN);
    assert_eq!(black_pawn.color(), Color::BLACK);
    assert!(!black_pawn.is_promoted());

    let white_pro_silver = PackedPiece::new(PieceType::SILVER, Color::WHITE, true);
    assert_eq!(white_pro_silver.piece_type(), PieceType::SILVER);
    assert_eq!(white_pro_silver.color(), Color::WHITE);
    assert!(white_pro_silver.is_promoted());

    let pieces = [
        Piece::B_PAWN,
        Piece::W_LANCE,
        Piece::B_KNIGHT,
        Piece::W_SILVER,
        Piece::B_GOLD,
        Piece::W_BISHOP,
        Piece::B_ROOK,
        Piece::W_KING,
    ];

    for piece in pieces {
        let packed = PackedPiece::from_piece(piece);
        let converted = packed.to_piece();
        assert_eq!(piece, converted);
    }
}

/// `BoardArray` が配列として正常に動作するかを確認
#[test]
fn test_board_array_behaves_like_81_square_buffer() {
    let mut board = BoardArray::empty();
    board.set(SQ_55, Piece::B_KING);

    assert!(!board.get(SQ_55).is_empty());
    assert_eq!(board.get(SQ_55).piece_type(), PieceType::KING);

    let non_empty_count = board.iter().filter(|(_, p)| !p.is_empty()).count();
    assert_eq!(non_empty_count, 1);
}

/// `Position::new` と `from_startpos` の初期状態を検証
#[test]
fn test_position_basic_state_and_startpos() {
    let pos = Position::empty();
    assert_eq!(pos.turn(), Color::BLACK);
    assert_eq!(pos.game_ply(), 1);
    assert_eq!(pos.hand(Color::BLACK), Hand::ZERO);
    assert_eq!(pos.hand(Color::WHITE), Hand::ZERO);

    let start = crate::board::hirate_position();

    assert_eq!(start.piece_on(SQ_59).piece_type(), PieceType::KING);
    assert_eq!(start.piece_on(SQ_59).color(), Color::BLACK);
    assert_eq!(start.piece_on(SQ_51).piece_type(), PieceType::KING);
    assert_eq!(start.piece_on(SQ_51).color(), Color::WHITE);

    assert_eq!(start.turn(), Color::BLACK);
    assert_eq!(start.game_ply(), 1);
}

#[test]
fn test_current_state_helpers_match_state_stack() {
    let mut pos = crate::board::hirate_position();

    apply_usi_move(&mut pos, "7g7f");
    apply_usi_move(&mut pos, "3c3d");

    let current = pos.current_state();
    let stack_current = pos.state_stack().current();

    assert_eq!(pos.state_stack_depth(), pos.state_stack().depth());
    assert_eq!(current.board_key, stack_current.board_key);
    assert_eq!(current.hand_key, stack_current.hand_key);
    assert_eq!(current.repetition_counter, stack_current.repetition_counter);
    assert_eq!(current.repetition_distance, stack_current.repetition_distance);
    assert_eq!(pos.last_move(), stack_current.last_move());
    assert_eq!(pos.last_moved_piece_type(), stack_current.last_moved_piece_type());
    assert_eq!(pos.repetition_counter(), stack_current.repetition_counter);
    assert_eq!(pos.repetition_distance(), stack_current.repetition_distance);
}

/// SFEN との往復と最低限の局面検証が成立するかを確認
/// （`is_valid` を使うため validation feature 必須。SFEN 往復自体は roundtrip.rs で被覆）
#[cfg(feature = "validation")]
#[test]
fn test_position_sfen_roundtrip_and_validation() {
    let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";

    let pos = crate::board::position_from_sfen(sfen).unwrap();
    let generated = pos.to_sfen(None);
    assert_eq!(sfen, generated);

    let mut invalid = Position::empty();
    assert!(invalid.is_valid());

    invalid.board.set(SQ_59, Piece::B_KING);
    invalid.board.set(SQ_51, Piece::W_KING);
    invalid.rebuild_bitboards();

    assert!(invalid.is_valid());
}

/// `flipped_sfen` が盤面はそのままで先後反転することを確認
#[test]
fn test_position_flipped_sfen_matches_yaneuraou() {
    let sfen = "lnsgk1snl/1r4g2/p1ppppb1p/6pP1/7R1/2P6/P2PPPP1P/1SG6/LN2KGSNL b BP2p 21";
    let expected = "lnsgk2nl/6gs1/p1pppp2p/6p2/1r7/1pP6/P1BPPPP1P/2G4R1/LNS1KGSNL w 2Pbp 21";

    let pos = crate::board::position_from_sfen(sfen).unwrap();
    assert_eq!(pos.to_sfen_flipped(None), expected);
}

#[test]
fn test_position_move_from_csa_and_move() {
    let pos = crate::board::hirate_position();

    let mv = pos.move_from_csa("7776FU");
    assert_eq!(mv.to_usi(), "7g7f");

    let mv = pos.move_from_csa("+7776FU");
    assert_eq!(mv.to_usi(), "7g7f");

    let mv16 = Move::normal("7g".parse().unwrap(), "7f".parse().unwrap());
    let mv = pos.move32_from_move(mv16);
    assert_eq!(mv.to_usi(), "7g7f");

    let mut promo_pos = Position::empty();
    let from = "2b".parse::<Square>().unwrap();
    promo_pos.board.set(from, Piece::B_BISHOP);
    let mv = promo_pos.move_from_csa("2233UM");
    assert_eq!(mv.to_usi(), "2b3c+");
}

#[test]
fn test_position_move_from_csa_rejects_non_ascii_without_panic() {
    let pos = crate::board::hirate_position();

    assert_eq!(pos.move_from_csa("ああ"), MOVE_NONE);
    assert_eq!(pos.move_from_csa("+ああ"), MOVE_NONE);
}

/// 通常手、捕獲、打ち、成り、不正手など `apply_move32` 派生の基本挙動を網羅
#[test]
fn test_apply_move_variants() {
    let mut pos = crate::board::hirate_position();

    let from = Square::from_file_rank(File::FILE_7, Rank::RANK_7);
    let to = Square::from_file_rank(File::FILE_7, Rank::RANK_6);
    let piece = pos.piece_on(from);
    let mv = Move32::normal(from, to, piece);

    let initial_ply = pos.game_ply();
    pos.apply_move32(mv);
    assert_eq!(pos.piece_on(from), Piece::NONE);
    assert_eq!(pos.piece_on(to), piece);
    assert_eq!(pos.turn(), Color::WHITE);
    assert_eq!(pos.game_ply(), initial_ply + 1);

    let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/4p4/4P4/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
    let mut capture_pos = crate::board::position_from_sfen(sfen).unwrap();

    let from = Square::from_file_rank(File::FILE_5, Rank::RANK_6);
    let to = Square::from_file_rank(File::FILE_5, Rank::RANK_5);
    let piece = capture_pos.piece_on(from);
    let captured = capture_pos.piece_on(to);
    let mv = Move32::normal(from, to, piece);
    capture_pos.apply_move32(mv);
    let state_stack = capture_pos.state_stack();
    let state = state_stack.current();
    assert_eq!(state.cold.captured_piece(), captured);
    assert_eq!(capture_pos.hand(Color::BLACK).pawn_count(), 1);

    let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPP1PPPP/1B5R1/LNSGKGSNL b P 1";
    let mut drop_pos = crate::board::position_from_sfen(sfen).unwrap();
    let to = Square::from_file_rank(File::FILE_5, Rank::RANK_5);
    let mv = Move32::drop(PieceType::PAWN, to, Color::BLACK);
    let initial_hand = drop_pos.hand(Color::BLACK);
    drop_pos.apply_move32(mv);
    let placed_piece = drop_pos.piece_on(to);
    assert_eq!(placed_piece, Piece::B_PAWN);
    assert_eq!(drop_pos.hand(Color::BLACK).pawn_count(), initial_hand.pawn_count() - 1);
    assert_eq!(drop_pos.state_stack().current().cold.captured_piece(), Piece::NONE);

    let sfen = "lnsgkgsnl/1r5b1/2R6/ppppppppp/9/9/PPPPPPPPP/1B7/LNSGKGSNL b - 1";
    let mut promote_pos = crate::board::position_from_sfen(sfen).unwrap();
    let from = Square::from_file_rank(File::FILE_7, Rank::RANK_3);
    let to = Square::from_file_rank(File::FILE_7, Rank::RANK_2);
    let piece = promote_pos.piece_on(from);
    let mv = Move32::promotion(from, to, piece);
    promote_pos.apply_move32(mv);
    assert!(is_promoted_piece(promote_pos.piece_on(to)));

    let invalid_move_pos = crate::board::hirate_position();

    let from = Square::from_file_rank(File::FILE_5, Rank::RANK_5);
    let to = Square::from_file_rank(File::FILE_5, Rank::RANK_4);
    let mv = Move32::normal(from, to, Piece::B_PAWN);
    assert!(!invalid_move_pos.is_legal_move32(mv));

    let to = Square::from_file_rank(File::FILE_5, Rank::RANK_5);
    let mv = Move32::drop(PieceType::PAWN, to, Color::BLACK);
    assert!(!invalid_move_pos.is_legal_move32(mv));
}

#[test]
fn test_move32_metadata_classifies_capture_drop_and_promotion() {
    let start = crate::board::hirate_position();
    let quiet = start.move32_from_move(Move::from_usi("7g7f").expect("valid move"));
    let quiet_meta = start.move32_metadata(quiet);
    assert_eq!(quiet_meta.piece_after_move(), Piece::B_PAWN);
    assert!(!quiet_meta.is_capture());
    assert!(!quiet_meta.is_drop());
    assert!(!quiet_meta.is_promotion());
    assert_eq!(quiet_meta.from_to_index(), quiet.from_to_index());

    let capture_pos = crate::board::position_from_sfen(
        "lnsgkgsnl/1r5b1/ppppppppp/9/4p4/4P4/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
    )
    .expect("valid sfen");
    let capture = capture_pos.move32_from_move(Move::from_usi("5f5e").expect("valid move"));
    let capture_meta = capture_pos.classify_move32(capture);
    assert!(capture_meta.is_capture());
    assert_eq!(capture_meta.captured_piece(), Piece::W_PAWN);
    assert_eq!(capture_meta.captured_piece_type(), PieceType::PAWN);
    assert_eq!(capture_meta.from_to_index(), capture.from_to_index());

    let drop_pos = crate::board::position_from_sfen(
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPP1PPPP/1B5R1/LNSGKGSNL b P 1",
    )
    .expect("valid sfen");
    let drop = Move32::drop(PieceType::PAWN, "5e".parse().expect("valid square"), Color::BLACK);
    let drop_meta = drop_pos.move32_metadata(drop);
    assert!(drop_meta.is_drop());
    assert_eq!(drop_meta.piece_after_move(), Piece::B_PAWN);
    assert!(!drop_meta.is_capture());
    assert_eq!(drop_meta.from_to_index(), drop.from_to_index());

    let promo_pos = crate::board::position_from_sfen(
        "lnsgkgsnl/1r5b1/2R6/ppppppppp/9/9/PPPPPPPPP/1B7/LNSGKGSNL b - 1",
    )
    .expect("valid sfen");
    let promo = promo_pos.move32_from_move(Move::from_usi("7c7b+").expect("valid move"));
    let promo_meta = promo_pos.move32_metadata(promo);
    assert!(promo_meta.is_promotion());
    assert_eq!(promo_meta.piece_after_move(), Piece::B_DRAGON);
    assert_eq!(promo_meta.from_to_index(), promo.from_to_index());
}
