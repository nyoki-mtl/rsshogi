use super::bit_io::{BitCursorOverflow, BitReader, BitWriter, bytes_to_words, words_to_bytes};
use super::{Ply, Position};
use crate::board::parser::generate_sfen;
use crate::board::position::BoardArray;
use crate::types::{Color, EnteringKingRule, Hand, HandPiece, Piece, PieceType, Square};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HuffmanCodedPos {
    pub data: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HuffmanCodedPosError {
    InvalidCursor,
    InvalidPieceCode,
    InvalidKingSquare(u16),
}

impl fmt::Display for HuffmanCodedPosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCursor => write!(f, "invalid huffman cursor"),
            Self::InvalidPieceCode => write!(f, "invalid huffman piece code"),
            Self::InvalidKingSquare(sq) => {
                write!(f, "invalid king square in HuffmanCodedPos: {sq}")
            }
        }
    }
}

impl std::error::Error for HuffmanCodedPosError {}

impl From<BitCursorOverflow> for HuffmanCodedPosError {
    fn from(_: BitCursorOverflow) -> Self {
        Self::InvalidCursor
    }
}

#[derive(Clone, Copy)]
struct HuffmanCode {
    code: u8,
    bits: u8,
}

#[derive(Clone, Copy)]
struct PieceCode {
    piece: Piece,
    code: HuffmanCode,
}

#[derive(Clone, Copy)]
struct HandCode {
    piece: Option<Piece>,
    code: HuffmanCode,
}

const BOARD_CODES: [PieceCode; 29] = [
    PieceCode { piece: Piece::NONE, code: HuffmanCode { code: 0x00, bits: 1 } },
    PieceCode { piece: Piece::B_PAWN, code: HuffmanCode { code: 0x01, bits: 4 } },
    PieceCode { piece: Piece::B_LANCE, code: HuffmanCode { code: 0x03, bits: 6 } },
    PieceCode { piece: Piece::B_KNIGHT, code: HuffmanCode { code: 0x07, bits: 6 } },
    PieceCode { piece: Piece::B_SILVER, code: HuffmanCode { code: 0x0b, bits: 6 } },
    PieceCode { piece: Piece::B_BISHOP, code: HuffmanCode { code: 0x1f, bits: 8 } },
    PieceCode { piece: Piece::B_ROOK, code: HuffmanCode { code: 0x3f, bits: 8 } },
    PieceCode { piece: Piece::B_GOLD, code: HuffmanCode { code: 0x0f, bits: 6 } },
    PieceCode { piece: Piece::B_PRO_PAWN, code: HuffmanCode { code: 0x09, bits: 4 } },
    PieceCode { piece: Piece::B_PRO_LANCE, code: HuffmanCode { code: 0x23, bits: 6 } },
    PieceCode { piece: Piece::B_PRO_KNIGHT, code: HuffmanCode { code: 0x27, bits: 6 } },
    PieceCode { piece: Piece::B_PRO_SILVER, code: HuffmanCode { code: 0x2b, bits: 6 } },
    PieceCode { piece: Piece::B_HORSE, code: HuffmanCode { code: 0x9f, bits: 8 } },
    PieceCode { piece: Piece::B_DRAGON, code: HuffmanCode { code: 0xbf, bits: 8 } },
    PieceCode { piece: Piece::W_PAWN, code: HuffmanCode { code: 0x05, bits: 4 } },
    PieceCode { piece: Piece::W_LANCE, code: HuffmanCode { code: 0x13, bits: 6 } },
    PieceCode { piece: Piece::W_KNIGHT, code: HuffmanCode { code: 0x17, bits: 6 } },
    PieceCode { piece: Piece::W_SILVER, code: HuffmanCode { code: 0x1b, bits: 6 } },
    PieceCode { piece: Piece::W_BISHOP, code: HuffmanCode { code: 0x5f, bits: 8 } },
    PieceCode { piece: Piece::W_ROOK, code: HuffmanCode { code: 0x7f, bits: 8 } },
    PieceCode { piece: Piece::W_GOLD, code: HuffmanCode { code: 0x2f, bits: 6 } },
    PieceCode { piece: Piece::W_PRO_PAWN, code: HuffmanCode { code: 0x0d, bits: 4 } },
    PieceCode { piece: Piece::W_PRO_LANCE, code: HuffmanCode { code: 0x33, bits: 6 } },
    PieceCode { piece: Piece::W_PRO_KNIGHT, code: HuffmanCode { code: 0x37, bits: 6 } },
    PieceCode { piece: Piece::W_PRO_SILVER, code: HuffmanCode { code: 0x3b, bits: 6 } },
    PieceCode { piece: Piece::W_HORSE, code: HuffmanCode { code: 0xdf, bits: 8 } },
    PieceCode { piece: Piece::W_DRAGON, code: HuffmanCode { code: 0xff, bits: 8 } },
    PieceCode { piece: Piece::B_KING, code: HuffmanCode { code: 0x00, bits: 0 } },
    PieceCode { piece: Piece::W_KING, code: HuffmanCode { code: 0x00, bits: 0 } },
];

