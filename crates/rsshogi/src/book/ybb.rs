use std::fs;
use std::path::Path;

use crate::board::{PackedSfen, Ply, Position};
use crate::book::BookError;
use crate::types::{Move, Square};

const MAGIC: &[u8; 16] = b"YANE-BINBOOK-V1\0";
const HEADER_BYTES: usize = 32;
const INDEX_BYTES: usize = 44;

#[derive(Debug, Clone)]
pub struct YbbBook {
    bytes: Vec<u8>,
    rows: Vec<IndexRow>,
    move_area: usize,
    has_depth: bool,
    options: YbbBookOpenOptions,
}

#[derive(Debug, Clone, Copy)]
struct IndexRow {
    packed_sfen: PackedSfen,
    moves_offset: u64,
    ply: Ply,
    move_count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct YbbBookOpenOptions {
    ignore_ply: bool,
    flipped: bool,
}

impl YbbBookOpenOptions {
    #[must_use]
    pub const fn new() -> Self {
        Self { ignore_ply: false, flipped: false }
    }

    #[must_use]
    pub const fn with_ignore_ply(mut self, ignore_ply: bool) -> Self {
        self.ignore_ply = ignore_ply;
        self
    }

    #[must_use]
    pub const fn with_flipped(mut self, flipped: bool) -> Self {
        self.flipped = flipped;
        self
    }

    #[must_use]
    pub const fn ignore_ply(self) -> bool {
        self.ignore_ply
    }

