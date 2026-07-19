use std::fs;
use std::path::Path;

use super::{Book, BookEntry, BookError, BookKey, BookMove, MemoryBook};
use crate::board::zobrist::ZobristKey;

const MAGIC: [u8; 8] = *b"RSHOGIBK";
const VERSION: u16 = 1;
const FLAG_HAS_SCORE: u16 = 1 << 0;
const FLAG_HAS_DEPTH: u16 = 1 << 1;

/// ソート済みキーと指し手オフセットを持つ、コンパクトな不変定跡データ構造。
///
/// [`MemoryBook`] をこの形式に変換することで、高速な検索とコンパクトなバイナリ直列化を実現する。
#[derive(Debug, Clone)]
pub struct StaticBook {
    keys: Vec<BookKey>,
    offsets: Vec<u32>,
    moves: Vec<BookMove>,
}

impl StaticBook {
    /// 検証を行わずに `StaticBook` を生成する。
    ///
    /// 新規コードでは、不変条件を検証する [`StaticBook::try_new`] を優先する。
    #[must_use]
    pub fn new(keys: Vec<BookKey>, offsets: Vec<u32>, moves: Vec<BookMove>) -> Self {
        Self { keys, offsets, moves }
    }

    /// 検証済みの `StaticBook` を生成する。
    ///
    /// `keys` は strictly increasing、`offsets` は `keys.len() + 1` 要素で、
    /// 先頭が 0、末尾が `moves.len()`、かつ非減少でなければならない。
    pub fn try_new(
        keys: Vec<BookKey>,
        offsets: Vec<u32>,
        moves: Vec<BookMove>,
    ) -> Result<Self, BookError> {
        validate_keys(&keys)?;
        validate_offsets(&offsets, keys.len(), moves.len())?;
        Ok(Self { keys, offsets, moves })
    }

    #[must_use]
    pub fn keys(&self) -> &[BookKey] {
        &self.keys
    }

    #[must_use]
    pub fn moves(&self) -> &[BookMove] {
        &self.moves
    }

    #[must_use]
    pub fn offsets(&self) -> &[u32] {
        &self.offsets
    }

