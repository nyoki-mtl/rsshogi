//! 1.x の廃止予定 API の公開パスと動作を、外部 crate と同じ入口から検査する。

// 非推奨 API の互換性検査では警告を許可し、維持対象の検査では関数単位で禁止する。
#![allow(deprecated)]

use rsshogi::board::attack_tables::{
    GOLD_ATTACKS, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS, SILVER_ATTACKS,
    gold_attacks_unchecked, king_attacks_unchecked, knight_attacks_unchecked,
    pawn_attacks_unchecked, rook_rank_attacks, silver_attacks_unchecked,
};
use rsshogi::board::movegen::{LegalAll, MoveGenType};
use rsshogi::board::{hirate_position, position_from_sfen};
use rsshogi::types::{Bitboard, Color, File, Hand, HandPiece, Move, PieceType, Square};

#[test]
#[deny(deprecated)]
fn retained_rule_and_attack_primitives_are_not_deprecated() {
    let position = hirate_position();
    assert!(position.has_pawn_on_file(Color::BLACK, File::FILE_5));
    let square = Square::from_usi("5e").unwrap();
    let occupied = Bitboard::EMPTY;
    assert_eq!(
        rsshogi::board::attack_tables::rook_attacks(square, occupied),
        rook_rank_attacks(square, occupied)
            | rsshogi::board::attack_tables::rook_file_attacks(square, occupied)
    );
}

#[test]
fn key_prediction_matches_real_updates_without_mutating_source() {
    let fixtures = [
        ("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1", "7g7f"),
        ("lnsgk1snl/1r4gb1/p1pppp2p/6pp1/1p7/2P6/PP1PPPP1P/1BG4R1/LNS1KGSNL b p 11", "2h2d"),
        ("lnsgk1snl/1p4g2/pR1ppp2p/2p6/9/9/P1SPPPP1P/2G6/LN2KGSNL b B3Prb2p 25", "8c8f+"),
        ("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPP1PPPP/1B5R1/LNSGKGSNL b P 1", "P*5e"),
        ("lnsgkgsnl/1b5r1/pppp1pppp/9/9/9/PPPPPPPPP/1R5B1/LNSGKGSNL w p 1", "P*5e"),
    ];
    for (sfen, usi) in fixtures {
        let position = position_from_sfen(sfen).unwrap();
        let mv = Move::from_usi(usi).unwrap();
        let full = position.move32_from_move(mv);
        assert!(position.is_legal_move(mv), "{usi}");
        let before = (position.key(), position.board_key(), position.game_ply());
        let predicted = position.key_after(full);
        let predicted_board = position.board_key_after(full);
        assert_eq!(predicted, position.key_after_move(mv));
        assert_eq!(predicted_board, position.board_key_after_move(mv));
        assert_eq!(before, (position.key(), position.board_key(), position.game_ply()));
        let mut actual = position.clone();
        actual.apply_move32(full);
        assert_eq!(predicted, actual.key(), "{usi}");
        assert_eq!(predicted_board, actual.board_key(), "{usi}");
    }
    let position = hirate_position();
    let mut actual = position.clone();
    actual.apply_null_move().unwrap();
    assert_eq!(position.key_after_null(), actual.key());
}

#[test]
fn classification_and_metadata_keep_their_published_meaning() {
    for (sfen, usi, capture, pawn_promotion, valuable_promotion) in [
        ("4k4/9/9/9/9/9/4P4/9/4K4 b - 1", "5g5f", false, false, false),
        ("4k4/9/9/9/4p4/4P4/9/9/4K4 b - 1", "5f5e", true, false, false),
        ("4k4/9/4P4/9/9/9/9/9/4K4 b - 1", "5c5b+", false, true, true),
        ("4k4/9/4S4/9/9/9/9/9/4K4 b - 1", "5c4b+", false, false, false),
        ("4k4/9/9/9/9/9/9/9/4K4 b P 1", "P*5e", false, false, false),
    ] {
        let position = position_from_sfen(sfen).unwrap();
        let mv = Move::from_usi(usi).unwrap();
        let full = position.move32_from_move(mv);
        assert!(position.is_legal_move(mv));
        assert_eq!(position.is_capture_or_promotion(full), capture || mv.is_promotion());
        assert_eq!(position.is_pawn_promotion(full), pawn_promotion);
        assert_eq!(position.is_capture_or_pawn_promotion(full), capture || pawn_promotion);
        assert_eq!(position.is_capture_or_valuable_promotion(full), capture || valuable_promotion);
        assert_eq!(position.is_capture_stage(full), capture);
        let metadata: rsshogi::types::moves::Move32Metadata = position.move32_metadata(full);
        let alias: rsshogi::types::Move32Metadata = position.classify_move32(full);
        assert_eq!(metadata, alias);
        assert_eq!(metadata.is_capture(), capture);
        assert_eq!(metadata.is_promotion(), mv.is_promotion());
        assert_eq!(metadata.is_drop(), mv.is_drop());
        assert_eq!(metadata.from_to_index(), full.from_to_index());
        assert_eq!(position.moved_piece_before(full), position.moved_piece_before_move(mv));
        let mut actual = position.clone();
        let facts = actual.try_apply_move32_with_facts(full).unwrap();
        assert_eq!(metadata.piece_after_move(), facts.moved_piece_after);
        assert_eq!(metadata.captured_piece(), facts.captured_piece);
        let site: rsshogi::board::MoveStackSiteFacts = facts.search_stack_site();
        let same_path: rsshogi::board::position::MoveStackSiteFacts = site;
        assert_eq!(same_path.moved_piece, facts.moved_piece_after);
        assert_eq!(same_path.to, facts.to);
    }
}

