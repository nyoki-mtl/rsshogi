use super::bit_io::{BitCursorOverflow, BitReader, BitWriter};
use super::{BoardArray, Ply, Position};
use crate::types::{Color, Hand, HandPiece, Piece, PieceType, Square};
use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HuffmanCodedPos {
    pub data: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HuffmanCodedPosError {
    InvalidCursor,
    InvalidPieceCode,
    InvalidKingSquare(u16),
    InvalidInventory { piece: PieceType, count: u32, limit: u32 },
}

impl fmt::Display for HuffmanCodedPosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCursor => f.write_str("Huffman cursor overflow"),
            Self::InvalidPieceCode => f.write_str("invalid Huffman piece code"),
            Self::InvalidKingSquare(square) => write!(f, "invalid king square: {square}"),
            Self::InvalidInventory { piece, count, limit } => {
                write!(f, "invalid {piece} inventory: {count} exceeds {limit}")
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

const HAND_ORDER: [PieceType; 7] = [
    PieceType::PAWN,
    PieceType::LANCE,
    PieceType::KNIGHT,
    PieceType::SILVER,
    PieceType::GOLD,
    PieceType::BISHOP,
    PieceType::ROOK,
];
const INVENTORY: [u8; 7] = [18, 4, 4, 4, 4, 2, 2];

fn board_code(piece: Piece) -> Option<(u16, u8)> {
    let white = piece.color() == Color::WHITE;
    let (code, bits) = match piece.piece_type() {
        PieceType::NONE => return Some((0, 1)),
        PieceType::PAWN => (if white { 0x05 } else { 0x01 }, 4),
        PieceType::LANCE => (if white { 0x13 } else { 0x03 }, 6),
        PieceType::KNIGHT => (if white { 0x17 } else { 0x07 }, 6),
        PieceType::SILVER => (if white { 0x1b } else { 0x0b }, 6),
        PieceType::GOLD => (if white { 0x2f } else { 0x0f }, 6),
        PieceType::BISHOP => (if white { 0x5f } else { 0x1f }, 8),
        PieceType::ROOK => (if white { 0x7f } else { 0x3f }, 8),
        PieceType::PRO_PAWN => (if white { 0x0d } else { 0x09 }, 4),
        PieceType::PRO_LANCE => (if white { 0x33 } else { 0x23 }, 6),
        PieceType::PRO_KNIGHT => (if white { 0x37 } else { 0x27 }, 6),
        PieceType::PRO_SILVER => (if white { 0x3b } else { 0x2b }, 6),
        PieceType::HORSE => (if white { 0xdf } else { 0x9f }, 8),
        PieceType::DRAGON => (if white { 0xff } else { 0xbf }, 8),
        _ => return None,
    };
    Some((code, bits))
}

fn hand_code(color: Color, piece: PieceType) -> (u16, u8) {
    let white = color == Color::WHITE;
    match piece {
        PieceType::PAWN => (if white { 0x04 } else { 0x00 }, 3),
        PieceType::LANCE => (if white { 0x11 } else { 0x01 }, 5),
        PieceType::KNIGHT => (if white { 0x13 } else { 0x03 }, 5),
        PieceType::SILVER => (if white { 0x15 } else { 0x05 }, 5),
        PieceType::GOLD => (if white { 0x17 } else { 0x07 }, 5),
        PieceType::BISHOP => (if white { 0x5f } else { 0x1f }, 7),
        PieceType::ROOK => (if white { 0x7f } else { 0x3f }, 7),
        _ => unreachable!("only hand piece types are encoded"),
    }
}

fn box_code(piece: PieceType) -> (u16, u8) {
    match piece {
        PieceType::PAWN => (0x02, 3),
        PieceType::LANCE => (0x09, 5),
        PieceType::KNIGHT => (0x0b, 5),
        PieceType::SILVER => (0x0d, 5),
        PieceType::GOLD => (0x1d, 5),
        PieceType::BISHOP => (0x0f, 7),
        PieceType::ROOK => (0x2f, 7),
        _ => unreachable!("only box piece types are encoded"),
    }
}

fn decode_code<T: Copy>(
    reader: &mut BitReader<'_>,
    table: &[(u16, u8, T)],
) -> Result<T, HuffmanCodedPosError> {
    let mut value = 0;
    for bits in 1..=8 {
        if reader.read_one_bit()? {
            value |= 1 << (bits - 1);
        }
        if let Some((_, _, item)) =
            table.iter().find(|(code, width, _)| *width == bits && *code == value)
        {
            return Ok(*item);
        }
    }
    Err(HuffmanCodedPosError::InvalidPieceCode)
}

fn decode_board_piece(reader: &mut BitReader<'_>) -> Result<Piece, HuffmanCodedPosError> {
    let mut table = [(0, 0, Piece::NONE); 29];
    let mut next = 0;
    table[next] = (0, 1, Piece::NONE);
    next += 1;
    for color in [Color::BLACK, Color::WHITE] {
        for piece_type in [
            PieceType::PAWN,
            PieceType::LANCE,
            PieceType::KNIGHT,
            PieceType::SILVER,
            PieceType::GOLD,
            PieceType::BISHOP,
            PieceType::ROOK,
            PieceType::PRO_PAWN,
            PieceType::PRO_LANCE,
            PieceType::PRO_KNIGHT,
            PieceType::PRO_SILVER,
            PieceType::HORSE,
            PieceType::DRAGON,
        ] {
            let piece = Piece::from_parts(color, piece_type);
            let (code, bits) = board_code(piece).expect("declared board code");
            table[next] = (code, bits, piece);
            next += 1;
        }
    }
    decode_code(reader, &table)
}

#[derive(Clone, Copy)]
enum SuffixPiece {
    Hand(Color, PieceType),
    Box(PieceType),
}

fn decode_suffix_piece(reader: &mut BitReader<'_>) -> Result<SuffixPiece, HuffmanCodedPosError> {
    let mut table = [(0, 0, SuffixPiece::Box(PieceType::PAWN)); 21];
    let mut next = 0;
    for color in [Color::BLACK, Color::WHITE] {
        for piece_type in HAND_ORDER {
            let (code, bits) = hand_code(color, piece_type);
            table[next] = (code, bits, SuffixPiece::Hand(color, piece_type));
            next += 1;
        }
    }
    for piece_type in HAND_ORDER {
        let (code, bits) = box_code(piece_type);
        table[next] = (code, bits, SuffixPiece::Box(piece_type));
        next += 1;
    }
    decode_code(reader, &table)
}

fn inventory_index(piece_type: PieceType) -> Option<usize> {
    HAND_ORDER.iter().position(|&piece| piece == piece_type.demote())
}

fn hands_from_counts(
    counts: [[u8; 7]; Color::COUNT],
) -> Result<[Hand; Color::COUNT], HuffmanCodedPosError> {
    let mut hands = [Hand::ZERO; Color::COUNT];
    for color in [Color::BLACK, Color::WHITE] {
        for (index, piece_type) in HAND_ORDER.into_iter().enumerate() {
            let hand_piece = HandPiece::from_piece_type(piece_type).expect("declared hand type");
            hands[color.to_index()] = hands[color.to_index()]
                .checked_add(hand_piece, u32::from(counts[color.to_index()][index]))
                .ok_or(HuffmanCodedPosError::InvalidPieceCode)?;
        }
    }
    Ok(hands)
}

impl Position {
    /// 局面を HuffmanCodedPos に変換する。
    ///
    /// # Panics
    ///
    /// 玉位置または駒の総数が HCP の表現可能範囲外の場合に panic する。
    /// 外部入力から構築した未検証局面には [`Self::try_to_huffman_coded_pos`] を使う。
    pub fn to_huffman_coded_pos(&self) -> HuffmanCodedPos {
        self.try_to_huffman_coded_pos().expect("position must be representable as HuffmanCodedPos")
    }

    /// 局面を HuffmanCodedPos に変換し、表現不能な局面をエラーにする。
    pub fn try_to_huffman_coded_pos(&self) -> Result<HuffmanCodedPos, HuffmanCodedPosError> {
        let black_king = self.king_square(Color::BLACK);
        let white_king = self.king_square(Color::WHITE);
        if !black_king.is_valid() {
            return Err(HuffmanCodedPosError::InvalidKingSquare(black_king.raw() as u16));
        }
        if !white_king.is_valid() {
            return Err(HuffmanCodedPosError::InvalidKingSquare(white_king.raw() as u16));
        }
        if black_king == white_king {
            return Err(HuffmanCodedPosError::InvalidPieceCode);
        }

        let mut packed = HuffmanCodedPos::default();
        let mut writer = BitWriter::new(&mut packed.data);
        writer.write_one_bit(self.turn() == Color::WHITE);
        writer.write_n_bits(black_king.raw() as u16, 7);
        writer.write_n_bits(white_king.raw() as u16, 7);

        let mut used = [0u32; 7];
        for raw in 0..Square::COUNT {
            let square = Square::new(raw as i8);
            if square == black_king || square == white_king {
                continue;
            }
            let piece = self.piece_on(square);
            let (code, bits) = board_code(piece).ok_or(HuffmanCodedPosError::InvalidPieceCode)?;
            if !piece.is_empty() {
                let base = piece.piece_type().demote();
                let Some(index) = inventory_index(base) else {
                    return Err(HuffmanCodedPosError::InvalidPieceCode);
                };
                used[index] =
                    used[index].checked_add(1).ok_or(HuffmanCodedPosError::InvalidInventory {
                        piece: base,
                        count: u32::MAX,
                        limit: u32::from(INVENTORY[index]),
                    })?;
                if used[index] > u32::from(INVENTORY[index]) {
                    return Err(HuffmanCodedPosError::InvalidInventory {
                        piece: base,
                        count: used[index],
                        limit: u32::from(INVENTORY[index]),
                    });
                }
            }
            writer.write_n_bits(code, bits);
        }
        for color in [Color::BLACK, Color::WHITE] {
            for (index, piece_type) in HAND_ORDER.into_iter().enumerate() {
                let count = self
                    .hand(color)
                    .count(HandPiece::from_piece_type(piece_type).expect("hand type"));
                used[index] = used[index].checked_add(count).ok_or(
                    HuffmanCodedPosError::InvalidInventory {
                        piece: piece_type,
                        count: u32::MAX,
                        limit: u32::from(INVENTORY[index]),
                    },
                )?;
                if used[index] > u32::from(INVENTORY[index]) {
                    return Err(HuffmanCodedPosError::InvalidInventory {
                        piece: piece_type,
                        count: used[index],
                        limit: u32::from(INVENTORY[index]),
                    });
                }
                let (code, bits) = hand_code(color, piece_type);
                for _ in 0..count {
                    writer.write_n_bits(code, bits);
                }
            }
        }
        for (index, piece_type) in HAND_ORDER.into_iter().enumerate() {
            let (code, bits) = box_code(piece_type);
            for _ in used[index]..u32::from(INVENTORY[index]) {
                writer.write_n_bits(code, bits);
            }
        }
        if writer.cursor() != 256 {
            return Err(HuffmanCodedPosError::InvalidCursor);
        }
        Ok(packed)
    }

    pub fn huffman_coded_pos_unpack(
        packed: &HuffmanCodedPos,
    ) -> Result<String, HuffmanCodedPosError> {
        let mut position = Position::empty();
        position.set_huffman_coded_pos(packed, 0)?;
        Ok(position.to_sfen(None))
    }

    pub fn set_huffman_coded_pos(
        &mut self,
        packed: &HuffmanCodedPos,
        ply: Ply,
    ) -> Result<(), HuffmanCodedPosError> {
        let mut reader = BitReader::new(&packed.data);
        let side = if reader.read_one_bit()? { Color::WHITE } else { Color::BLACK };
        let black_king = reader.read_n_bits(7)?;
        if black_king >= Square::COUNT as u16 {
            return Err(HuffmanCodedPosError::InvalidKingSquare(black_king));
        }
        let white_king = reader.read_n_bits(7)?;
        if white_king >= Square::COUNT as u16 {
            return Err(HuffmanCodedPosError::InvalidKingSquare(white_king));
        }
        if black_king == white_king {
            return Err(HuffmanCodedPosError::InvalidPieceCode);
        }
        let black_king = Square::new(black_king as i8);
        let white_king = Square::new(white_king as i8);
        let mut board = BoardArray::empty();
        let mut inventory = [0u8; 7];
        let mut hand_counts = [[0u8; 7]; Color::COUNT];
        board.set(black_king, Piece::from_parts(Color::BLACK, PieceType::KING));
        board.set(white_king, Piece::from_parts(Color::WHITE, PieceType::KING));
        for raw in 0..Square::COUNT {
            let square = Square::new(raw as i8);
            if square == black_king || square == white_king {
                continue;
            }
            let piece = decode_board_piece(&mut reader)?;
            if !piece.is_empty() {
                let Some(index) = inventory_index(piece.piece_type()) else {
                    return Err(HuffmanCodedPosError::InvalidPieceCode);
                };
                inventory[index] = inventory[index]
                    .checked_add(1)
                    .ok_or(HuffmanCodedPosError::InvalidPieceCode)?;
            }
            board.set(square, piece);
        }
        while reader.cursor() < 256 {
            match decode_suffix_piece(&mut reader)? {
                SuffixPiece::Hand(color, piece_type) => {
                    let index = inventory_index(piece_type).expect("declared hand type");
                    inventory[index] = inventory[index]
                        .checked_add(1)
                        .ok_or(HuffmanCodedPosError::InvalidPieceCode)?;
                    hand_counts[color.to_index()][index] = hand_counts[color.to_index()][index]
                        .checked_add(1)
                        .ok_or(HuffmanCodedPosError::InvalidPieceCode)?;
                }
                SuffixPiece::Box(piece_type) => {
                    let index = inventory_index(piece_type).expect("declared box type");
                    inventory[index] = inventory[index]
                        .checked_add(1)
                        .ok_or(HuffmanCodedPosError::InvalidPieceCode)?;
                }
            }
        }
        if reader.cursor() != 256 {
            return Err(HuffmanCodedPosError::InvalidCursor);
        }
        if inventory != INVENTORY {
            return Err(HuffmanCodedPosError::InvalidPieceCode);
        }
        let hands = hands_from_counts(hand_counts)?;
        self.board = board;
        self.hands = hands;
        self.side_to_move = side;
        self.ply = ply;
        self.entering_king_rule = crate::types::EnteringKingRule::None;
        self.entering_king_point = [0, 0];
        self.rebuild_bitboards();
        self.reset_state_stack_to_current_position();
        Ok(())
    }
}