    #[must_use]
    pub fn from_memory_book(book: &MemoryBook) -> Self {
        let mut entries: Vec<(BookKey, &[BookMove])> =
            book.entries().iter().map(|(key, moves)| (*key, moves.as_slice())).collect();

        entries.sort_by_key(|(lhs, _)| key_parts(*lhs));

        let mut keys = Vec::with_capacity(entries.len());
        let mut offsets = Vec::with_capacity(entries.len() + 1);
        let mut moves = Vec::new();

        offsets.push(0);
        for (key, entry_moves) in entries {
            keys.push(key);
            moves.extend_from_slice(entry_moves);
            offsets.push(u32::try_from(moves.len()).expect("move count fits in u32"));
        }

        Self { keys, offsets, moves }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BookError> {
        let mut cursor = 0usize;

        let magic = read_bytes(bytes, &mut cursor, MAGIC.len())?;
        if magic != MAGIC {
            return Err(BookError::InvalidFormat("magic mismatch"));
        }

        let version = read_u16(bytes, &mut cursor)?;
        if version != VERSION {
            return Err(BookError::Unsupported("version mismatch"));
        }

        let flags = read_u16(bytes, &mut cursor)?;
        if flags & FLAG_HAS_SCORE == 0 || flags & FLAG_HAS_DEPTH == 0 {
            return Err(BookError::Unsupported("score/depth is required"));
        }

        let key_bytes = read_u8(bytes, &mut cursor)?;
        let _reserved = read_u8(bytes, &mut cursor)?;
        validate_key_bytes(key_bytes)?;

        let node_count = read_u32(bytes, &mut cursor)? as usize;
        let move_count = read_u32(bytes, &mut cursor)? as usize;

        let mut keys = Vec::with_capacity(node_count);
        for _ in 0..node_count {
            let key = match key_bytes {
                8 => {
                    let low = read_u64(bytes, &mut cursor)?;
                    ZobristKey::from_u64(low)
                }
                16 => {
                    let low = read_u64(bytes, &mut cursor)?;
                    let high = read_u64(bytes, &mut cursor)?;
                    ZobristKey::new(low, high)
                }
                _ => unreachable!("key width was validated"),
            };
            keys.push(key);
        }
        validate_keys(&keys)?;

        let mut offsets = Vec::with_capacity(node_count + 1);
        for _ in 0..=node_count {
            offsets.push(read_u32(bytes, &mut cursor)?);
        }

        validate_offsets(&offsets, node_count, move_count)?;

        let mut moves = Vec::with_capacity(move_count);
        for _ in 0..move_count {
            let mv = read_u16(bytes, &mut cursor)?;
            let score = read_i16(bytes, &mut cursor)?;
            let depth = read_u16(bytes, &mut cursor)?;
            moves.push(BookMove::new(crate::types::Move::from_raw(mv), score, depth));
        }

        if cursor != bytes.len() {
            return Err(BookError::InvalidFormat("trailing bytes"));
        }

        Ok(Self { keys, offsets, moves })
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, BookError> {
        let bytes = fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// この定跡を rsshogi static-book バイナリ形式に直列化する。
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let key_bytes = default_key_bytes();
        let flags = FLAG_HAS_SCORE | FLAG_HAS_DEPTH;

        let mut out = Vec::new();
        out.extend_from_slice(&MAGIC);
        write_u16(&mut out, VERSION);
        write_u16(&mut out, flags);
        write_u8(&mut out, key_bytes);
        write_u8(&mut out, 0);
        write_u32(&mut out, u32::try_from(self.keys.len()).expect("node count fits in u32"));
        write_u32(&mut out, u32::try_from(self.moves.len()).expect("move count fits in u32"));

        for key in &self.keys {
            match key_bytes {
                8 => write_u64(&mut out, key.low_u64()),
                16 => {
                    write_u64(&mut out, key.low_u64());
                    write_u64(&mut out, key.high_u64());
                }
                _ => unreachable!("invalid key size"),
            }
        }

        for offset in &self.offsets {
            write_u32(&mut out, *offset);
        }

        for book_move in &self.moves {
            write_u16(&mut out, book_move.mv().raw());
            write_i16(&mut out, book_move.score());
            write_u16(&mut out, book_move.depth());
        }

        out
    }

    /// この定跡を rsshogi static-book バイナリ形式でディスクに書き出す。
    pub fn write_file(&self, path: impl AsRef<Path>) -> Result<(), BookError> {
        fs::write(path, self.to_bytes())?;
        Ok(())
    }
}

impl Book for StaticBook {
    fn get(&self, key: BookKey) -> Option<BookEntry<'_>> {
        let target = key_parts(key);
        let index = self.keys.binary_search_by(|probe| key_parts(*probe).cmp(&target)).ok()?;

        let stored_key = *self.keys.get(index)?;
        let start = usize::try_from(*self.offsets.get(index)?).ok()?;
        let end = usize::try_from(*self.offsets.get(index.checked_add(1)?)?).ok()?;
        let moves = self.moves.get(start..end)?;
        Some(BookEntry::new(stored_key, moves))
    }

    fn len(&self) -> usize {
        self.keys.len()
    }
}

fn key_parts(key: BookKey) -> (u64, u64) {
    (key.low_u64(), key.high_u64())
}

fn default_key_bytes() -> u8 {
    #[cfg(feature = "hash-128")]
    {
        16
    }
    #[cfg(not(feature = "hash-128"))]
    {
        8
    }
}

fn validate_key_bytes(key_bytes: u8) -> Result<(), BookError> {
    match key_bytes {
        8 | 16 if key_bytes == default_key_bytes() => Ok(()),
        8 | 16 => {
            Err(BookError::Unsupported("static book key width does not match current hash feature"))
        }
        _ => Err(BookError::Unsupported("invalid key size")),
    }
}

fn validate_keys(keys: &[BookKey]) -> Result<(), BookError> {
    for pair in keys.windows(2) {
        if key_parts(pair[0]) >= key_parts(pair[1]) {
            return Err(BookError::InvalidFormat("keys must be strictly increasing"));
        }
    }
    Ok(())
}

fn validate_offsets(offsets: &[u32], key_count: usize, move_count: usize) -> Result<(), BookError> {
    if offsets.len() != key_count + 1 {
        return Err(BookError::InvalidFormat("offset count must be key count + 1"));
    }
    if offsets[0] != 0 {
        return Err(BookError::InvalidFormat("first offset must be zero"));
    }
    let mut prev = 0u32;
    for (index, &offset) in offsets.iter().enumerate() {
        if offset < prev {
            return Err(BookError::InvalidFormat("offsets must be non-decreasing"));
        }
        if index == offsets.len() - 1 && offset as usize != move_count {
            return Err(BookError::InvalidFormat("last offset must match move count"));
        }
        prev = offset;
    }
    Ok(())
}

fn read_bytes<'a>(bytes: &'a [u8], cursor: &mut usize, len: usize) -> Result<&'a [u8], BookError> {
    let end = cursor.checked_add(len).ok_or(BookError::InvalidFormat("overflow"))?;
    if end > bytes.len() {
        return Err(BookError::InvalidFormat("unexpected eof"));
    }
    let out = &bytes[*cursor..end];
    *cursor = end;
    Ok(out)
}

