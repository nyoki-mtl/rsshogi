use super::*;
use crate::board::position::{
    bit_io::BitReader,
    packed_sfen::{read_board_piece, read_board_piece_fast, read_hand_piece, read_hand_piece_fast},
};
use crate::types::HandPiece;

struct TestPackedSfenSink {
    board: BoardArray,
    hands: [Hand; Color::COUNT],
    side_to_move: Color,
}

impl Default for TestPackedSfenSink {
    fn default() -> Self {
        Self {
            board: BoardArray::empty(),
            hands: [Hand::ZERO; Color::COUNT],
            side_to_move: Color::BLACK,
        }
    }
}

impl PackedSfenSink for TestPackedSfenSink {
    fn reset(&mut self, side_to_move: Color) {
        self.board = BoardArray::empty();
        self.hands = [Hand::ZERO; Color::COUNT];
        self.side_to_move = side_to_move;
    }

    fn set_piece(&mut self, square: Square, piece: Piece) {
        self.board.set(square, piece);
    }

    fn add_hand_piece(&mut self, color: Color, piece: HandPiece) {
        self.hands[color.to_index()].add(piece, 1);
    }
}

#[test]
fn test_fast_huffman_matches_reference_for_all_prefixes_and_alignments() {
    for offset in 0u8..8 {
        for prefix in 0u64..=u64::from(u8::MAX) {
            let words = [prefix << offset, 0, 0, 0];

            let mut fast_board = BitReader::new(&words);
            fast_board.advance(offset).expect("offset is in range");
            let mut reference_board = BitReader::new(&words);
            reference_board.advance(offset).expect("offset is in range");
            assert_eq!(
                read_board_piece_fast(&mut fast_board),
                read_board_piece(&mut reference_board)
            );
            assert_eq!(fast_board.cursor(), reference_board.cursor());

            let mut fast_hand = BitReader::new(&words);
            fast_hand.advance(offset).expect("offset is in range");
            let mut reference_hand = BitReader::new(&words);
            reference_hand.advance(offset).expect("offset is in range");
            assert_eq!(read_hand_piece_fast(&mut fast_hand), read_hand_piece(&mut reference_hand));
            assert_eq!(fast_hand.cursor(), reference_hand.cursor());
        }
    }
}

#[test]
fn test_decode_into_matches_position_state_with_and_without_mirror() {
    let cases = [
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        "l3k2nl/1r1sg1gp1/2np1s2p/p1p1pppR1/1p7/P1PPP1P1P/1PS2P3/2G1GS3/LN1K3NL w BPb 15",
        "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2Sp1/1PPb2P1P/P5GS1/R8/LN4bKL w GR5pnsg 1",
    ];

    for sfen in cases {
        let position = Position::from_sfen(sfen).expect("sfen should parse");
        let packed = position.to_packed_sfen();

        let mut sink = TestPackedSfenSink::default();
        packed.decode_into(false, &mut sink).expect("packed sfen should decode");
        assert_eq!(sink.board, position.board);
        assert_eq!(sink.hands, position.hands);
        assert_eq!(sink.side_to_move, position.turn());

        packed.decode_into(true, &mut sink).expect("mirrored packed sfen should decode");
        assert_eq!(sink.board, mirror_board(&position.board));
        assert_eq!(sink.hands, position.hands);
        assert_eq!(sink.side_to_move, position.turn());
    }
}

#[test]
fn test_packed_sfen_rejects_invalid_king_square() {
    let mut data = [0u8; 32];
    let value = 81u16 << 1;
    data[0] = (value & 0xff) as u8;
    data[1] = (value >> 8) as u8;
    let packed = PackedSfen::from_bytes(data);

    let mut sink = TestPackedSfenSink::default();
    assert_eq!(
        packed.decode_into(false, &mut sink),
        Err(PackedSfenError::InvalidKingSquare { color: Color::BLACK, square: 81 })
    );
    assert_eq!(
        Position::sfen_unpack(&packed),
        Err(PackedSfenError::InvalidKingSquare { color: Color::BLACK, square: 81 })
    );
}

fn mirror_board(board: &BoardArray) -> BoardArray {
    let mut mirrored = BoardArray::empty();
    for (sq, packed) in board.iter() {
        if packed.is_empty() {
            continue;
        }
        mirrored.set(sq.mirror_file(), packed);
    }
    mirrored
}

fn position_from_parts(
    board: BoardArray,
    hands: [Hand; Color::COUNT],
    side_to_move: Color,
    ply: Ply,
) -> Position {
    let mut pos = Position::empty();
    pos.board = board;
    pos.hands = hands;
    pos.set_side_to_move(side_to_move);
    pos.ply = ply;
    pos.rebuild_bitboards();

    let keys = pos.compute_keys();
    pos.board_key = keys.board_key;
    pos.hand_key = keys.hand_key;
    pos.zobrist = pos.board_key ^ pos.hand_key;
    pos
}