#[test]
fn helpers_keep_const_generic_and_failure_behavior() {
    const EMPTY: Hand = Hand::sub_one(Hand::ZERO, HandPiece::PAWN);
    assert_eq!(EMPTY, Hand::ZERO);
    let hand = Hand::ZERO.checked_add(HandPiece::PAWN, 2).unwrap();
    assert_eq!(Hand::sub_one(hand, HandPiece::PAWN).pawn_count(), 1);
    for count in [
        hand.lance_count(),
        hand.knight_count(),
        hand.silver_count(),
        hand.bishop_count(),
        hand.rook_count(),
        hand.gold_count(),
    ] {
        assert_eq!(count, 0);
    }
    assert!(LegalAll::is_generate_all_legal());
    let position = hirate_position();
    let types = [PieceType::PAWN, PieceType::GOLD];
    let expected = position.pieces_by_type(types[0]) | position.pieces_by_type(types[1]);
    assert_eq!(position.pieces_by_types(&types), expected);
    assert_eq!(position.pieces_by_types_array(types), expected);
    let expected =
        position.pieces_for(types[0], Color::BLACK) | position.pieces_for(types[1], Color::BLACK);
    assert_eq!(position.pieces_for_types(&types, Color::BLACK), expected);
    assert_eq!(position.pieces_for_types_array(types, Color::BLACK), expected);
    assert_eq!(position.pieces_by_types(&[]), Bitboard::EMPTY);
    assert!(position.has_pawn_on_file(Color::BLACK, File::FILE_5));
    let mv = position.move32_from_move(Move::from_usi("5g5f").unwrap());
    assert_eq!(
        position.is_discovery_check_on_king(Color::BLACK, mv),
        position.blockers_for_king(Color::BLACK).test(mv.to_move().from_sq())
    );
    let checks = position.current_state_cache().check_squares();
    let full = checks.to_full_array();
    for pt in PieceType::iter() {
        assert_eq!(full[pt.to_index()], checks.get(pt));
    }
}

#[test]
fn legacy_attack_functions_match_independent_board_geometry() {
    for square in Square::iter() {
        for color in Color::iter() {
            // SAFETY: square と color は有効な値だけを列挙している。
            unsafe {
                assert_eq!(
                    pawn_attacks_unchecked(square, color),
                    PAWN_ATTACKS[square][color.to_index()]
                );
                assert_eq!(
                    knight_attacks_unchecked(square, color),
                    KNIGHT_ATTACKS[square][color.to_index()]
                );
                assert_eq!(
                    silver_attacks_unchecked(square, color),
                    SILVER_ATTACKS[square][color.to_index()]
                );
                assert_eq!(
                    gold_attacks_unchecked(square, color),
                    GOLD_ATTACKS[square][color.to_index()]
                );
                assert_eq!(king_attacks_unchecked(square), KING_ATTACKS[square]);
            }
        }
        // 横方向の各遮蔽配置を盤上の歩行で検査し、旧 API の境界を維持する。
        for mask in 0_u16..512 {
            let mut occupied = Bitboard::EMPTY;
            for file in 0..9 {
                if mask & (1 << file) != 0 {
                    occupied |= Bitboard::from_square(Square::from_file_rank(
                        File::new(file),
                        square.rank(),
                    ));
                }
            }
            let mut expected = Bitboard::EMPTY;
            for direction in [-1_i8, 1] {
                let mut file = square.file().raw() + direction;
                while (0..9).contains(&file) {
                    let target = Square::from_file_rank(File::new(file), square.rank());
                    expected |= Bitboard::from_square(target);
                    if occupied.test(target) {
                        break;
                    }
                    file += direction;
                }
            }
            assert_eq!(rook_rank_attacks(square, occupied), expected);
        }
    }
}
