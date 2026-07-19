use crate::board::{
    BoardArray, PositionState, generate_position_state, position_from_position_state,
};
use crate::types::{Color, File, Hand, HandPiece, Piece, Rank, Square};

#[test]
// 基本局面のSFEN往復が成立しZobristも一致するかを確認
fn test_sfen_roundtrip_basic_positions() {
    let cases = [
        ("startpos", "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"),
        ("after_76fu", "lnsgkgsnl/1r5b1/ppppppppp/9/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL w - 2"),
        (
            "bench_opening",
            "lnsgkgsnl/1r7/p1ppp1bpp/1p3pp2/7P1/2P6/PP1PPPP1P/1B3S1R1/LNSGKG1NL b - 9",
        ),
        (
            "bench_midgame1",
            "l4S2l/4g1gs1/5p1p1/pr2N1pkp/4Gn3/PP3PPPP/2GPP4/1K7/L3r+s2L w BS2N5Pb 1",
        ),
    ];

    for (name, sfen) in cases {
        let pos = crate::board::position_from_sfen(sfen)
            .unwrap_or_else(|e| panic!("Failed to parse SFEN for {name}: {e:?}\nSFEN: {sfen}"));
        let generated_sfen = pos.to_sfen(None);
        assert_eq!(generated_sfen, sfen, "SFEN roundtrip failed for {name}");

        let pos2 = crate::board::position_from_sfen(&generated_sfen)
            .unwrap_or_else(|e| panic!("Failed to parse generated SFEN for {name}: {e:?}"));

        assert_eq!(pos2.to_sfen(None), sfen, "Second roundtrip failed for {name}");
        assert_eq!(pos.key(), pos2.key(), "Zobrist hash mismatch for {name}");
    }
}

#[test]
// 持ち駒付きSFENの往復が正しく行えるか確認
fn test_sfen_roundtrip_with_pieces_in_hand() {
    let test_cases = [
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b P 1",
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b RBG2S2N2L2P 1",
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w rbg2s2n2l2p 1",
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b RBrbp 1",
    ];

    for sfen in test_cases {
        let pos = crate::board::position_from_sfen(sfen).expect("Valid SFEN should parse");
        let generated = pos.to_sfen(None);
        let pos2 =
            crate::board::position_from_sfen(&generated).expect("Generated SFEN should parse");
        assert_eq!(pos2.to_sfen(None), generated, "Roundtrip with hand pieces failed");
    }
}

#[test]
// 手数フィールドが往復で保持されるかを確認
fn test_sfen_roundtrip_preserves_ply() {
    let ply_cases = [
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 2",
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 100",
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 999",
    ];

    for sfen in ply_cases {
        let pos = crate::board::position_from_sfen(sfen).expect("Valid SFEN should parse");
        assert_eq!(pos.to_sfen(None), sfen, "SFEN should roundtrip with ply");
    }
}

// --- PositionState 往復テスト ---

#[test]
fn test_position_state_roundtrip_basic() {
    let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
    let pos = crate::board::position_from_sfen(sfen).unwrap();

    // Position → PositionState → Position の round-trip
    let state = generate_position_state(&pos);
    let pos2 = position_from_position_state(&state);

    assert_eq!(pos.to_sfen(None), pos2.to_sfen(None));
    assert_eq!(pos.key(), pos2.key());
    assert_eq!(generate_position_state(&pos2), state);
}

#[test]
fn test_position_state_roundtrip_with_hand() {
    let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b RBG2S2N2L2P 1";
    let pos = crate::board::position_from_sfen(sfen).unwrap();

    let state = generate_position_state(&pos);
    let pos2 = position_from_position_state(&state);

    assert_eq!(pos.to_sfen(None), pos2.to_sfen(None));
    assert_eq!(pos.key(), pos2.key());
    assert_eq!(generate_position_state(&pos2), state);
}

