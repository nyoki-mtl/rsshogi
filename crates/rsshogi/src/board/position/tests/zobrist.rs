use super::helpers::{apply_usi_move, move_from_usi};
use super::*;
use crate::board::zobrist::{Zobrist, ZobristKey};
use crate::types::{Hand, HandPiece, Piece, PieceType, Square};

fn expected_pawn_key(pos: &Position) -> ZobristKey {
    let mut key = Zobrist::no_pawns();
    for (sq, piece) in pos.board_array().iter() {
        if piece.piece_type() == PieceType::PAWN {
            key ^= Zobrist::psq(sq, piece);
        }
    }
    key
}

fn expected_minor_piece_key(pos: &Position) -> ZobristKey {
    let mut key = ZobristKey::default();
    for (sq, piece) in pos.board_array().iter() {
        if piece.piece_type().is_minor() {
            key ^= Zobrist::psq(sq, piece);
        }
    }
    key
}

fn expected_non_pawn_key(pos: &Position, color: Color) -> ZobristKey {
    let mut key = ZobristKey::default();
    for (sq, piece) in pos.board_array().iter() {
        if piece != Piece::NONE && piece.color() == color && piece.piece_type() != PieceType::PAWN {
            key ^= Zobrist::psq(sq, piece);
        }
    }
    key
}

fn expected_material_key(pos: &Position) -> ZobristKey {
    let mut key = ZobristKey::default();
    for (_sq, piece) in pos.board_array().iter() {
        if piece != Piece::NONE {
            key.add(Zobrist::material(piece));
        }
    }
    key
}

fn expected_material_value(pos: &Position) -> i32 {
    const MATERIAL_VALUES: [i32; PieceType::COUNT] =
        [0, 90, 315, 405, 495, 855, 990, 540, 15_000, 540, 540, 540, 540, 945, 1_395, 0];

    let mut value = 0;
    for (_sq, piece) in pos.board_array().iter() {
        if piece != Piece::NONE {
            let sign = if piece.color() == Color::BLACK { 1 } else { -1 };
            value += sign * MATERIAL_VALUES[piece.piece_type().to_index()];
        }
    }

    for color in [Color::BLACK, Color::WHITE] {
        let hand = pos.hand(color);
        let sign = if color == Color::BLACK { 1 } else { -1 };
        for hp in HandPiece::iter() {
            let count = Hand::count_of(hand, hp);
            if count == 0 {
                continue;
            }
            value += sign
                * MATERIAL_VALUES[hp.to_piece_type().to_index()]
                * i32::try_from(count).expect("hand count fits in i32");
        }
    }

    value
}

fn expected_partial_keys(pos: &Position) -> PartialKeys {
    PartialKeys::new(
        expected_pawn_key(pos),
        expected_minor_piece_key(pos),
        [expected_non_pawn_key(pos, Color::BLACK), expected_non_pawn_key(pos, Color::WHITE)],
        expected_material_key(pos),
        expected_material_value(pos),
    )
}

fn assert_partial_keys_match_reference(pos: &Position) {
    let expected = expected_partial_keys(pos);
    assert_eq!(pos.partial_keys(), expected);
    assert_eq!(pos.current_state().partial_keys(), expected);
    pos.debug_assert_partial_keys_consistent();
}

#[test]
// 通常手適用後のZobrist更新内容が理論値と一致するか検証
fn test_zobrist_after_normal_move_matches_expected() {
    let mut pos = crate::board::hirate_position();

    let initial_zobrist = pos.key();
    let mut board_key = pos.board_key();
    let hand_key = pos.hand_key();

    apply_usi_move(&mut pos, "7g7f");

    assert_ne!(pos.key(), initial_zobrist);

    let from = Square::from_usi("7g").unwrap();
    let to = Square::from_usi("7f").unwrap();
    board_key ^= Zobrist::psq(from, Piece::B_PAWN);
    board_key ^= Zobrist::psq(to, Piece::B_PAWN);
    board_key ^= Zobrist::side();
    let expected = board_key ^ hand_key;

    assert_eq!(pos.key(), expected);
}

#[test]
// 捕獲を含む手でZobrist更新が期待値通りか確認
fn test_zobrist_after_capture_matches_expected() {
    let sfen = "lnsgk1snl/1r4gb1/p1pppp2p/6pp1/1p7/2P6/PP1PPPP1P/1BG4R1/LNS1KGSNL b p 11";
    let mut pos = crate::board::position_from_sfen(sfen).unwrap();

    let initial_zobrist = pos.key();
    let mut board_key = pos.board_key();
    let mut hand_key = pos.hand_key();

    let mv = move_from_usi(&pos, "2h2d");
    let from = mv.from_sq();
    let to = mv.to_sq();
    let moved_piece = pos.piece_on(from);
    let captured_piece = pos.piece_on(to);
    let us = pos.turn();
    assert!(pos.is_legal_move32(mv));
    pos.apply_move32(mv);

    assert_ne!(pos.key(), initial_zobrist);

    let moved_after = if mv.is_promotion() { moved_piece.promote() } else { moved_piece };
    board_key ^= Zobrist::psq(from, moved_piece);
    if captured_piece != Piece::NONE {
        board_key ^= Zobrist::psq(to, captured_piece);
    }
    board_key ^= Zobrist::psq(to, moved_after);
    board_key ^= Zobrist::side();
    if captured_piece != Piece::NONE {
        hand_key.add(Zobrist::hand(us, captured_piece.piece_type().demote(), 1));
    }
    let expected = board_key ^ hand_key;

    assert_eq!(pos.key(), expected);
}

