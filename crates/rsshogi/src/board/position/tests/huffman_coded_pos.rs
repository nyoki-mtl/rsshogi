use super::*;
use crate::types::PieceType;

const KINGS_ONLY_HCP: [u8; 32] = [
    0x58, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x24, 0x49, 0x92, 0x24,
    0x49, 0x92, 0x94, 0x52, 0x4a, 0x6b, 0xad, 0xd5, 0x5a, 0x6b, 0xbd, 0xf7, 0xfe, 0x78, 0xbc, 0x5e,
];

#[test]
fn test_huffman_coded_pos_matches_independent_kings_only_vector() {
    // LSB-first encoding of side 0, kings on raw squares 44 and 36, 79 empty
    // squares, then the standard piece-box counts in the documented wire order.
    let sfen = "4k4/9/9/9/9/9/9/9/4K4 b - 1";
    let pos = crate::board::position_from_sfen(sfen).expect("sfen should parse");
    assert_eq!(pos.to_huffman_coded_pos().data, KINGS_ONLY_HCP);

    let mut decoded = Position::empty();
    decoded
        .set_huffman_coded_pos(&crate::board::HuffmanCodedPos { data: KINGS_ONLY_HCP }, 1)
        .expect("golden HCP should decode");
    assert_eq!(decoded.to_sfen(None), sfen);
}

#[test]
fn test_huffman_coded_pos_roundtrip_matches_to_sfen() {
    let cases = [
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        "l3k2nl/1r1sg1gp1/2np1s2p/p1p1pppR1/1p7/P1PPP1P1P/1PS2P3/2G1GS3/LN1K3NL w BPb 15",
        "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2Sp1/1PPb2P1P/P5GS1/R8/LN4bKL w GR5pnsg 1",
    ];

    for sfen in cases {
        let pos = crate::board::position_from_sfen(sfen).expect("sfen should parse");
        let packed = pos.to_huffman_coded_pos();

        let mut roundtrip = Position::empty();
        roundtrip.set_huffman_coded_pos(&packed, pos.game_ply()).expect("hcp should decode");

        assert_eq!(roundtrip.to_sfen(None), pos.to_sfen(None), "HCP roundtrip mismatch");
    }
}

#[test]
fn test_huffman_coded_pos_unpack_matches_to_sfen() {
    let pos = crate::board::position_from_sfen(
        "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2Sp1/1PPb2P1P/P5GS1/R8/LN4bKL w GR5pnsg 1",
    )
    .expect("sfen should parse");
    let packed = pos.to_huffman_coded_pos();
    let unpacked = Position::huffman_coded_pos_unpack(&packed).expect("hcp should unpack");

    let mut expected = pos;
    expected.ply = 0;
    assert_eq!(unpacked, expected.to_sfen(None));
}

#[test]
fn test_huffman_coded_pos_rejects_invalid_code() {
    let packed = crate::board::HuffmanCodedPos { data: [0xff; 32] };
    let mut pos = Position::empty();
    let result = pos.set_huffman_coded_pos(&packed, 1);
    assert!(result.is_err());
}

#[test]
fn test_huffman_coded_pos_rejects_invalid_king_square() {
    let mut packed = crate::board::hirate_position().to_huffman_coded_pos();
    // 手番 1bit + 先手玉の升 7bit
    packed.data[0] = (packed.data[0] & !0b1111_1110u8) | ((81u8) << 1);

    let mut pos = Position::empty();
    let result = pos.set_huffman_coded_pos(&packed, 1);
    assert!(matches!(result, Err(crate::board::HuffmanCodedPosError::InvalidKingSquare(81))));
}

#[test]
fn test_huffman_coded_pos_encoder_rejects_excess_inventory() {
    let cases = [
        (
            "4k4/9/9/9/9/9/9/GGGGG4/4K4 b - 1",
            crate::board::HuffmanCodedPosError::InvalidInventory {
                piece: PieceType::GOLD,
                count: 5,
                limit: 4,
            },
        ),
        (
            "kRRRRRRRR/RRRRRRRRR/RRRRRRRRR/RRRRRRRRR/RRRRRRRR1/9/9/9/K8 b - 1",
            crate::board::HuffmanCodedPosError::InvalidInventory {
                piece: PieceType::ROOK,
                count: 3,
                limit: 2,
            },
        ),
    ];

    for (sfen, expected) in cases {
        let pos = crate::board::position_from_sfen(sfen).expect("structurally parseable position");
        assert_eq!(pos.try_to_huffman_coded_pos(), Err(expected));
    }
}
