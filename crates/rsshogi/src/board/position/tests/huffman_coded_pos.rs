use super::*;

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
