use super::bit_io::{BitCursorOverflow, BitReader, BitWriter};
use super::{BoardArray, Ply, Position};
use crate::types::{Color, Hand, HandPiece, Piece, PieceType, Square};
use core::{cmp::Ordering, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PackedSfen {
    pub data: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackedSfenError {
    InvalidCursor,
    InvalidPieceCode,
    InvalidKingSquare { color: Color, square: u16 },
    InvalidInventory { piece: PieceType, count: u32, limit: u32 },
}

impl fmt::Display for PackedSfenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCursor => f.write_str("PackedSfen cursor overflow"),
            Self::InvalidPieceCode => f.write_str("invalid PackedSfen piece code"),
            Self::InvalidKingSquare { color, square } => {
                write!(f, "invalid {color:?} king square: {square}")
            }
            Self::InvalidInventory { piece, count, limit } => {
                write!(f, "invalid {piece} inventory: {count} exceeds {limit}")
            }
        }
    }
}

impl std::error::Error for PackedSfenError {}

impl From<BitCursorOverflow> for PackedSfenError {
    fn from(_: BitCursorOverflow) -> Self {
        Self::InvalidCursor
    }
}

pub trait PackedSfenSink {
    fn reset(&mut self, side_to_move: Color);
    fn set_piece(&mut self, square: Square, piece: Piece);
    fn add_hand_piece(&mut self, color: Color, piece: HandPiece);
}

const ORDER: [PieceType; 7] = [
    PieceType::PAWN,
    PieceType::LANCE,
    PieceType::KNIGHT,
    PieceType::SILVER,
    PieceType::GOLD,
    PieceType::BISHOP,
    PieceType::ROOK,
];
const INVENTORY: [u8; 7] = [18, 4, 4, 4, 4, 2, 2];

fn base_code(piece_type: PieceType) -> (u16, u8) {
    match piece_type {
        PieceType::PAWN => (0x01, 2),
        PieceType::LANCE => (0x03, 4),
        PieceType::KNIGHT => (0x0b, 4),
        PieceType::SILVER => (0x07, 4),
        PieceType::BISHOP => (0x1f, 6),
        PieceType::ROOK => (0x3f, 6),
        PieceType::GOLD => (0x0f, 5),
        _ => unreachable!("only base piece types are encoded"),
    }
}

fn box_code(piece_type: PieceType) -> (u16, u8) {
    match piece_type {
        PieceType::PAWN => (0x02, 2),
        PieceType::LANCE => (0x09, 4),
        PieceType::KNIGHT => (0x0d, 4),
        PieceType::SILVER => (0x0b, 4),
        PieceType::BISHOP => (0x2f, 6),
        PieceType::ROOK => (0x3f, 6),
        PieceType::GOLD => (0x1b, 5),
        _ => unreachable!("only base piece types are encoded"),
    }
}

fn read_prefix(
    reader: &mut BitReader<'_>,
    table: &[(u16, u8, PieceType)],
) -> Result<PieceType, PackedSfenError> {
    let mut value = 0;
    for bits in 1..=6 {
        if reader.read_one_bit()? {
            value |= 1 << (bits - 1);
        }
        if let Some((_, _, piece_type)) =
            table.iter().find(|(code, width, _)| *width == bits && *code == value)
        {
            return Ok(*piece_type);
        }
    }
    Err(PackedSfenError::InvalidPieceCode)
}

fn board_prefixes() -> [(u16, u8, PieceType); 8] {
    [
        (0, 1, PieceType::NONE),
        (0x01, 2, PieceType::PAWN),
        (0x03, 4, PieceType::LANCE),
        (0x0b, 4, PieceType::KNIGHT),
        (0x07, 4, PieceType::SILVER),
        (0x1f, 6, PieceType::BISHOP),
        (0x3f, 6, PieceType::ROOK),
        (0x0f, 5, PieceType::GOLD),
    ]
}

pub(super) fn read_board_piece_fast(reader: &mut BitReader<'_>) -> Result<Piece, PackedSfenError> {
    read_board_piece(reader)
}

#[cfg(test)]
pub(super) fn read_hand_piece_fast(reader: &mut BitReader<'_>) -> Result<Piece, PackedSfenError> {
    read_hand_piece(reader)
}

