use std::cmp::Ordering;

use super::bit_io::{BitCursorOverflow, BitReader, BitWriter, bytes_to_words, words_to_bytes};
use super::{Ply, Position};
use crate::board::parser::generate_sfen;
use crate::board::position::BoardArray;
use crate::types::{Color, EnteringKingRule, Hand, HandPiece, Piece, PieceType, Square};

// ANCHOR: packed_sfen_struct
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PackedSfen {
    pub data: [u8; 32],
}
// ANCHOR_END: packed_sfen_struct

impl PackedSfen {
    /// 32 バイトのリトルエンディアン表現から packed SFEN キーを生成する。
    #[must_use]
    pub const fn from_bytes(data: [u8; 32]) -> Self {
        Self { data }
    }

    /// 32 バイトのリトルエンディアン packed SFEN 表現を返す。
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.data
    }

    /// packed SFEN の 32 バイト表現をそのまま辞書順で比較する。
    ///
    /// `.ybb` のインデックス順序はこの byte order を使う。SBK / BookConv の
    /// `u32` word order とは意図的に分離している。
    #[must_use]
    pub fn cmp_bytes(&self, other: &Self) -> Ordering {
        self.data.cmp(&other.data)
    }

    /// SBK が使用する 8 個のリトルエンディアン 32 ビットワードから packed SFEN キーを生成する。
    #[must_use]
    pub fn from_le_u32_words(words: [u32; 8]) -> Self {
        let mut data = [0u8; 32];
        for (idx, word) in words.iter().enumerate() {
            let start = idx * 4;
            data[start..start + 4].copy_from_slice(&word.to_le_bytes());
        }
        Self { data }
    }

    /// SBK / BookConv 検索で使用する 8 個のリトルエンディアン 32 ビットワードを返す。
    #[must_use]
    pub fn to_le_u32_words(self) -> [u32; 8] {
        let mut words = [0u32; 8];
        for (idx, word) in words.iter_mut().enumerate() {
            let start = idx * 4;
            let chunk: [u8; 4] = self.data[start..start + 4].try_into().expect("chunk fits");
            *word = u32::from_le_bytes(chunk);
        }
        words
    }

    /// SBK / BookConv のワード順序で packed SFEN キーを比較する。
    ///
    /// Rust のバイト順序とは意図的に分離している。ShogiHome と
    /// BookConv は行を 8 個のリトルエンディアン `u32` ワードとしてソートするため。
    #[must_use]
    pub fn cmp_sbk_words(&self, other: &Self) -> Ordering {
        self.to_le_u32_words().cmp(&other.to_le_u32_words())
    }

    /// Packed SFEN を caller-owned sink へ直接デコードする。
    ///
    /// `Position` の bitboard、Zobrist key、state stack を必要としない consumer
    /// 向けの allocation-free 経路である。エラー時は sink に途中まで書き込まれる。
    ///
    /// # Errors
    ///
    /// bitstream が 256 ビット境界を越える場合、駒符号が不正な場合、または玉の升が
    /// 盤外を示す場合に返す。
    pub fn decode_into<S: PackedSfenSink>(
        &self,
        mirror: bool,
        sink: &mut S,
    ) -> Result<(), PackedSfenError> {
        decode_packed_sfen_into(self, mirror, sink)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackedSfenError {
    InvalidCursor,
    InvalidPieceCode,
    InvalidKingSquare { color: Color, square: u16 },
}

impl From<BitCursorOverflow> for PackedSfenError {
    fn from(_: BitCursorOverflow) -> Self {
        Self::InvalidCursor
    }
}

/// Packed SFEN の局面要素を受け取る sink。
///
/// decoder は最初に [`Self::reset`] を 1 回呼び、続いて盤上駒と持ち駒を渡す。
/// 空マスと駒箱の駒は通知しない。
pub trait PackedSfenSink {
    /// 既存内容を消去し、手番を設定する。
    fn reset(&mut self, side_to_move: Color);

    /// 指定した升へ駒を設定する。
    fn set_piece(&mut self, square: Square, piece: Piece);

    /// 指定した手番の持ち駒を 1 枚増やす。
    fn add_hand_piece(&mut self, color: Color, piece: HandPiece);
}

#[derive(Clone, Copy)]
struct HuffmanPiece {
    code: u8,
    bits: u8,
}

// ANCHOR: huffman_table
const HUFFMAN_TABLE: [HuffmanPiece; 8] = [
    HuffmanPiece { code: 0x00, bits: 1 }, // 駒なし
    HuffmanPiece { code: 0x01, bits: 2 }, // 歩
    HuffmanPiece { code: 0x03, bits: 4 }, // 香
    HuffmanPiece { code: 0x0b, bits: 4 }, // 桂
    HuffmanPiece { code: 0x07, bits: 4 }, // 銀
    HuffmanPiece { code: 0x1f, bits: 6 }, // 角
    HuffmanPiece { code: 0x3f, bits: 6 }, // 飛
    HuffmanPiece { code: 0x0f, bits: 5 }, // 金
];
// ANCHOR_END: huffman_table

const HUFFMAN_TABLE_PIECEBOX: [HuffmanPiece; 8] = [
    HuffmanPiece { code: 0x00, bits: 1 }, // 未使用
    HuffmanPiece { code: 0x02, bits: 2 }, // 歩
    HuffmanPiece { code: 0x09, bits: 4 }, // 香
    HuffmanPiece { code: 0x0d, bits: 4 }, // 桂
    HuffmanPiece { code: 0x0b, bits: 4 }, // 銀
    HuffmanPiece { code: 0x2f, bits: 6 }, // 角
    HuffmanPiece { code: 0x3f, bits: 6 }, // 飛
    HuffmanPiece { code: 0x1b, bits: 5 }, // 金（駒箱）
];

#[derive(Clone, Copy)]
struct HuffmanDecodeEntry {
    piece: Piece,
    consumed_bits: u8,
}

const BOARD_DECODE_BITS: u8 = 8;
const HAND_DECODE_BITS: u8 = 7;

const fn piece_type_from_huffman_index(index: usize) -> PieceType {
    match index {
        0 => PieceType::NONE,
        1 => PieceType::PAWN,
        2 => PieceType::LANCE,
        3 => PieceType::KNIGHT,
        4 => PieceType::SILVER,
        5 => PieceType::BISHOP,
        6 => PieceType::ROOK,
        7 => PieceType::GOLD,
        _ => panic!("invalid Huffman table index"),
    }
}

const fn build_decode_table<const N: usize>(hand: bool) -> [HuffmanDecodeEntry; N] {
    let mut table = [HuffmanDecodeEntry { piece: Piece::NONE, consumed_bits: 0 }; N];
    let mut value = 0usize;
    while value < N {
        let mut index = if hand { 1 } else { 0 };
        let mut matched = false;
        while index < HUFFMAN_TABLE.len() {
            let mut code = HUFFMAN_TABLE[index].code;
            let mut bits = HUFFMAN_TABLE[index].bits;
            if hand {
                code >>= 1;
                bits -= 1;
            }
            let mask = (1usize << bits) - 1;
            if value & mask == code as usize {
                let base = piece_type_from_huffman_index(index);
                let mut consumed_bits = bits;
                if index == 0 {
                    table[value] = HuffmanDecodeEntry { piece: Piece::NONE, consumed_bits };
                    matched = true;
                    break;
                }

                let promoted = if index == 7 {
                    false
                } else {
                    let promoted = ((value >> consumed_bits) & 1) != 0;
                    consumed_bits += 1;
                    promoted
                };
                let color =
                    if ((value >> consumed_bits) & 1) == 0 { Color::BLACK } else { Color::WHITE };
                consumed_bits += 1;
                let piece_type = if promoted { base.promote() } else { base };
                table[value] = HuffmanDecodeEntry {
                    piece: Piece::from_parts(color, piece_type),
                    consumed_bits,
                };
                matched = true;
                break;
            }
            index += 1;
        }
        if !matched {
            panic!("Huffman decode table does not cover every prefix");
        }
        value += 1;
    }
    table
}

const BOARD_DECODE_TABLE: [HuffmanDecodeEntry; 1 << BOARD_DECODE_BITS] = build_decode_table(false);
const HAND_DECODE_TABLE: [HuffmanDecodeEntry; 1 << HAND_DECODE_BITS] = build_decode_table(true);

const APERY_PIECES: [PieceType; 8] = [
    PieceType::NONE,
    PieceType::PAWN,
    PieceType::LANCE,
    PieceType::KNIGHT,
    PieceType::SILVER,
    PieceType::GOLD,
    PieceType::BISHOP,
    PieceType::ROOK,
];

const fn huffman_index(piece_type: PieceType) -> Option<usize> {
    match piece_type {
        PieceType::NONE => Some(0),
        PieceType::PAWN => Some(1),
        PieceType::LANCE => Some(2),
        PieceType::KNIGHT => Some(3),
        PieceType::SILVER => Some(4),
        PieceType::BISHOP => Some(5),
        PieceType::ROOK => Some(6),
        PieceType::GOLD => Some(7),
        _ => None,
    }
}

const fn is_promoted_piece(piece: Piece) -> bool {
    piece.piece_type().raw() >= 9
}

const fn base_piece_type(piece: Piece) -> PieceType {
    piece.piece_type().demote()
}

impl Position {
    #[must_use]
    pub fn to_packed_sfen(&self) -> PackedSfen {
        let mut words = [0u64; 4];
        let mut writer = BitWriter::new(&mut words);

        writer.write_one_bit(self.turn().raw() != 0);

        for color in [Color::BLACK, Color::WHITE] {
            let king_sq = self.king_square(color);
            let king_idx = u16::try_from(king_sq.raw()).expect("square fits in u16");
            writer.write_n_bits(king_idx, 7);
        }

        let mut piecebox_count = [0i32; 8];
        piecebox_count[1] = 18;
        piecebox_count[2] = 4;
        piecebox_count[3] = 4;
        piecebox_count[4] = 4;
        piecebox_count[5] = 2;
        piecebox_count[6] = 2;
        piecebox_count[7] = 4;

        for sq_idx in 0..Square::COUNT {
            let sq = Square::from_index(sq_idx);
            let piece = self.piece_on(sq);
            if piece.piece_type() == PieceType::KING {
                continue;
            }
            write_board_piece(&mut writer, piece);
            if piece != Piece::NONE {
                let raw = base_piece_type(piece);
                if let Some(idx) = huffman_index(raw) {
                    piecebox_count[idx] -= 1;
                }
            }
        }

        for color in [Color::BLACK, Color::WHITE] {
            for &piece_type in APERY_PIECES.iter().skip(1) {
                let hand_piece =
                    HandPiece::from_piece_type(piece_type).expect("hand piece must exist");
                let count = self.hand(color).count(hand_piece);
                for _ in 0..count {
                    let piece = Piece::from_parts(color, piece_type);
                    write_hand_piece(&mut writer, piece);
                }
                let count_i32 = i32::try_from(count).expect("hand count fits in i32");
                if let Some(idx) = huffman_index(piece_type) {
                    piecebox_count[idx] -= count_i32;
                }
            }
        }

        for &piece_type in APERY_PIECES.iter().skip(1) {
            let idx = huffman_index(piece_type).expect("piecebox piece must be encodable");
            let count = piecebox_count[idx];
            for _ in 0..count {
                write_piecebox_piece(&mut writer, piece_type);
            }
        }

        debug_assert!(writer.cursor() == 256, "packed sfen must be 256 bits");
        PackedSfen { data: words_to_bytes(&words) }
    }

    pub fn sfen_unpack(packed: &PackedSfen) -> Result<String, PackedSfenError> {
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

    pub fn set_packed_sfen(
        &mut self,
        packed: &PackedSfen,
        mirror: bool,
        ply: Ply,
    ) -> Result<(), PackedSfenError> {
        let words = bytes_to_words(&packed.data);
        let mut reader = BitReader::new(&words);
        let (mut board, hands, side_to_move) = unpack_raw(&mut reader)?;

        if mirror {
            let mut mirrored = BoardArray::empty();
            for sq_idx in 0..Square::COUNT {
                let sq = Square::from_index(sq_idx);
                let msq = sq.mirror_file();
                let piece = board.get(sq);
                if piece.is_empty() {
                    continue;
                }
                mirrored.set(msq, piece);
            }
            board = mirrored;
        }

        self.board = board;
        self.hands = hands;
        self.set_side_to_move(side_to_move);
        self.ply = ply;
        self.entering_king_rule = EnteringKingRule::None;
        self.entering_king_point = [0, 0];

        self.rebuild_bitboards();

        let keys = self.compute_keys();
        let (board_key, hand_key) = (keys.board_key, keys.hand_key);
        self.board_key = board_key;
        self.hand_key = hand_key;
        self.zobrist = board_key ^ hand_key;

        self.reset_state_stack_to_current_position();
        self.update_entering_point();
        self.debug_assert_partial_keys_consistent();

        Ok(())
    }
}

fn unpack_raw(
    reader: &mut BitReader<'_>,
) -> Result<(BoardArray, [Hand; Color::COUNT], Color), PackedSfenError> {
    let side_to_move = if reader.read_one_bit()? { Color::WHITE } else { Color::BLACK };

    let mut board = BoardArray::empty();
    for color in [Color::BLACK, Color::WHITE] {
        let raw = reader.read_n_bits(7)?;
        let square = Square::new(i8::try_from(raw).expect("square fits in i8"));
        if !square.is_on_board() {
            return Err(PackedSfenError::InvalidKingSquare { color, square: raw });
        }
        board.set(square, Piece::from_parts(color, PieceType::KING));
    }

    for sq_idx in 0..Square::COUNT {
        let sq = Square::from_index(sq_idx);
        let existing = board.get(sq);
        if existing.piece_type() == PieceType::KING {
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
        if is_promoted_piece(piece) {
            continue;
        }
        let hp =
            HandPiece::from_piece_type(piece.piece_type()).expect("hand piece must be promotable");
        let idx = piece.color().to_index();
        hands[idx].add(hp, 1);
    }

    if reader.cursor() != 256 {
        return Err(PackedSfenError::InvalidCursor);
    }

    Ok((board, hands, side_to_move))
}

fn decode_packed_sfen_into<S: PackedSfenSink>(
    packed: &PackedSfen,
    mirror: bool,
    sink: &mut S,
) -> Result<(), PackedSfenError> {
    let words = bytes_to_words(&packed.data);
    let mut reader = BitReader::new(&words);
    let side_to_move = if reader.read_one_bit()? { Color::WHITE } else { Color::BLACK };
    sink.reset(side_to_move);

    let mut king_squares = [Square::NONE; Color::COUNT];
    for color in [Color::BLACK, Color::WHITE] {
        let raw = reader.read_n_bits(7)?;
        let square = Square::new(i8::try_from(raw).expect("square fits in i8"));
        if !square.is_on_board() {
            return Err(PackedSfenError::InvalidKingSquare { color, square: raw });
        }
        king_squares[color.to_index()] = square;
        let output_square = if mirror { square.mirror_file() } else { square };
        sink.set_piece(output_square, Piece::from_parts(color, PieceType::KING));
    }

    for square_index in 0..Square::COUNT {
        let square = Square::from_index(square_index);
        if square == king_squares[0] || square == king_squares[1] {
            continue;
        }
        let piece = read_board_piece_fast(&mut reader)?;
        if piece != Piece::NONE {
            let output_square = if mirror { square.mirror_file() } else { square };
            sink.set_piece(output_square, piece);
        }
    }

    while reader.cursor() < 256 {
        let piece = read_hand_piece_fast(&mut reader)?;
        if is_promoted_piece(piece) {
            continue;
        }
        let hand_piece =
            HandPiece::from_piece_type(piece.piece_type()).expect("hand piece must exist");
        sink.add_hand_piece(piece.color(), hand_piece);
    }
    if reader.cursor() != 256 {
        return Err(PackedSfenError::InvalidCursor);
    }
    Ok(())
}

fn write_board_piece(writer: &mut BitWriter<'_>, piece: Piece) {
    let raw = base_piece_type(piece);
    let idx = huffman_index(raw).expect("board piece must be encodable");
    let code = HUFFMAN_TABLE[idx].code;
    let bits = HUFFMAN_TABLE[idx].bits;
    writer.write_n_bits(u16::from(code), bits);
    if piece == Piece::NONE {
        return;
    }
    if raw != PieceType::GOLD {
        writer.write_one_bit(is_promoted_piece(piece));
    }
    writer.write_one_bit(piece.color().raw() != 0);
}

fn write_hand_piece(writer: &mut BitWriter<'_>, piece: Piece) {
    let raw = base_piece_type(piece);
    let idx = huffman_index(raw).expect("hand piece must be encodable");
    let code = HUFFMAN_TABLE[idx].code >> 1;
    let bits = HUFFMAN_TABLE[idx].bits - 1;
    writer.write_n_bits(u16::from(code), bits);
    if raw != PieceType::GOLD {
        writer.write_one_bit(false);
    }
    writer.write_one_bit(piece.color().raw() != 0);
}

fn write_piecebox_piece(writer: &mut BitWriter<'_>, piece_type: PieceType) {
    let idx = huffman_index(piece_type).expect("piecebox piece must be encodable");
    let code = HUFFMAN_TABLE_PIECEBOX[idx].code;
    let bits = HUFFMAN_TABLE_PIECEBOX[idx].bits;
    writer.write_n_bits(u16::from(code), bits);
    if piece_type != PieceType::GOLD {
        writer.write_one_bit(false);
    }
}

pub(super) fn read_board_piece_fast(reader: &mut BitReader<'_>) -> Result<Piece, PackedSfenError> {
    let prefix = reader.peek_n_bits_zero_padded(BOARD_DECODE_BITS);
    let entry = BOARD_DECODE_TABLE[usize::from(prefix)];
    reader.advance(entry.consumed_bits)?;
    Ok(entry.piece)
}

pub(super) fn read_hand_piece_fast(reader: &mut BitReader<'_>) -> Result<Piece, PackedSfenError> {
    let prefix = reader.peek_n_bits_zero_padded(HAND_DECODE_BITS);
    let entry = HAND_DECODE_TABLE[usize::from(prefix)];
    reader.advance(entry.consumed_bits)?;
    Ok(entry.piece)
}

pub(super) fn read_board_piece(reader: &mut BitReader<'_>) -> Result<Piece, PackedSfenError> {
    let mut code = 0u8;
    for bits in 1u8..=6 {
        code |= u8::from(reader.read_one_bit()?) << (bits - 1);
        for (index, entry) in HUFFMAN_TABLE.iter().enumerate() {
            if entry.code == code && entry.bits == bits {
                let piece_type = piece_type_from_huffman_index(index);
                if piece_type == PieceType::NONE {
                    return Ok(Piece::NONE);
                }
                let promoted =
                    if piece_type == PieceType::GOLD { false } else { reader.read_one_bit()? };
                let color = if reader.read_one_bit()? { Color::WHITE } else { Color::BLACK };
                let piece_type = if promoted { piece_type.promote() } else { piece_type };
                return Ok(Piece::from_parts(color, piece_type));
            }
        }
    }
    Err(PackedSfenError::InvalidPieceCode)
}

pub(super) fn read_hand_piece(reader: &mut BitReader<'_>) -> Result<Piece, PackedSfenError> {
    let mut code = 0u8;
    for bits in 1u8..=5 {
        code |= u8::from(reader.read_one_bit()?) << (bits - 1);
        for (index, entry) in HUFFMAN_TABLE.iter().enumerate().skip(1) {
            if entry.code >> 1 == code && entry.bits - 1 == bits {
                let piece_type = piece_type_from_huffman_index(index);
                let promoted =
                    if piece_type == PieceType::GOLD { false } else { reader.read_one_bit()? };
                let color = if reader.read_one_bit()? { Color::WHITE } else { Color::BLACK };
                let piece_type = if promoted { piece_type.promote() } else { piece_type };
                return Ok(Piece::from_parts(color, piece_type));
            }
        }
    }
    Err(PackedSfenError::InvalidPieceCode)
}
