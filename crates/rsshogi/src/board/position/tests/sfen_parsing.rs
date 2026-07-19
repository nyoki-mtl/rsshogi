use super::*;

#[test]
// 無効SFENの各種パターンでエラーが返るか検証
fn test_sfen_parse_error_cases() {
    let invalid_sfens = vec![
        ("", "Empty SFEN should fail"),
        ("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL", "Missing turn field"),
        ("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b", "Missing hand field"),
        (
            "Xnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
            "Invalid piece character",
        ),
        ("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL x - 1", "Invalid turn"),
        ("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - abc", "Invalid ply"),
    ];

    for (invalid_sfen, error_msg) in invalid_sfens {
        let mut pos = Position::empty();
        let result = pos.set_sfen(invalid_sfen);
        assert!(result.is_err(), "{error_msg}: SFEN='{invalid_sfen}' should fail to parse");
    }
}

#[test]
fn test_sfen_parses_hand_counts_in_matsuri_position() {
    use crate::types::{Color, HandPiece};

    let sfen = "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2Sp1/1PPb2P1P/P5GS1/R8/LN4bKL w GR5pnsg 1";
    let mut pos = Position::empty();
    pos.set_sfen(sfen).expect("parse sfen");

    let hand = pos.hand(Color::WHITE);
    assert_eq!(hand.count(HandPiece::PAWN), 5, "white should have 5 pawns");
    assert_eq!(hand.count(HandPiece::KNIGHT), 1, "white should have 1 knight");
    assert_eq!(hand.count(HandPiece::SILVER), 1, "white should have 1 silver");
    assert_eq!(hand.count(HandPiece::GOLD), 1, "white should have 1 gold");
}

#[test]
fn test_sfen_allows_missing_ply_and_trailing_tokens() {
    let mut pos = Position::empty();
    pos.set_sfen("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b -")
        .expect("missing ply should be accepted");
    assert_eq!(pos.game_ply(), 0);

    let mut pos = Position::empty();
    pos.set_sfen("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1 extra")
        .expect("trailing tokens should be ignored");
    assert_eq!(pos.game_ply(), 1);
}