pub(super) fn read_board_piece(reader: &mut BitReader<'_>) -> Result<Piece, PackedSfenError> {
    let piece_type = read_prefix(reader, &board_prefixes())?;
    if piece_type == PieceType::NONE {
        return Ok(Piece::NONE);
    }
    let promoted = if piece_type == PieceType::GOLD { false } else { reader.read_one_bit()? };
    let color = if reader.read_one_bit()? { Color::WHITE } else { Color::BLACK };
    Ok(Piece::from_parts(color, if promoted { piece_type.promote() } else { piece_type }))
}

#[cfg(test)]
pub(super) fn read_hand_piece(reader: &mut BitReader<'_>) -> Result<Piece, PackedSfenError> {
    match read_suffix_piece(reader)? {
        SuffixPiece::Hand(piece) => Ok(piece),
        SuffixPiece::Box(_) => Ok(Piece::NONE),
    }
}

#[derive(Clone, Copy)]
enum SuffixPiece {
    Hand(Piece),
    Box(PieceType),
}

fn read_suffix_piece(reader: &mut BitReader<'_>) -> Result<SuffixPiece, PackedSfenError> {
    let mut table = [(0, 0, Piece::NONE); 21];
    let mut is_box = [false; 21];
    let mut box_types = [PieceType::PAWN; 21];
    let mut next = 0;
    for color in [Color::BLACK, Color::WHITE] {
        for piece_type in ORDER {
            let (base, bits) = base_code(piece_type);
            let prefix = base >> 1;
            let prefix_bits = bits - 1;
            let total_bits =
                if piece_type == PieceType::GOLD { prefix_bits + 1 } else { prefix_bits + 2 };
            let code = prefix | ((color == Color::WHITE) as u16) << (total_bits - 1);
            table[next] = (code, total_bits, Piece::from_parts(color, piece_type));
            next += 1;
        }
    }
    for piece_type in ORDER {
        let (prefix, prefix_bits) = box_code(piece_type);
        table[next] = (
            prefix,
            if piece_type == PieceType::GOLD { prefix_bits } else { prefix_bits + 1 },
            Piece::NONE,
        );
        is_box[next] = true;
        box_types[next] = piece_type;
        next += 1;
    }
    let mut value = 0;
    for bits in 1..=7 {
        if reader.read_one_bit()? {
            value |= 1 << (bits - 1);
        }
        if let Some(index) =
            table.iter().position(|(code, width, _)| *width == bits && *code == value)
        {
            return if is_box[index] {
                Ok(SuffixPiece::Box(box_types[index]))
            } else {
                Ok(SuffixPiece::Hand(table[index].2))
            };
        }
    }
    Err(PackedSfenError::InvalidPieceCode)
}

fn inventory_index(piece_type: PieceType) -> Option<usize> {
    ORDER.iter().position(|&piece| piece == piece_type.demote())
}

fn hands_from_counts(
    counts: [[u8; 7]; Color::COUNT],
) -> Result<[Hand; Color::COUNT], PackedSfenError> {
    let mut hands = [Hand::ZERO; Color::COUNT];
    for color in [Color::BLACK, Color::WHITE] {
        for (index, piece_type) in ORDER.into_iter().enumerate() {
            let hand_piece = HandPiece::from_piece_type(piece_type).expect("declared hand type");
            hands[color.to_index()] = hands[color.to_index()]
                .checked_add(hand_piece, u32::from(counts[color.to_index()][index]))
                .ok_or(PackedSfenError::InvalidPieceCode)?;
        }
    }
    Ok(hands)
}

impl PackedSfen {
    pub const fn from_bytes(data: [u8; 32]) -> Self {
        Self { data }
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.data
    }

    pub fn cmp_bytes(&self, other: &Self) -> Ordering {
        self.data.cmp(&other.data)
    }

