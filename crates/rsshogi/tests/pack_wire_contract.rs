#![cfg(feature = "records")]

use rsshogi::records::formats::pack::{
    PackEndReason, PackGame, PackGameResult, PackPly, PackStartPosition, decode_game, encode_game,
    game_from_record, record_from_game,
};
use rsshogi::types::AperyMove;

#[test]
fn empty_startpos_game_has_the_declared_exact_bytes() {
    let game = PackGame {
        start_position: PackStartPosition::Startpos,
        plies: Vec::new(),
        result: PackGameResult::BlackWin,
        end_reason: PackEndReason::Resign,
    };

    let expected = [0x01, 0x81, 0x00, 0x00];
    assert_eq!(encode_game(&game), expected);

    let (decoded, consumed) = decode_game(&expected).expect("declared PACK vector must decode");
    assert_eq!(decoded, game);
    assert_eq!(consumed, expected.len());
}

#[test]
fn games_are_self_delimiting_and_concatenable() {
    let first = PackGame {
        start_position: PackStartPosition::Startpos,
        plies: Vec::new(),
        result: PackGameResult::Draw,
        end_reason: PackEndReason::RepetitionDraw,
    };
    let second = PackGame {
        start_position: PackStartPosition::Startpos,
        plies: Vec::new(),
        result: PackGameResult::WhiteWin,
        end_reason: PackEndReason::TimeUp,
    };

    let mut bytes = encode_game(&first);
    bytes.extend_from_slice(&encode_game(&second));

    let (decoded_first, consumed) = decode_game(&bytes).expect("first game must decode");
    let (decoded_second, second_consumed) =
        decode_game(&bytes[consumed..]).expect("second game must decode");
    assert_eq!(decoded_first, first);
    assert_eq!(decoded_second, second);
    assert_eq!(consumed + second_consumed, bytes.len());
}

#[test]
fn malformed_terminal_marker_is_rejected() {
    let invalid = [0x01, 0x03, 0x00, 0x00];
    assert!(decode_game(&invalid).is_err());
}

#[test]
fn promotion_and_drop_game_matches_apery_wire_bytes_and_record_roundtrip() {
    let game = PackGame {
        start_position: PackStartPosition::Startpos,
        plies: vec![
            PackPly { mv: AperyMove::from_raw(0x1e3b), eval: 10 },
            PackPly { mv: AperyMove::from_raw(0x0a15), eval: -20 },
            PackPly { mv: AperyMove::from_raw(0x630a), eval: 300 },
            PackPly { mv: AperyMove::from_raw(0x090a), eval: 0 },
            PackPly { mv: AperyMove::from_raw(0x2a9f), eval: 32_000 },
        ],
        result: PackGameResult::BlackWin,
        end_reason: PackEndReason::Resign,
    };
    let expected = [
        0x01, 0x3b, 0x1e, 0x0a, 0x00, 0x15, 0x0a, 0xec, 0xff, 0x0a, 0x63, 0x2c, 0x01, 0x0a, 0x09,
        0x00, 0x00, 0x9f, 0x2a, 0x00, 0x7d, 0x81, 0x00, 0x00,
    ];

    assert_eq!(encode_game(&game), expected);
    let (decoded, consumed) = decode_game(&expected).expect("golden PACK game must decode");
    assert_eq!(decoded, game);
    assert_eq!(consumed, expected.len());

    let record = record_from_game(&decoded).expect("PACK game must convert to a record");
    assert_eq!(game_from_record(&record).expect("record must convert back to PACK"), game);
}

#[test]
fn record_to_pack_clamps_special_evaluations_to_centipawn_range() {
    for (input, expected) in [(i16::MIN, -32_000), (i16::MAX, 32_000)] {
        let game = PackGame {
            start_position: PackStartPosition::Startpos,
            plies: vec![PackPly { mv: AperyMove::from_raw(0x1e3b), eval: input }],
            result: PackGameResult::BlackWin,
            end_reason: PackEndReason::Resign,
        };
        let record = record_from_game(&game).expect("PACK game must convert to a record");
        let rebuilt = game_from_record(&record).expect("record must convert back to PACK");
        assert_eq!(rebuilt.plies[0].eval, expected);
    }
}