    #[must_use]
    pub const fn flipped(self) -> bool {
        self.flipped
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YbbEntry {
    packed_sfen: PackedSfen,
    ply: Ply,
    flipped: bool,
    moves: Vec<YbbMove>,
}

impl YbbEntry {
    #[must_use]
    pub const fn packed_sfen(&self) -> PackedSfen {
        self.packed_sfen
    }

    #[must_use]
    pub const fn ply(&self) -> Ply {
        self.ply
    }

    #[must_use]
    pub const fn flipped(&self) -> bool {
        self.flipped
    }

    #[must_use]
    pub fn moves(&self) -> &[YbbMove] {
        &self.moves
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YbbMove {
    mv: Move,
    raw_move: u16,
    eval: i16,
    depth: Option<u16>,
}

impl YbbMove {
    #[must_use]
    pub const fn mv(self) -> Move {
        self.mv
    }

    #[must_use]
    pub const fn raw_move(self) -> u16 {
        self.raw_move
    }

    #[must_use]
    pub const fn eval(self) -> i16 {
        self.eval
    }

    #[must_use]
    pub const fn depth(self) -> Option<u16> {
        self.depth
    }
}

impl YbbBook {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, BookError> {
        Self::open_with_options(path, YbbBookOpenOptions::new())
    }

    pub fn open_with_options(
        path: impl AsRef<Path>,
        options: YbbBookOpenOptions,
    ) -> Result<Self, BookError> {
        let bytes = fs::read(path)?;
        if bytes.len() < HEADER_BYTES {
            return Err(BookError::InvalidFormat("truncated YBB header"));
        }
        if &bytes[..16] != MAGIC {
            return Err(BookError::InvalidFormat("invalid YBB magic"));
        }
        let count = read_u64(&bytes[16..24])?;
        let flags = read_u64(&bytes[24..32])?;
        if flags & !1 != 0 {
            return Err(BookError::Unsupported("unknown YBB header flags"));
        }
        let count_usize = usize::try_from(count)
            .map_err(|_| BookError::InvalidFormat("YBB record count overflows usize"))?;
        let index_bytes = count_usize
            .checked_mul(INDEX_BYTES)
            .ok_or(BookError::InvalidFormat("YBB index size overflow"))?;
        let move_area = HEADER_BYTES
            .checked_add(index_bytes)
            .ok_or(BookError::InvalidFormat("YBB index end overflow"))?;
        if move_area > bytes.len() {
            return Err(BookError::InvalidFormat("truncated YBB index"));
        }
        let has_depth = flags & 1 != 0;
        let move_record_bytes = if has_depth { 6usize } else { 4usize };
        let mut rows: Vec<IndexRow> = Vec::with_capacity(count_usize);
        for index in 0..count_usize {
            let start = HEADER_BYTES + index * INDEX_BYTES;
            let packed_sfen = PackedSfen::from_bytes(
                bytes[start..start + 32].try_into().expect("fixed YBB index slice"),
            );
            let moves_offset = read_u64(&bytes[start + 32..start + 40])?;
            let ply = read_u16(&bytes[start + 40..start + 42])?;
            let move_count = read_u16(&bytes[start + 42..start + 44])?;
            let byte_count = usize::from(move_count)
                .checked_mul(move_record_bytes)
                .ok_or(BookError::InvalidFormat("YBB move count overflow"))?;
            let offset = usize::try_from(moves_offset)
                .map_err(|_| BookError::InvalidFormat("YBB move offset overflows usize"))?;
            let end = offset
                .checked_add(byte_count)
                .ok_or(BookError::InvalidFormat("YBB move range overflow"))?;
            if end > bytes.len() - move_area {
                return Err(BookError::InvalidFormat("truncated YBB move data"));
            }
            let row = IndexRow { packed_sfen, moves_offset, ply, move_count };
            if let Some(previous) = rows.last()
                && previous.packed_sfen.cmp_bytes(&row.packed_sfen).is_gt()
            {
                return Err(BookError::InvalidFormat("YBB index is not sorted"));
            }
            rows.push(row);
        }
        let book = Self { bytes, rows, move_area, has_depth, options };
        for row in &book.rows {
            let _ = book.decode_moves(*row, false)?;
        }
        Ok(book)
    }

    #[must_use]
    pub const fn len(&self) -> u64 {
        self.rows.len() as u64
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    #[must_use]
    pub const fn has_depth(&self) -> bool {
        self.has_depth
    }

    #[must_use]
    pub const fn options(&self) -> YbbBookOpenOptions {
        self.options
    }

    pub fn lookup_position(&self, pos: &Position) -> Result<Option<YbbEntry>, BookError> {
        let packed = pos.to_packed_sfen();
        if let Some(entry) = self.lookup(packed, pos.game_ply(), false)? {
            return Ok(Some(entry));
        }
        if !self.options.flipped() {
            return Ok(None);
        }
        let flipped_sfen = pos.to_sfen_flipped(Some(i32::from(pos.game_ply())));
        let flipped = Position::from_sfen(&flipped_sfen).map_err(|error| {
            BookError::InvalidData(format!("cannot flip YBB position: {error}"))
        })?;
        self.lookup(flipped.to_packed_sfen(), pos.game_ply(), true)
    }

    pub fn lookup_packed_sfen(
        &self,
        packed_sfen: PackedSfen,
        ply: Ply,
    ) -> Result<Option<YbbEntry>, BookError> {
        if let Some(entry) = self.lookup(packed_sfen, ply, false)? {
            return Ok(Some(entry));
        }
        if !self.options.flipped() {
            return Ok(None);
        }
        let mut position = Position::empty();
        position.set_packed_sfen(&packed_sfen, false, ply).map_err(|error| {
            BookError::InvalidData(format!("invalid YBB PackedSfen lookup key: {error}"))
        })?;
        let flipped_sfen = position.to_sfen_flipped(Some(i32::from(ply)));
        let flipped = Position::from_sfen(&flipped_sfen).map_err(|error| {
            BookError::InvalidData(format!("cannot flip YBB lookup key: {error}"))
        })?;
        self.lookup(flipped.to_packed_sfen(), ply, true)
    }

    fn lookup(
        &self,
        packed_sfen: PackedSfen,
        ply: Ply,
        flipped: bool,
    ) -> Result<Option<YbbEntry>, BookError> {
        let Ok(mut index) =
            self.rows.binary_search_by(|row| row.packed_sfen.cmp_bytes(&packed_sfen))
        else {
            return Ok(None);
        };
        while index > 0 && self.rows[index - 1].packed_sfen == packed_sfen {
            index -= 1;
        }
        let end = self.rows[index..]
            .iter()
            .position(|row| row.packed_sfen != packed_sfen)
            .map_or(self.rows.len(), |relative| index + relative);
        let row = if self.options.ignore_ply() {
            self.rows[index]
        } else {
            let Some(row) = self.rows[index..end].iter().find(|row| row.ply == ply) else {
                return Ok(None);
            };
            *row
        };
        Ok(Some(YbbEntry {
            packed_sfen: row.packed_sfen,
            ply: row.ply,
            flipped,
            moves: self.decode_moves(row, flipped)?,
        }))
    }

    fn decode_moves(&self, row: IndexRow, flipped: bool) -> Result<Vec<YbbMove>, BookError> {
        let record_bytes = if self.has_depth { 6usize } else { 4usize };
        let offset = usize::try_from(row.moves_offset)
            .map_err(|_| BookError::InvalidFormat("YBB move offset overflows usize"))?;
        let start = self.move_area + offset;
        let mut moves = Vec::with_capacity(usize::from(row.move_count));
        for index in 0..usize::from(row.move_count) {
            let base = start + index * record_bytes;
            let raw_move = read_u16(&self.bytes[base..base + 2])?;
            if !is_valid_raw_move(raw_move) {
                return Err(BookError::InvalidData(format!(
                    "invalid YBB raw move: {raw_move:#06x}"
                )));
            }
            let eval = read_i16(&self.bytes[base + 2..base + 4])?;
            let depth = if self.has_depth {
                Some(read_u16(&self.bytes[base + 4..base + 6])?)
            } else {
                None
            };
            let mv = if flipped {
                flip_move(Move::from_raw(raw_move))
            } else {
                Move::from_raw(raw_move)
            };
            moves.push(YbbMove { mv, raw_move, eval, depth });
        }
        Ok(moves)
    }
}

fn is_valid_raw_move(raw: u16) -> bool {
    let mv = Move::from_raw(raw);
    if !mv.is_normal() || mv.to_sq().raw() < 0 || mv.to_sq().raw() >= Square::COUNT as i8 {
        return false;
    }
    if mv.is_drop() {
        !mv.is_promotion() && matches!((raw >> 7) & 0x7f, 1..=7)
    } else {
        mv.from_sq().raw() >= 0
            && mv.from_sq().raw() < Square::COUNT as i8
            && mv.from_sq() != mv.to_sq()
    }
}

fn flip_move(mv: Move) -> Move {
    if mv.is_drop() {
        Move::drop(mv.dropped_piece().expect("validated YBB drop"), mv.to_sq().flip())
    } else if mv.is_promotion() {
        Move::promotion(mv.from_sq().flip(), mv.to_sq().flip())
    } else {
        Move::normal(mv.from_sq().flip(), mv.to_sq().flip())
    }
}

fn read_u16(bytes: &[u8]) -> Result<u16, BookError> {
    Ok(u16::from_le_bytes(
        bytes.try_into().map_err(|_| BookError::InvalidFormat("truncated YBB u16"))?,
    ))
}

fn read_i16(bytes: &[u8]) -> Result<i16, BookError> {
    Ok(i16::from_le_bytes(
        bytes.try_into().map_err(|_| BookError::InvalidFormat("truncated YBB i16"))?,
    ))
}

fn read_u64(bytes: &[u8]) -> Result<u64, BookError> {
    Ok(u64::from_le_bytes(
        bytes.try_into().map_err(|_| BookError::InvalidFormat("truncated YBB u64"))?,
    ))
}