fn read_u8(bytes: &[u8], cursor: &mut usize) -> Result<u8, BookError> {
    let buf = read_bytes(bytes, cursor, 1)?;
    Ok(buf[0])
}

fn read_u16(bytes: &[u8], cursor: &mut usize) -> Result<u16, BookError> {
    let buf = read_bytes(bytes, cursor, 2)?;
    Ok(u16::from_le_bytes([buf[0], buf[1]]))
}

fn read_i16(bytes: &[u8], cursor: &mut usize) -> Result<i16, BookError> {
    let buf = read_bytes(bytes, cursor, 2)?;
    Ok(i16::from_le_bytes([buf[0], buf[1]]))
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, BookError> {
    let buf = read_bytes(bytes, cursor, 4)?;
    Ok(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]))
}

fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, BookError> {
    let buf = read_bytes(bytes, cursor, 8)?;
    Ok(u64::from_le_bytes([buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7]]))
}

fn write_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_i16(out: &mut Vec<u8>, value: i16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Move;

    fn key(low: u64) -> BookKey {
        ZobristKey::from_u64(low)
    }

    fn book_move() -> BookMove {
        BookMove::new(Move::from_usi("7g7f").expect("move"), 12, 4)
    }

    #[test]
    fn test_static_book_try_new_accepts_valid_parts() {
        let book = StaticBook::try_new(vec![key(1), key(2)], vec![0, 1, 1], vec![book_move()])
            .expect("valid static book");

        let entry = book.get(key(1)).expect("entry");
        assert_eq!(entry.moves().len(), 1);
        assert_eq!(book.get(key(2)).expect("empty entry").moves().len(), 0);
    }

    #[test]
    fn test_static_book_try_new_rejects_invalid_offsets() {
        assert!(matches!(
            StaticBook::try_new(vec![key(1)], vec![0], vec![]),
            Err(BookError::InvalidFormat("offset count must be key count + 1"))
        ));
        assert!(matches!(
            StaticBook::try_new(vec![key(1)], vec![1, 1], vec![book_move()]),
            Err(BookError::InvalidFormat("first offset must be zero"))
        ));
        assert!(matches!(
            StaticBook::try_new(vec![key(1)], vec![0, 2], vec![book_move()]),
            Err(BookError::InvalidFormat("last offset must match move count"))
        ));
    }

    #[test]
    fn test_static_book_try_new_rejects_unsorted_or_duplicate_keys() {
        assert!(matches!(
            StaticBook::try_new(vec![key(2), key(1)], vec![0, 0, 0], vec![]),
            Err(BookError::InvalidFormat("keys must be strictly increasing"))
        ));
        assert!(matches!(
            StaticBook::try_new(vec![key(1), key(1)], vec![0, 0, 0], vec![]),
            Err(BookError::InvalidFormat("keys must be strictly increasing"))
        ));
    }

    #[test]
    fn test_static_book_get_does_not_panic_on_invalid_new_offsets() {
        let book = StaticBook::new(vec![key(1)], vec![0], vec![book_move()]);

        assert!(book.get(key(1)).is_none());
    }

    #[test]
    fn test_static_book_from_bytes_rejects_mismatched_key_width() {
        let book = StaticBook::try_new(vec![], vec![0], vec![]).expect("empty static book");
        let mut bytes = book.to_bytes();
        bytes[12] = if default_key_bytes() == 8 { 16 } else { 8 };

        assert!(matches!(
            StaticBook::from_bytes(&bytes),
            Err(BookError::Unsupported(message))
                if message.contains("key width does not match current hash feature")
        ));
    }
}