#[test]
fn test_position_state_roundtrip_preserves_ply() {
    let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 42";
    let pos = crate::board::position_from_sfen(sfen).unwrap();

    let state = generate_position_state(&pos);
    assert_eq!(state.ply, 42);

    let pos2 = position_from_position_state(&state);
    assert_eq!(pos2.game_ply(), 42);
    assert_eq!(generate_position_state(&pos2), state);
}

#[test]
fn test_position_state_edit_and_rebuild() {
    let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
    let mut state = crate::board::parse_sfen(sfen).unwrap();

    // 手番を後手に変更
    state.side_to_move = crate::types::Color::WHITE;
    state.ply = 2;

    let pos = position_from_position_state(&state);
    assert_eq!(pos.turn(), crate::types::Color::WHITE);
    assert_eq!(pos.game_ply(), 2);
    assert_eq!(generate_position_state(&pos), state);
}

#[test]
fn test_generate_sfen_from_position_state() {
    let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
    let state = crate::board::parse_sfen(sfen).unwrap();

    let generated = crate::board::generate_sfen_from_position_state(&state, true);
    assert_eq!(generated, sfen);
}

#[test]
fn test_generate_sfen_from_position_state_without_ply() {
    let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b -";
    let state =
        crate::board::parse_sfen("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1")
            .unwrap();

    let generated = crate::board::generate_sfen_from_position_state(&state, false);
    assert_eq!(generated, sfen);
}

#[test]
fn test_raw_state_roundtrip_without_string_serialization() {
    let mut board = BoardArray::empty();
    board.set(Square::from_file_rank(File::FILE_5, Rank::RANK_1), Piece::B_KING);
    board.set(Square::from_file_rank(File::FILE_5, Rank::RANK_9), Piece::W_KING);
    board.set(Square::from_file_rank(File::FILE_7, Rank::RANK_7), Piece::B_PAWN);
    board.set(Square::from_file_rank(File::FILE_3, Rank::RANK_3), Piece::W_BISHOP);

    let mut black_hand = Hand::ZERO;
    black_hand.add(HandPiece::ROOK, 1);
    black_hand.add(HandPiece::GOLD, 2);

    let mut white_hand = Hand::ZERO;
    white_hand.add(HandPiece::SILVER, 1);

    let state = PositionState {
        board,
        hands: [black_hand, white_hand],
        side_to_move: Color::WHITE,
        ply: 77,
    };

    let pos = position_from_position_state(&state);
    let exported = generate_position_state(&pos);

    assert_eq!(exported, state);
    assert_eq!(crate::board::generate_sfen_from_position_state(&state, true), pos.to_sfen(None));
    #[cfg(feature = "validation")]
    assert!(pos.validate().is_ok());
}

#[cfg(feature = "validation")]
#[test]
fn test_raw_state_import_is_unchecked_and_can_be_validated_afterward() {
    let mut board = BoardArray::empty();
    board.set(Square::from_file_rank(File::FILE_5, Rank::RANK_1), Piece::B_KING);
    board.set(Square::from_file_rank(File::FILE_5, Rank::RANK_9), Piece::W_KING);
    board.set(Square::from_file_rank(File::FILE_1, Rank::RANK_3), Piece::B_PAWN);
    board.set(Square::from_file_rank(File::FILE_1, Rank::RANK_4), Piece::B_PAWN);

    let state = PositionState {
        board,
        hands: [Hand::ZERO; Color::COUNT],
        side_to_move: Color::BLACK,
        ply: 1,
    };

    let pos = position_from_position_state(&state);

    assert_eq!(generate_position_state(&pos), state);
    assert_eq!(
        pos.validate(),
        Err(crate::board::position::ValidationError::DoublePawn(File::FILE_1, Color::BLACK,)),
    );

    let report = pos.validate_all();
    assert!(!report.is_valid());
    assert!(report.issues().iter().any(|issue| matches!(
        issue,
        crate::board::ValidationIssue::DoublePawn(File::FILE_1, Color::BLACK)
    )));
}