#[test]
fn test_packed_sfen_public_accessors_roundtrip_words_and_bytes() {
    let words = [
        0x2491_9320,
        0x8a34_ab0c,
        0x3897_5c85,
        0xef3c_7020,
        0xafa4_0899,
        0x2400_50f0,
        0x7c31_1201,
        0xbce1_f3e3,
    ];
    let packed = PackedSfen::from_le_u32_words(words);

    assert_eq!(packed.to_le_u32_words(), words);
    assert_eq!(PackedSfen::from_bytes(*packed.as_bytes()), packed);
    assert_eq!(
        packed.as_bytes(),
        &[
            0x20, 0x93, 0x91, 0x24, 0x0c, 0xab, 0x34, 0x8a, 0x85, 0x5c, 0x97, 0x38, 0x20, 0x70,
            0x3c, 0xef, 0x99, 0x08, 0xa4, 0xaf, 0xf0, 0x50, 0x00, 0x24, 0x01, 0x12, 0x31, 0x7c,
            0xe3, 0xf3, 0xe1, 0xbc,
        ]
    );
}

#[test]
fn test_packed_sfen_sbk_word_order_is_not_byte_order() {
    let word_greater = PackedSfen::from_le_u32_words([0x0000_0100, 0, 0, 0, 0, 0, 0, 0]);
    let word_less = PackedSfen::from_le_u32_words([0x0000_00ff, 0, 0, 0, 0, 0, 0, 0]);

    assert!(word_greater.as_bytes() < word_less.as_bytes());
    assert_eq!(word_greater.cmp_sbk_words(&word_less), std::cmp::Ordering::Greater);
}

#[test]
fn test_position_from_sfen_associated_function_matches_board_helper() {
    let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
    let from_assoc = Position::from_sfen(sfen).expect("sfen should parse");
    let from_helper = crate::board::position_from_sfen(sfen).expect("sfen should parse");

    assert_eq!(from_assoc.to_sfen(None), from_helper.to_sfen(None));
}

#[test]
// PackedSfenの往復でSFENが一致するか確認
fn test_packed_sfen_roundtrip_matches_to_sfen() {
    let cases = [
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        "l3k2nl/1r1sg1gp1/2np1s2p/p1p1pppR1/1p7/P1PPP1P1P/1PS2P3/2G1GS3/LN1K3NL w BPb 15",
        "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2Sp1/1PPb2P1P/P5GS1/R8/LN4bKL w GR5pnsg 1",
    ];

    for sfen in cases {
        let pos = crate::board::position_from_sfen(sfen).expect("sfen should parse");
        let packed = pos.to_packed_sfen();

        let mut roundtrip = Position::empty();
        roundtrip
            .set_packed_sfen(&packed, false, pos.game_ply())
            .expect("packed sfen should decode");

        assert_eq!(roundtrip.to_sfen(None), pos.to_sfen(None), "PackedSfen roundtrip mismatch");
    }
}

#[test]
// PackedSfenのアンパックがSFEN生成と一致するか確認
fn test_packed_sfen_unpack_matches_to_sfen() {
    let cases = [
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        "l3k2nl/1r1sg1gp1/2np1s2p/p1p1pppR1/1p7/P1PPP1P1P/1PS2P3/2G1GS3/LN1K3NL w BPb 15",
        "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2Sp1/1PPb2P1P/P5GS1/R8/LN4bKL w GR5pnsg 1",
    ];

    for sfen in cases {
        let pos = crate::board::position_from_sfen(sfen).expect("sfen should parse");
        let packed = pos.to_packed_sfen();
        let unpacked = Position::sfen_unpack(&packed).expect("packed sfen should unpack");
        let expected = position_from_parts(pos.board, pos.hands, pos.turn(), 0);
        assert_eq!(unpacked, expected.to_sfen(None), "PackedSfen unpack mismatch");
    }
}

#[test]
// PackedSfenの左右反転が期待通りか確認
fn test_packed_sfen_mirror_matches_expected() {
    let sfen = "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2Sp1/1PPb2P1P/P5GS1/R8/LN4bKL w GR5pnsg 1";
    let pos = crate::board::position_from_sfen(sfen).expect("sfen should parse");
    let packed = pos.to_packed_sfen();

    let mut mirrored = Position::empty();
    mirrored
        .set_packed_sfen(&packed, true, pos.game_ply())
        .expect("packed sfen should decode with mirror");

    let expected_board = mirror_board(&pos.board);
    let expected = position_from_parts(expected_board, pos.hands, pos.turn(), pos.game_ply());
    assert_eq!(mirrored.to_sfen(None), expected.to_sfen(None), "mirror result mismatch");
}

