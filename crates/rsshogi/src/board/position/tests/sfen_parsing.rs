use super::*;

#[test]
fn test_sfen_rejects_extra_ranks_and_dangling_hand_counts_without_mutation() {
    let mut pos = crate::board::hirate_position();
    let mv = pos.move32_from_move(crate::types::Move::from_usi("7g7f").unwrap());
    pos.apply_move32(mv);
    let before = pos.to_sfen(None);
    let key = pos.key();
    let invalid = [
        format!("{}{}", "9/".repeat(256), crate::board::STARTPOS_SFEN),
        format!("{} b - 1", vec!["9"; 129].join("/")),
        "9/9/9/9/9/9/9/9/9/9 b - 1".to_owned(),
        "9/9/9/9/9/9/9/9/9 b P2 1".to_owned(),
        "9/9/9/9/9/9/9/9/9 b 2 1".to_owned(),
        "9/9/9/9/9/9/9/9/9 b 9999999999999999999999 1".to_owned(),
    ];
    for sfen in invalid {
        assert!(pos.set_sfen(&sfen).is_err(), "accepted malformed SFEN: {sfen}");
        assert_eq!(pos.to_sfen(None), before);
        assert_eq!(pos.key(), key);
        assert_eq!(pos.last_move(), mv);
        assert_eq!(pos.state_stack_depth(), 1);
    }
    pos.undo_move32(mv).unwrap();
    assert_eq!(pos.to_sfen(None), crate::board::STARTPOS_SFEN);
}

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