#[test]
// 駒打ち時のZobrist更新が正しく行われるか検証
fn test_zobrist_after_drop_matches_expected() {
    let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPP1PPPP/1B5R1/LNSGKGSNL b P 1";
    let mut pos = crate::board::position_from_sfen(sfen).unwrap();

    let initial_zobrist = pos.key();
    let mut board_key = pos.board_key();
    let mut hand_key = pos.hand_key();

    apply_usi_move(&mut pos, "P*5e");

    assert_ne!(pos.key(), initial_zobrist);

    let to = Square::from_usi("5e").unwrap();
    board_key ^= Zobrist::psq(to, Piece::B_PAWN);
    board_key ^= Zobrist::side();
    hand_key.sub(Zobrist::hand(Color::BLACK, PieceType::PAWN, 1));
    let expected = board_key ^ hand_key;

    assert_eq!(pos.key(), expected);
}

#[test]
// 成りを含む手のZobrist更新が期待通りか確認
fn test_zobrist_after_promote_matches_expected() {
    let sfen = "lnsgk1snl/1p4g2/pR1ppp2p/2p6/9/9/P1SPPPP1P/2G6/LN2KGSNL b B3Prb2p 25";
    let mut pos = crate::board::position_from_sfen(sfen).unwrap();

    let mut board_key = pos.board_key();
    let hand_key = pos.hand_key();

    let mv = move_from_usi(&pos, "8c8f+");
    let from = mv.from_sq();
    let to = mv.to_sq();
    let moved_piece = pos.piece_on(from);
    assert!(pos.is_legal_move32(mv));
    pos.apply_move32(mv);

    assert_ne!(pos.key(), board_key ^ hand_key);

    board_key ^= Zobrist::psq(from, moved_piece);
    board_key ^= Zobrist::psq(to, moved_piece.promote());
    board_key ^= Zobrist::side();
    let expected = board_key ^ hand_key;

    assert_eq!(pos.key(), expected);
}

#[test]
// 異なる手順で同一局面に到達した際Zobristが一致するか確認
fn test_zobrist_is_consistent_for_equivalent_sequences() {
    let mut pos1 = crate::board::hirate_position();

    let from1 = Square::from_file_rank(crate::types::File::FILE_7, crate::types::Rank::RANK_7);
    let to1 = Square::from_file_rank(crate::types::File::FILE_7, crate::types::Rank::RANK_6);
    let piece1 = pos1.piece_on(from1);
    let mv1 = Move32::normal(from1, to1, piece1);
    pos1.apply_move32(mv1);

    let from2 = Square::from_file_rank(crate::types::File::FILE_3, crate::types::Rank::RANK_3);
    let to2 = Square::from_file_rank(crate::types::File::FILE_3, crate::types::Rank::RANK_4);
    let piece2 = pos1.piece_on(from2);
    let mv2 = Move32::normal(from2, to2, piece2);
    pos1.apply_move32(mv2);

    let sfen = "lnsgkgsnl/1r5b1/pppppp1pp/6p2/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL b - 3";
    let pos2 = crate::board::position_from_sfen(sfen).unwrap();

    assert_eq!(pos1.key(), pos2.key());
}

#[test]
// 初期局面の Zobrist がゼロ値にならないことを確認する。
fn test_zobrist_initial_position_is_non_zero() {
    let pos = crate::board::hirate_position();

    assert_ne!(pos.key().low_u64(), 0);
}

#[test]
// 既知の手順で Zobrist が変化することを確認する。
fn test_zobrist_changes_after_known_sequence() {
    let mut pos = crate::board::hirate_position();
    let initial = pos.key();

    for usi in ["7g7f", "3c3d", "2g2f"] {
        apply_usi_move(&mut pos, usi);
    }

    assert_ne!(pos.key(), initial);
}

#[test]
// Zobristテーブルの特定エントリがゼロでないことを確認
fn test_zobrist_table_sample_entry_is_non_zero() {
    let sq = Square::from_file_rank(crate::types::File::FILE_5, crate::types::Rank::RANK_5);
    let piece = Piece::from_parts(Color::BLACK, PieceType::PAWN);
    let hash = Zobrist::psq(sq, piece);

    assert_ne!(hash.low_u64(), 0);
}