#[test]
// PSVのサンプルデータに対するPackedSfenの互換性を検証
fn test_packed_sfen_psv_samples_match_cshogi() {
    let samples = [
        PsvSample {
            sfen: "2+P1g2nl/3ps1k2/pp2p2p1/3+B1Pp1p/3P2nl1/1P4N2/P2GP1PPP/L4SSK1/+b4G1NL b 2RPgsp 1",
            packed: [
                0x20, 0x93, 0x91, 0x24, 0x0c, 0xab, 0x34, 0x8a, 0x85, 0x5c, 0x97, 0x38, 0x20, 0x70,
                0x3c, 0xef, 0x99, 0x08, 0xa4, 0xaf, 0xf0, 0x50, 0x00, 0x24, 0x01, 0x12, 0x31, 0x7c,
                0xe3, 0xf3, 0xe1, 0xbc,
            ],
            game_ply: 87,
        },
        PsvSample {
            sfen: "ln1g1gsn1/1r1s1k1bl/p2p1p1pp/2p1p1p2/1p6P/2PP5/PPB1PPPP1/2R2KS2/LNSG1G1NL b - 1",
            packed: [
                0x44, 0x1c, 0x63, 0x0a, 0x0c, 0xeb, 0x67, 0x22, 0x2c, 0x27, 0x49, 0x1c, 0xde, 0x44,
                0x3c, 0x48, 0x82, 0xf7, 0x4c, 0x82, 0x07, 0x29, 0x3e, 0x7e, 0x8e, 0xf5, 0x97, 0x14,
                0x96, 0x51, 0x22, 0x0c,
            ],
            game_ply: 17,
        },
        PsvSample {
            sfen: "l8/5+RSbk/p6gp/4pppp1/3P3g1/4SPP1l/P3P1G1b/3+r2SP1/LN3GK2 b SN2P2nl4p 1",
            packed: [
                0x34, 0x01, 0xc9, 0xf8, 0x89, 0xcf, 0x37, 0x5f, 0x82, 0x43, 0x8a, 0xe7, 0xf0, 0x27,
                0x05, 0x0f, 0xd2, 0x11, 0x40, 0xf0, 0x0f, 0x00, 0xc0, 0x32, 0x4a, 0x84, 0x01, 0x28,
                0x83, 0x24, 0x63, 0xad,
            ],
            game_ply: 96,
        },
        PsvSample {
            sfen: "lnkg2g1l/2s1Ps+R2/1p1ppg1pp/p1p2n3/7P1/PPPP3BP/1K2+bPp2/2S2s3/LNG4+RL b N2P 1",
            packed: [
                0x8a, 0xb6, 0x51, 0x12, 0x0c, 0xa4, 0xf8, 0xe0, 0xef, 0xfd, 0x83, 0xc4, 0xf9, 0xae,
                0xc4, 0x49, 0x24, 0xbe, 0x79, 0x25, 0x81, 0x53, 0x0a, 0xc7, 0xb3, 0x4a, 0xc2, 0x32,
                0x92, 0x82, 0x01, 0x28,
            ],
            game_ply: 111,
        },
        PsvSample {
            sfen: "l7l/2gsksg2/ppnpppn2/7rp/2p1S4/1R4P2/P1NPPP1PP/2G1KSG2/L6NL b B2Pb2p 1",
            packed: [
                0x56, 0xa5, 0x91, 0x24, 0x0c, 0xf8, 0x25, 0x2c, 0xde, 0x95, 0xf0, 0x70, 0x26, 0xe2,
                0x20, 0x1d, 0x82, 0x33, 0x11, 0x78, 0x57, 0x69, 0x79, 0x90, 0xfc, 0x60, 0x94, 0x08,
                0x03, 0xf0, 0x20, 0x9f,
            ],
            game_ply: 45,
        },
    ];

    for (idx, sample) in samples.iter().enumerate() {
        let packed_sfen = PackedSfen { data: sample.packed };
        let unpacked = Position::sfen_unpack(&packed_sfen).expect("packed sfen should unpack");
        let pos = crate::board::position_from_sfen(sample.sfen).expect("sample sfen should parse");
        let expected_unpacked = position_from_parts(pos.board, pos.hands, pos.turn(), 0);
        assert_eq!(unpacked, expected_unpacked.to_sfen(None), "PSV unpack mismatch at index {idx}");
        let pos = crate::board::position_from_sfen(sample.sfen).expect("sample sfen should parse");
        let expected = position_from_parts(pos.board, pos.hands, pos.turn(), sample.game_ply);
        let repacked = pos.to_packed_sfen();
        assert_eq!(repacked.data, sample.packed, "PSV repack mismatch at index {idx}");

        let mut roundtrip = Position::empty();
        roundtrip
            .set_packed_sfen(&packed_sfen, false, sample.game_ply)
            .expect("packed sfen should decode");
        assert_eq!(
            roundtrip.to_sfen(None),
            expected.to_sfen(None),
            "PSV set_packed_sfen mismatch at index {idx}"
        );
    }
}

struct PsvSample {
    sfen: &'static str,
    packed: [u8; 32],
    game_ply: u16,
}