const HAND_CODES: [HandCode; 21] = [
    HandCode { piece: Some(Piece::B_PAWN), code: HuffmanCode { code: 0x00, bits: 3 } },
    HandCode { piece: Some(Piece::W_PAWN), code: HuffmanCode { code: 0x04, bits: 3 } },
    HandCode { piece: Some(Piece::B_LANCE), code: HuffmanCode { code: 0x01, bits: 5 } },
    HandCode { piece: Some(Piece::W_LANCE), code: HuffmanCode { code: 0x11, bits: 5 } },
    HandCode { piece: Some(Piece::B_KNIGHT), code: HuffmanCode { code: 0x03, bits: 5 } },
    HandCode { piece: Some(Piece::W_KNIGHT), code: HuffmanCode { code: 0x13, bits: 5 } },
    HandCode { piece: Some(Piece::B_SILVER), code: HuffmanCode { code: 0x05, bits: 5 } },
    HandCode { piece: Some(Piece::W_SILVER), code: HuffmanCode { code: 0x15, bits: 5 } },
    HandCode { piece: Some(Piece::B_GOLD), code: HuffmanCode { code: 0x07, bits: 5 } },
    HandCode { piece: Some(Piece::W_GOLD), code: HuffmanCode { code: 0x17, bits: 5 } },
    HandCode { piece: Some(Piece::B_BISHOP), code: HuffmanCode { code: 0x1f, bits: 7 } },
    HandCode { piece: Some(Piece::W_BISHOP), code: HuffmanCode { code: 0x5f, bits: 7 } },
    HandCode { piece: Some(Piece::B_ROOK), code: HuffmanCode { code: 0x3f, bits: 7 } },
    HandCode { piece: Some(Piece::W_ROOK), code: HuffmanCode { code: 0x7f, bits: 7 } },
    HandCode { piece: None, code: HuffmanCode { code: 0x02, bits: 3 } },
    HandCode { piece: None, code: HuffmanCode { code: 0x09, bits: 5 } },
    HandCode { piece: None, code: HuffmanCode { code: 0x0b, bits: 5 } },
    HandCode { piece: None, code: HuffmanCode { code: 0x0d, bits: 5 } },
    HandCode { piece: None, code: HuffmanCode { code: 0x1d, bits: 5 } },
    HandCode { piece: None, code: HuffmanCode { code: 0x0f, bits: 7 } },
    HandCode { piece: None, code: HuffmanCode { code: 0x2f, bits: 7 } },
];

const PIECEBOX_CODES: [(PieceType, HuffmanCode); 7] = [
    (PieceType::PAWN, HuffmanCode { code: 0x02, bits: 3 }),
    (PieceType::LANCE, HuffmanCode { code: 0x09, bits: 5 }),
    (PieceType::KNIGHT, HuffmanCode { code: 0x0b, bits: 5 }),
    (PieceType::SILVER, HuffmanCode { code: 0x0d, bits: 5 }),
    (PieceType::GOLD, HuffmanCode { code: 0x1d, bits: 5 }),
    (PieceType::BISHOP, HuffmanCode { code: 0x0f, bits: 7 }),
    (PieceType::ROOK, HuffmanCode { code: 0x2f, bits: 7 }),
];