    pub fn from_le_u32_words(words: [u32; 8]) -> Self {
        let mut data = [0; 32];
        for (index, word) in words.into_iter().enumerate() {
            data[index * 4..index * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        Self { data }
    }

    pub fn to_le_u32_words(self) -> [u32; 8] {
        let mut words = [0; 8];
        for (index, chunk) in self.data.chunks_exact(4).enumerate() {
            words[index] = u32::from_le_bytes(chunk.try_into().expect("four byte word"));
        }
        words
    }

    pub fn cmp_sbk_words(&self, other: &Self) -> Ordering {
        self.to_le_u32_words().cmp(&other.to_le_u32_words())
    }

    fn decode_state(&self) -> Result<(Color, BoardArray, [Hand; Color::COUNT]), PackedSfenError> {
        let mut reader = BitReader::new(&self.data);
        let side = if reader.read_one_bit()? { Color::WHITE } else { Color::BLACK };
        let black_king = reader.read_n_bits(7)?;
        if black_king >= Square::COUNT as u16 {
            return Err(PackedSfenError::InvalidKingSquare {
                color: Color::BLACK,
                square: black_king,
            });
        }
        let white_king = reader.read_n_bits(7)?;
        if white_king >= Square::COUNT as u16 {
            return Err(PackedSfenError::InvalidKingSquare {
                color: Color::WHITE,
                square: white_king,
            });
        }
        if black_king == white_king {
            return Err(PackedSfenError::InvalidPieceCode);
        }

        let mut board = BoardArray::empty();
        board.set(Square::new(black_king as i8), Piece::from_parts(Color::BLACK, PieceType::KING));
        board.set(Square::new(white_king as i8), Piece::from_parts(Color::WHITE, PieceType::KING));
        let mut inventory = [0u8; 7];
        let mut hand_counts = [[0u8; 7]; Color::COUNT];
        for raw in 0..Square::COUNT {
            let square = Square::new(raw as i8);
            if square.raw() == black_king as i8 || square.raw() == white_king as i8 {
                continue;
            }
            let piece = read_board_piece_fast(&mut reader)?;
            if !piece.is_empty() {
                let Some(index) = inventory_index(piece.piece_type()) else {
                    return Err(PackedSfenError::InvalidPieceCode);
                };
                inventory[index] =
                    inventory[index].checked_add(1).ok_or(PackedSfenError::InvalidPieceCode)?;
                board.set(square, piece);
            }
        }
        while reader.cursor() < 256 {
            match read_suffix_piece(&mut reader)? {
                SuffixPiece::Hand(piece) => {
                    let index = inventory_index(piece.piece_type()).expect("declared hand type");
                    inventory[index] =
                        inventory[index].checked_add(1).ok_or(PackedSfenError::InvalidPieceCode)?;
                    hand_counts[piece.color().to_index()][index] = hand_counts
                        [piece.color().to_index()][index]
                        .checked_add(1)
                        .ok_or(PackedSfenError::InvalidPieceCode)?;
                }
                SuffixPiece::Box(piece_type) => {
                    let index = inventory_index(piece_type).expect("declared box type");
                    inventory[index] =
                        inventory[index].checked_add(1).ok_or(PackedSfenError::InvalidPieceCode)?;
                }
            }
        }
        if reader.cursor() != 256 || inventory != INVENTORY {
            return Err(PackedSfenError::InvalidPieceCode);
        }
        Ok((side, board, hands_from_counts(hand_counts)?))
    }

    pub fn decode_into<S: PackedSfenSink>(
        &self,
        mirror: bool,
        sink: &mut S,
    ) -> Result<(), PackedSfenError> {
        let (side, board, hands) = self.decode_state()?;
        let map = |square: Square| {
            if mirror {
                Square::new((8 - square.file().raw()) * 9 + square.rank().raw())
            } else {
                square
            }
        };
        sink.reset(side);
        for (square, piece) in board.iter() {
            if piece.is_empty() {
                continue;
            }
            sink.set_piece(map(square), piece);
        }
        for color in [Color::BLACK, Color::WHITE] {
            for piece_type in ORDER {
                let hand_piece =
                    HandPiece::from_piece_type(piece_type).expect("declared hand type");
                for _ in 0..hands[color.to_index()].count(hand_piece) {
                    sink.add_hand_piece(color, hand_piece);
                }
            }
        }
        Ok(())
    }
}

struct PositionSink {
    board: BoardArray,
    hands: [Hand; Color::COUNT],
    side: Color,
}

impl Default for PositionSink {
    fn default() -> Self {
        Self { board: BoardArray::empty(), hands: [Hand::ZERO; Color::COUNT], side: Color::BLACK }
    }
}

impl PackedSfenSink for PositionSink {
    fn reset(&mut self, side_to_move: Color) {
        self.board = BoardArray::empty();
        self.hands = [Hand::ZERO; Color::COUNT];
        self.side = side_to_move;
    }

    fn set_piece(&mut self, square: Square, piece: Piece) {
        self.board.set(square, piece);
    }

    fn add_hand_piece(&mut self, color: Color, piece: HandPiece) {
        self.hands[color.to_index()].add(piece, 1);
    }
}

impl Position {
    /// 局面を PackedSfen に変換する。
    ///
    /// # Panics
    ///
    /// 玉位置または駒の総数が PackedSfen の表現可能範囲外の場合に panic する。
    /// 外部入力から構築した未検証局面には [`Self::try_to_packed_sfen`] を使う。
    pub fn to_packed_sfen(&self) -> PackedSfen {
        self.try_to_packed_sfen().expect("position must be representable as PackedSfen")
    }

    /// 局面を PackedSfen に変換し、表現不能な局面をエラーにする。
    pub fn try_to_packed_sfen(&self) -> Result<PackedSfen, PackedSfenError> {
        let black_king = self.king_square(Color::BLACK);
        let white_king = self.king_square(Color::WHITE);
        if !black_king.is_valid() {
            return Err(PackedSfenError::InvalidKingSquare {
                color: Color::BLACK,
                square: black_king.raw() as u16,
            });
        }
        if !white_king.is_valid() {
            return Err(PackedSfenError::InvalidKingSquare {
                color: Color::WHITE,
                square: white_king.raw() as u16,
            });
        }
        if black_king == white_king {
            return Err(PackedSfenError::InvalidPieceCode);
        }

        let mut packed = PackedSfen::default();
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
            if piece.is_empty() {
                writer.write_one_bit(false);
                continue;
            }
            let base = piece.piece_type().demote();
            let Some(index) = inventory_index(base) else {
                return Err(PackedSfenError::InvalidPieceCode);
            };
            used[index] = used[index].checked_add(1).ok_or(PackedSfenError::InvalidInventory {
                piece: base,
                count: u32::MAX,
                limit: u32::from(INVENTORY[index]),
            })?;
            if used[index] > u32::from(INVENTORY[index]) {
                return Err(PackedSfenError::InvalidInventory {
                    piece: base,
                    count: used[index],
                    limit: u32::from(INVENTORY[index]),
                });
            }
            let (code, bits) = base_code(base);
            writer.write_n_bits(code, bits);
            if base != PieceType::GOLD {
                writer.write_one_bit(piece.piece_type().is_promoted());
            }
            writer.write_one_bit(piece.color() == Color::WHITE);
        }
        for color in [Color::BLACK, Color::WHITE] {
            for (index, piece_type) in ORDER.into_iter().enumerate() {
                let count = self
                    .hand(color)
                    .count(HandPiece::from_piece_type(piece_type).expect("hand type"));
                used[index] =
                    used[index].checked_add(count).ok_or(PackedSfenError::InvalidInventory {
                        piece: piece_type,
                        count: u32::MAX,
                        limit: u32::from(INVENTORY[index]),
                    })?;
                if used[index] > u32::from(INVENTORY[index]) {
                    return Err(PackedSfenError::InvalidInventory {
                        piece: piece_type,
                        count: used[index],
                        limit: u32::from(INVENTORY[index]),
                    });
                }
                let (code, bits) = base_code(piece_type);
                for _ in 0..count {
                    writer.write_n_bits(code >> 1, bits - 1);
                    if piece_type != PieceType::GOLD {
                        writer.write_one_bit(false);
                    }
                    writer.write_one_bit(color == Color::WHITE);
                }
            }
        }
        for (index, piece_type) in ORDER.into_iter().enumerate() {
            let (code, bits) = box_code(piece_type);
            for _ in used[index]..u32::from(INVENTORY[index]) {
                writer.write_n_bits(code, bits);
                if piece_type != PieceType::GOLD {
                    writer.write_one_bit(false);
                }
            }
        }
        if writer.cursor() != 256 {
            return Err(PackedSfenError::InvalidCursor);
        }
        Ok(packed)
    }

    pub fn sfen_unpack(packed: &PackedSfen) -> Result<String, PackedSfenError> {
        let mut position = Position::empty();
        position.set_packed_sfen(packed, false, 0)?;
        Ok(position.to_sfen(None))
    }

    pub fn set_packed_sfen(
        &mut self,
        packed: &PackedSfen,
        mirror: bool,
        ply: Ply,
    ) -> Result<(), PackedSfenError> {
        let mut sink = PositionSink::default();
        packed.decode_into(mirror, &mut sink)?;
        self.board = sink.board;
        self.hands = sink.hands;
        self.side_to_move = sink.side;
        self.ply = ply;
        self.entering_king_rule = crate::types::EnteringKingRule::None;
        self.entering_king_point = [0, 0];
        self.rebuild_bitboards();
        self.reset_state_stack_to_current_position();
        Ok(())
    }
}