#[test]
fn test_position_key_helpers_match_reference_computation() {
    let sfen = "ln1g3nl/1r3kg2/p2pppsp1/3s2p1p/1pp4P1/P1P1SP2P/1PSPP1P2/2GK3R1/LN3G1NL b Bb 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid sfen");

    assert_partial_keys_match_reference(&pos);
}

#[test]
fn test_position_key_helpers_match_reference_after_sequence() {
    let mut pos = crate::board::hirate_position();

    for usi in ["7g7f", "3c3d", "2g2f", "8c8d", "2f2e", "8d8e", "2e2d", "8e8f", "2d2c+"] {
        apply_usi_move(&mut pos, usi);
    }

    assert_partial_keys_match_reference(&pos);
}

#[test]
fn test_position_partial_keys_match_reference_across_hot_path_scenarios() {
    let hirate = crate::board::hirate_position();
    assert_partial_keys_match_reference(&hirate);

    let mut capture = crate::board::position_from_sfen(
        "lnsgk1snl/1r4gb1/p1pppp2p/6pp1/1p7/2P6/PP1PPPP1P/1BG4R1/LNS1KGSNL b p 11",
    )
    .expect("valid sfen");
    let capture_mv = move_from_usi(&capture, "2h2d");
    capture.apply_move32(capture_mv);
    assert_partial_keys_match_reference(&capture);

    let mut promote = crate::board::position_from_sfen(
        "lnsgk1snl/1p4g2/pR1ppp2p/2p6/9/9/P1SPPPP1P/2G6/LN2KGSNL b B3Prb2p 25",
    )
    .expect("valid sfen");
    let promote_mv = move_from_usi(&promote, "8c8f+");
    promote.apply_move32(promote_mv);
    assert_partial_keys_match_reference(&promote);

    let mut drop = crate::board::position_from_sfen(
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPP1PPPP/1B5R1/LNSGKGSNL b P 1",
    )
    .expect("valid sfen");
    let drop_mv = move_from_usi(&drop, "P*5e");
    drop.apply_move32(drop_mv);
    assert_partial_keys_match_reference(&drop);

    let mut evasion = crate::board::position_from_sfen(
        "l4S2l/4g1gs1/5p1p1/pr2N1pkp/4Gn3/PP3PPPP/2GPP4/1K2r4/L4+s2L b BS2N5Pb 2",
    )
    .expect("valid sfen");
    let evasion_mv = move_from_usi(&evasion, "8h8g");
    evasion.apply_move32(evasion_mv);
    assert_partial_keys_match_reference(&evasion);
}

#[test]
fn test_partial_keys_roundtrip_after_apply_and_undo() {
    let scenarios = [
        ("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1", "7g7f"),
        ("lnsgk1snl/1r4gb1/p1pppp2p/6pp1/1p7/2P6/PP1PPPP1P/1BG4R1/LNS1KGSNL b p 11", "2h2d"),
        ("lnsgk1snl/1p4g2/pR1ppp2p/2p6/9/9/P1SPPPP1P/2G6/LN2KGSNL b B3Prb2p 25", "8c8f+"),
        ("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPP1PPPP/1B5R1/LNSGKGSNL b P 1", "P*5e"),
        ("l4S2l/4g1gs1/5p1p1/pr2N1pkp/4Gn3/PP3PPPP/2GPP4/1K2r4/L4+s2L b BS2N5Pb 2", "8h8g"),
    ];

    for (sfen, usi) in scenarios {
        let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
        let before = pos.partial_keys();
        let before_state = pos.current_state().partial_keys();
        let mv = move_from_usi(&pos, usi);

        pos.apply_move32(mv);
        pos.undo_move32(mv).expect("undo move");

        assert_eq!(pos.partial_keys(), before, "partial keys should restore after {usi}");
        assert_eq!(
            pos.current_state().partial_keys(),
            before_state,
            "state partial keys should restore after {usi}"
        );
        assert_partial_keys_match_reference(&pos);
    }
}

#[test]
fn test_partial_keys_clone_for_search_preserves_current_state() {
    let mut pos = crate::board::hirate_position();
    for usi in ["7g7f", "3c3d", "2g2f", "8c8d", "2f2e", "8d8e", "2e2d", "8e8f", "2d2c+"] {
        apply_usi_move(&mut pos, usi);
    }

    let cloned = pos.clone_for_search();

    assert_eq!(cloned.partial_keys(), pos.partial_keys());
    assert_eq!(cloned.current_state().partial_keys(), pos.current_state().partial_keys());
    assert_partial_keys_match_reference(&cloned);
}

#[test]
fn test_partial_keys_survive_null_move_roundtrip() {
    let mut pos = crate::board::hirate_position();
    apply_usi_move(&mut pos, "7g7f");
    apply_usi_move(&mut pos, "3c3d");

    let before = pos.partial_keys();
    pos.apply_null_move().expect("null move");
    assert_eq!(pos.partial_keys(), before);
    assert_partial_keys_match_reference(&pos);

    pos.undo_null_move().expect("undo null move");
    assert_eq!(pos.partial_keys(), before);
    assert_partial_keys_match_reference(&pos);
}