const HAND_PIECE_ORDER: [PieceType; 7] = [
    PieceType::PAWN,
    PieceType::LANCE,
    PieceType::KNIGHT,
    PieceType::SILVER,
    PieceType::GOLD,
    PieceType::BISHOP,
    PieceType::ROOK,
];

fn piecebox_index(piece_type: PieceType) -> usize {
    match piece_type {
        PieceType::PAWN => 0,
        PieceType::LANCE => 1,
        PieceType::KNIGHT => 2,
        PieceType::SILVER => 3,
        PieceType::GOLD => 4,
        PieceType::BISHOP => 5,
        PieceType::ROOK => 6,
        _ => unreachable!("piecebox_index only accepts hand pieces"),
    }
}

fn board_code_for_piece(piece: Piece) -> HuffmanCode {
    BOARD_CODES
        .iter()
        .find(|entry| entry.piece == piece)
        .map(|entry| entry.code)
        .expect("piece must be encodable in HuffmanCodedPos")
}

fn hand_code_for_piece(piece: Piece) -> HuffmanCode {
    HAND_CODES
        .iter()
        .find(|entry| entry.piece == Some(piece))
        .map(|entry| entry.code)
        .expect("hand piece must be encodable in HuffmanCodedPos")
}

fn piecebox_code_for_piece_type(piece_type: PieceType) -> HuffmanCode {
    PIECEBOX_CODES
        .iter()
        .find(|(candidate, _)| *candidate == piece_type)
        .map(|(_, code)| *code)
        .expect("piecebox piece must be encodable in HuffmanCodedPos")
}

impl Position {
    #[must_use]
    pub fn to_huffman_coded_pos(&self) -> HuffmanCodedPos {
        let mut words = [0u64; 4];
        let mut writer = BitWriter::new(&mut words);

        writer.write_one_bit(self.turn().raw() != 0);

        for color in [Color::BLACK, Color::WHITE] {
            let king_sq = self.king_square(color);
            let king_idx = u16::try_from(king_sq.raw()).expect("square fits in u16");
            writer.write_n_bits(king_idx, 7);
        }

        let mut piecebox_count = [18, 4, 4, 4, 4, 2, 2];

        for sq_idx in 0..Square::COUNT {
            let sq = Square::from_index(sq_idx);
            let piece = self.piece_on(sq);
            if piece.piece_type() == PieceType::KING {
                continue;
            }
            let code = board_code_for_piece(piece);
            writer.write_n_bits(u16::from(code.code), code.bits);
            if piece != Piece::NONE {
                let base = piece.piece_type().demote();
                let idx = piecebox_index(base);
                piecebox_count[idx] -= 1;
            }
        }

        for color in [Color::BLACK, Color::WHITE] {
            for piece_type in HAND_PIECE_ORDER {
                let hand_piece =
                    HandPiece::from_piece_type(piece_type).expect("hand piece must exist");
                let count = self.hand(color).count(hand_piece);
                for _ in 0..count {
                    let piece = Piece::from_parts(color, piece_type);
                    let code = hand_code_for_piece(piece);
                    writer.write_n_bits(u16::from(code.code), code.bits);
                }
                let idx = piecebox_index(piece_type);
                piecebox_count[idx] -= i32::try_from(count).expect("hand count fits in i32");
            }
        }

        for piece_type in HAND_PIECE_ORDER {
            let count = piecebox_count[piecebox_index(piece_type)];
            let code = piecebox_code_for_piece_type(piece_type);
            for _ in 0..count {
                writer.write_n_bits(u16::from(code.code), code.bits);
            }
        }

        debug_assert!(writer.cursor() == 256, "HuffmanCodedPos must be 256 bits");
        HuffmanCodedPos { data: words_to_bytes(&words) }
    }

    pub fn huffman_coded_pos_unpack(
        packed: &HuffmanCodedPos,
    ) -> Result<String, HuffmanCodedPosError> {
        let words = bytes_to_words(&packed.data);
        let mut reader = BitReader::new(&words);
        let (board, hands, side_to_move) = unpack_raw(&mut reader)?;
        let mut pos = Self::empty();
        pos.board = board;
        pos.hands = hands;
        pos.set_side_to_move(side_to_move);
        pos.ply = 0;
        Ok(generate_sfen(&pos))
    }

    pub fn set_huffman_coded_pos(
        &mut self,
        packed: &HuffmanCodedPos,
        ply: Ply,
    ) -> Result<(), HuffmanCodedPosError> {
        let words = bytes_to_words(&packed.data);
        let mut reader = BitReader::new(&words);
        let (board, hands, side_to_move) = unpack_raw(&mut reader)?;

        self.board = board;
        self.hands = hands;
        self.set_side_to_move(side_to_move);
        self.ply = ply;
        self.entering_king_rule = EnteringKingRule::None;
        self.entering_king_point = [0, 0];

        self.rebuild_bitboards();

        let keys = self.compute_keys();
        self.board_key = keys.board_key;
        self.hand_key = keys.hand_key;
        self.zobrist = self.board_key ^ self.hand_key;

        self.reset_state_stack_to_current_position();
        self.update_entering_point();
        self.debug_assert_partial_keys_consistent();

        Ok(())
    }
}

fn unpack_raw(
    reader: &mut BitReader<'_>,
) -> Result<(BoardArray, [Hand; Color::COUNT], Color), HuffmanCodedPosError> {
    let side_to_move = if reader.read_one_bit()? { Color::WHITE } else { Color::BLACK };

    let mut board = BoardArray::empty();
    for color in [Color::BLACK, Color::WHITE] {
        let sq_raw = reader.read_n_bits(7)?;
        let sq = Square::new(i8::try_from(sq_raw).expect("square fits in i8"));
        if !sq.is_on_board() {
            return Err(HuffmanCodedPosError::InvalidKingSquare(sq_raw));
        }
        board.set(sq, Piece::from_parts(color, PieceType::KING));
    }

    for sq_idx in 0..Square::COUNT {
        let sq = Square::from_index(sq_idx);
        if board.get(sq).piece_type() == PieceType::KING {
            continue;
        }
        let piece = read_board_piece(reader)?;
        if piece != Piece::NONE {
            board.set(sq, piece);
        }
    }

    let mut hands = [Hand::ZERO; Color::COUNT];
    while reader.cursor() < 256 {
        let piece = read_hand_piece(reader)?;
        if piece == Piece::NONE {
            continue;
        }
        let hand_piece =
            HandPiece::from_piece_type(piece.piece_type()).expect("decoded hand piece is valid");
        hands[piece.color().to_index()].add(hand_piece, 1);
    }

    if reader.cursor() != 256 {
        return Err(HuffmanCodedPosError::InvalidCursor);
    }

    Ok((board, hands, side_to_move))
}

fn read_board_piece(reader: &mut BitReader<'_>) -> Result<Piece, HuffmanCodedPosError> {
    let mut code = 0u8;
    let mut bits = 0u8;
    loop {
        code |= u8::from(reader.read_one_bit()?) << bits;
        bits += 1;
        for entry in BOARD_CODES {
            if entry.code.bits == 0 {
                continue;
            }
            if entry.code.code == code && entry.code.bits == bits {
                return Ok(entry.piece);
            }
        }
        if bits > 8 {
            return Err(HuffmanCodedPosError::InvalidPieceCode);
        }
    }
}

fn read_hand_piece(reader: &mut BitReader<'_>) -> Result<Piece, HuffmanCodedPosError> {
    let mut code = 0u8;
    let mut bits = 0u8;
    loop {
        code |= u8::from(reader.read_one_bit()?) << bits;
        bits += 1;
        for entry in HAND_CODES {
            if entry.code.code == code && entry.code.bits == bits {
                return Ok(entry.piece.unwrap_or(Piece::NONE));
            }
        }
        if bits > 7 {
            return Err(HuffmanCodedPosError::InvalidPieceCode);
        }
    }
}
