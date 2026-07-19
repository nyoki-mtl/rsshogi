//! 読み取り専用 YBB 定跡 reader。

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::board::Position;
use crate::board::position::{PackedSfen, Ply};
use crate::book::BookError;
use crate::types::Move;

const MAGIC: &[u8; 16] = b"YANE-BINBOOK-V1\0";
const HEADER_SIZE: u64 = 32;
const INDEX_RECORD_SIZE: u64 = 44;
const MOVE_RECORD_SIZE_NO_DEPTH: u64 = 4;
const MOVE_RECORD_SIZE_WITH_DEPTH: u64 = 6;
const FLAG_HAS_DEPTH: u64 = 1 << 0;
const KNOWN_FLAGS: u64 = FLAG_HAS_DEPTH;

/// YBB ファイルを全展開せずに参照する読み取り専用 reader。
#[derive(Debug, Clone)]
pub struct YbbBook {
    path: PathBuf,
    file_bytes: u64,
    record_count: u64,
    flags: u64,
    moves_base: u64,
    options: YbbBookOpenOptions,
}

/// `.ybb` reader の open / lookup オプション。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct YbbBookOpenOptions {
    ignore_ply: bool,
    flipped: bool,
}

/// `.ybb` の 1 局面エントリ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YbbEntry {
    packed_sfen: PackedSfen,
    ply: Ply,
    flipped: bool,
    moves: Vec<YbbMove>,
}

/// `.ybb` に格納された 1 指し手。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YbbMove {
    mv: Move,
    eval: i16,
    depth: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IndexRecord {
    key: PackedSfen,
    moves_offset: u64,
    ply: Ply,
    move_count: u16,
}

impl YbbBook {
    /// YBB ファイルをデフォルトオプションで開く。
    pub fn open(path: impl AsRef<Path>) -> Result<Self, BookError> {
        Self::open_with_options(path, YbbBookOpenOptions::default())
    }

    /// YBB ファイルを指定オプションで開く。
    pub fn open_with_options(
        path: impl AsRef<Path>,
        options: YbbBookOpenOptions,
    ) -> Result<Self, BookError> {
        let path = path.as_ref().to_path_buf();
        let mut file = File::open(&path)?;
        let file_bytes = file.seek(SeekFrom::End(0))?;
        if file_bytes < HEADER_SIZE {
            return Err(BookError::InvalidFormat("truncated ybb header"));
        }

        file.seek(SeekFrom::Start(0))?;
        let mut header = [0u8; HEADER_SIZE as usize];
        file.read_exact(&mut header)?;
        if &header[..MAGIC.len()] != MAGIC {
            return Err(BookError::InvalidFormat("invalid ybb magic"));
        }

        let record_count = read_u64_le(&header[16..24]);
        let flags = read_u64_le(&header[24..32]);
        if flags & !KNOWN_FLAGS != 0 {
            return Err(BookError::Unsupported("unknown ybb flags"));
        }

        let index_bytes = record_count
            .checked_mul(INDEX_RECORD_SIZE)
            .ok_or(BookError::InvalidFormat("ybb index size overflow"))?;
        let moves_base = HEADER_SIZE
            .checked_add(index_bytes)
            .ok_or(BookError::InvalidFormat("ybb index size overflow"))?;
        if moves_base > file_bytes {
            return Err(BookError::InvalidFormat("truncated ybb index"));
        }

        Ok(Self { path, file_bytes, record_count, flags, moves_base, options })
    }

    /// インデックスレコード数を返す。
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.record_count
    }

    /// インデックスが空なら `true` を返す。
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.record_count == 0
    }

    /// move record に depth フィールドがあるなら `true` を返す。
    #[must_use]
    pub const fn has_depth(&self) -> bool {
        self.flags & FLAG_HAS_DEPTH != 0
    }

    /// open 時に指定された lookup オプションを返す。
    #[must_use]
    pub const fn options(&self) -> YbbBookOpenOptions {
        self.options
    }

    /// 局面オブジェクトで 1 局面を検索する。
    pub fn lookup_position(&self, pos: &Position) -> Result<Option<YbbEntry>, BookError> {
        let key = pos.to_packed_sfen();
        let ply = pos.game_ply();
        match self.lookup_exact(key, ply, false)? {
            Some(entry) => Ok(Some(entry)),
            None if self.options.flipped => {
                let flipped = flipped_position(pos)?;
                self.lookup_exact(flipped.to_packed_sfen(), flipped.game_ply(), true)
            }
            None => Ok(None),
        }
    }

    /// packed SFEN と手数で 1 局面を検索する。
    pub fn lookup_packed_sfen(
        &self,
        packed_sfen: PackedSfen,
        ply: Ply,
    ) -> Result<Option<YbbEntry>, BookError> {
        match self.lookup_exact(packed_sfen, ply, false)? {
            Some(entry) => Ok(Some(entry)),
            None if self.options.flipped => {
                let mut pos = Position::empty();
                pos.set_packed_sfen(&packed_sfen, false, ply).map_err(|err| {
                    BookError::InvalidData(format!("invalid packed SFEN: {err:?}"))
                })?;
                let flipped = flipped_position(&pos)?;
                self.lookup_exact(flipped.to_packed_sfen(), flipped.game_ply(), true)
            }
            None => Ok(None),
        }
    }

    fn lookup_exact(
        &self,
        key: PackedSfen,
        ply: Ply,
        flip_moves: bool,
    ) -> Result<Option<YbbEntry>, BookError> {
        let mut file = File::open(&self.path)?;
        let mut index = self.lower_bound(&mut file, &key)?;
        while index < self.record_count {
            let row = self.read_index_record(&mut file, index)?;
            match row.key.cmp_bytes(&key) {
                std::cmp::Ordering::Equal => {
                    if self.options.ignore_ply || row.ply == ply {
                        return self.read_entry(&mut file, row, flip_moves).map(Some);
                    }
                    index += 1;
                }
                _ => return Ok(None),
            }
        }
        Ok(None)
    }

    fn lower_bound(&self, file: &mut File, key: &PackedSfen) -> Result<u64, BookError> {
        let mut begin = 0u64;
        let mut end = self.record_count;
        while begin < end {
            let mid = begin + (end - begin) / 2;
            let row = self.read_index_record(file, mid)?;
            if row.key.cmp_bytes(key).is_lt() {
                begin = mid + 1;
            } else {
                end = mid;
            }
        }
        Ok(begin)
    }

    fn read_index_record(&self, file: &mut File, index: u64) -> Result<IndexRecord, BookError> {
        let relative = index
            .checked_mul(INDEX_RECORD_SIZE)
            .ok_or(BookError::InvalidFormat("ybb index offset overflow"))?;
        let offset = HEADER_SIZE
            .checked_add(relative)
            .ok_or(BookError::InvalidFormat("ybb index offset overflow"))?;
        let end = offset
            .checked_add(INDEX_RECORD_SIZE)
            .ok_or(BookError::InvalidFormat("ybb index offset overflow"))?;
        if end > self.moves_base {
            return Err(BookError::InvalidFormat("truncated ybb index"));
        }

        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = [0u8; INDEX_RECORD_SIZE as usize];
        file.read_exact(&mut bytes).map_err(map_unexpected_eof("truncated ybb index"))?;

        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes[..32]);
        Ok(IndexRecord {
            key: PackedSfen::from_bytes(key),
            moves_offset: read_u64_le(&bytes[32..40]),
            ply: read_u16_le(&bytes[40..42]),
            move_count: read_u16_le(&bytes[42..44]),
        })
    }

    fn read_entry(
        &self,
        file: &mut File,
        row: IndexRecord,
        flip_moves: bool,
    ) -> Result<YbbEntry, BookError> {
        let record_size = self.move_record_size();
        let moves_bytes = u64::from(row.move_count)
            .checked_mul(record_size)
            .ok_or(BookError::InvalidFormat("ybb moves size overflow"))?;
        let start = self
            .moves_base
            .checked_add(row.moves_offset)
            .ok_or(BookError::InvalidFormat("ybb moves offset overflow"))?;
        let end = start
            .checked_add(moves_bytes)
            .ok_or(BookError::InvalidFormat("ybb moves size overflow"))?;
        if end > self.file_bytes {
            return Err(BookError::InvalidFormat("truncated ybb moves"));
        }

        file.seek(SeekFrom::Start(start))?;
        let mut bytes = vec![
            0u8;
            usize::try_from(moves_bytes).map_err(|_| BookError::InvalidFormat(
                "ybb moves size is too large"
            ))?
        ];
        file.read_exact(&mut bytes).map_err(map_unexpected_eof("truncated ybb moves"))?;

        let step = usize::try_from(record_size).expect("small record size");
        let moves = bytes
            .chunks_exact(step)
            .map(|chunk| {
                let raw = read_u16_le(&chunk[0..2]);
                let mv = Move::from_raw(raw);
                if !mv.is_normal() {
                    return Err(BookError::InvalidData(format!(
                        "invalid ybb move raw value: 0x{raw:04x}"
                    )));
                }
                let mv = if flip_moves { flip_move(mv)? } else { mv };
                let eval = read_i16_le(&chunk[2..4]);
                let depth = if self.has_depth() { Some(read_u16_le(&chunk[4..6])) } else { None };
                Ok(YbbMove { mv, eval, depth })
            })
            .collect::<Result<Vec<_>, BookError>>()?;

        Ok(YbbEntry { packed_sfen: row.key, ply: row.ply, flipped: flip_moves, moves })
    }

    const fn move_record_size(&self) -> u64 {
        if self.has_depth() { MOVE_RECORD_SIZE_WITH_DEPTH } else { MOVE_RECORD_SIZE_NO_DEPTH }
    }
}

impl YbbBookOpenOptions {
    /// デフォルトオプションを生成する。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 手数不一致を無視するかどうかを設定する。
    #[must_use]
    pub const fn with_ignore_ply(self, ignore_ply: bool) -> Self {
        Self { ignore_ply, ..self }
    }

    /// 通常 lookup miss 後に先後反転局面も検索するかどうかを設定する。
    #[must_use]
    pub const fn with_flipped(self, flipped: bool) -> Self {
        Self { flipped, ..self }
    }

    /// 手数不一致を無視するなら `true`。
    #[must_use]
    pub const fn ignore_ply(self) -> bool {
        self.ignore_ply
    }

    /// 先後反転 lookup を有効化しているなら `true`。
    #[must_use]
    pub const fn flipped(self) -> bool {
        self.flipped
    }
}

impl YbbEntry {
    /// index record の packed SFEN を返す。
    #[must_use]
    pub const fn packed_sfen(&self) -> PackedSfen {
        self.packed_sfen
    }

    /// index record の手数を返す。
    #[must_use]
    pub const fn ply(&self) -> Ply {
        self.ply
    }

    /// 先後反転 lookup で得たエントリなら `true`。
    #[must_use]
    pub const fn flipped(&self) -> bool {
        self.flipped
    }

    /// 候補手一覧を返す。
    #[must_use]
    pub fn moves(&self) -> &[YbbMove] {
        &self.moves
    }
}

impl YbbMove {
    /// 指し手を返す。
    #[must_use]
    pub const fn mv(self) -> Move {
        self.mv
    }

    /// 返却済みの指し手視点で YBB の Move16 raw 値を返す。
    ///
    /// flipped lookup で得た手は元局面視点へ反転した後の値になる。
    #[must_use]
    pub const fn raw_move(self) -> u16 {
        self.mv.raw()
    }

    /// 評価値を返す。
    #[must_use]
    pub const fn eval(self) -> i16 {
        self.eval
    }

    /// depth が存在する場合は返す。
    #[must_use]
    pub const fn depth(self) -> Option<u16> {
        self.depth
    }
}

fn flipped_position(pos: &Position) -> Result<Position, BookError> {
    Position::from_sfen(&pos.to_sfen_flipped(None))
        .map_err(|err| BookError::InvalidData(format!("invalid flipped position: {err}")))
}

fn flip_move(mv: Move) -> Result<Move, BookError> {
    if !mv.is_normal() {
        return Ok(mv);
    }
    let to = mv.to_sq().flip();
    if mv.is_drop() {
        let piece_type = mv.dropped_piece().ok_or_else(|| {
            BookError::InvalidData(format!("invalid ybb drop move: 0x{:04x}", mv.raw()))
        })?;
        Ok(Move::drop(piece_type, to))
    } else {
        let from = mv.from_sq().flip();
        Ok(if mv.is_promotion() { Move::promotion(from, to) } else { Move::normal(from, to) })
    }
}

fn read_u16_le(bytes: &[u8]) -> u16 {
    u16::from_le_bytes(bytes.try_into().expect("u16 field length"))
}

fn read_i16_le(bytes: &[u8]) -> i16 {
    i16::from_le_bytes(bytes.try_into().expect("i16 field length"))
}

fn read_u64_le(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().expect("u64 field length"))
}

fn map_unexpected_eof(message: &'static str) -> impl FnOnce(std::io::Error) -> BookError {
    move |err| {
        if err.kind() == std::io::ErrorKind::UnexpectedEof {
            BookError::InvalidFormat(message)
        } else {
            BookError::Io(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::board::{self, InitialPosition};

    static TEMP_BOOK_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn test_ybb_lookup_without_depth() {
        board::init();
        let pos = hirate();
        let mv = Move::from_usi("7g7f").unwrap();
        let drop = Move::from_usi("P*5e").unwrap();
        let path = write_fixture(
            "no-depth",
            0,
            vec![fixture_record(
                pos.to_packed_sfen(),
                pos.game_ply(),
                vec![fixture_move(mv, 12, None), fixture_move(drop, -3, None)],
            )],
        );

        let book = YbbBook::open(&path).unwrap();
        let entry = book.lookup_position(&pos).unwrap().unwrap();

        assert_eq!(book.len(), 1);
        assert!(!book.has_depth());
        assert_eq!(entry.ply(), pos.game_ply());
        assert!(!entry.flipped());
        assert_eq!(entry.moves()[0].mv(), mv);
        assert_eq!(entry.moves()[0].raw_move(), mv.raw());
        assert_eq!(entry.moves()[0].eval(), 12);
        assert_eq!(entry.moves()[0].depth(), None);
        assert_eq!(entry.moves()[1].mv(), drop);
        assert_eq!(entry.moves()[1].raw_move(), drop.raw());
        assert_eq!(entry.moves()[1].eval(), -3);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_ybb_reads_native_move16_for_promotion_and_drop() {
        board::init();
        let pos = hirate();
        let promote = Move::from_usi("8h2b+").unwrap();
        let drop = Move::from_usi("P*5e").unwrap();
        assert_ne!(promote.raw(), promote.to_apery().raw());
        assert_ne!(drop.raw(), drop.to_apery().raw());
        let path = write_fixture(
            "native-move16",
            0,
            vec![fixture_record(
                pos.to_packed_sfen(),
                pos.game_ply(),
                vec![fixture_move(promote, 21, None), fixture_move(drop, -4, None)],
            )],
        );

        let book = YbbBook::open(&path).unwrap();
        let entry = book.lookup_position(&pos).unwrap().unwrap();

        assert_eq!(entry.moves()[0].mv(), promote);
        assert_eq!(entry.moves()[0].raw_move(), promote.raw());
        assert_eq!(entry.moves()[0].mv().to_usi(), "8h2b+");
        assert_eq!(entry.moves()[0].eval(), 21);
        assert_eq!(entry.moves()[1].mv(), drop);
        assert_eq!(entry.moves()[1].raw_move(), drop.raw());
        assert_eq!(entry.moves()[1].mv().to_usi(), "P*5e");
        assert_eq!(entry.moves()[1].eval(), -4);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_ybb_lookup_with_depth() {
        board::init();
        let pos = hirate();
        let mv = Move::from_usi("2g2f").unwrap();
        let path = write_fixture(
            "with-depth",
            FLAG_HAS_DEPTH,
            vec![fixture_record(
                pos.to_packed_sfen(),
                pos.game_ply(),
                vec![fixture_move(mv, -23, Some(34))],
            )],
        );

        let book = YbbBook::open(&path).unwrap();
        let entry = book.lookup_position(&pos).unwrap().unwrap();

        assert!(book.has_depth());
        assert_eq!(entry.moves()[0].mv(), mv);
        assert_eq!(entry.moves()[0].eval(), -23);
        assert_eq!(entry.moves()[0].depth(), Some(34));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_ybb_lookup_miss() {
        board::init();
        let pos = hirate();
        let mut other = pos.clone();
        other.apply_move(Move::from_usi("7g7f").unwrap());
        let path = write_fixture(
            "miss",
            0,
            vec![fixture_record(
                pos.to_packed_sfen(),
                pos.game_ply(),
                vec![fixture_move(Move::from_usi("7g7f").unwrap(), 1, None)],
            )],
        );

        let book = YbbBook::open(&path).unwrap();

        assert!(book.lookup_position(&other).unwrap().is_none());

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_ybb_lookup_rejects_ply_mismatch_by_default() {
        board::init();
        let pos = hirate();
        let path = write_fixture(
            "ply-mismatch",
            0,
            vec![fixture_record(
                pos.to_packed_sfen(),
                pos.game_ply() + 2,
                vec![fixture_move(Move::from_usi("7g7f").unwrap(), 1, None)],
            )],
        );

        let book = YbbBook::open(&path).unwrap();

        assert!(book.lookup_position(&pos).unwrap().is_none());

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_ybb_lookup_ignore_ply_hits() {
        board::init();
        let pos = hirate();
        let mv = Move::from_usi("7g7f").unwrap();
        let path = write_fixture(
            "ignore-ply",
            0,
            vec![fixture_record(
                pos.to_packed_sfen(),
                pos.game_ply() + 2,
                vec![fixture_move(mv, 1, None)],
            )],
        );

        let book =
            YbbBook::open_with_options(&path, YbbBookOpenOptions::new().with_ignore_ply(true))
                .unwrap();
        let entry = book.lookup_position(&pos).unwrap().unwrap();

        assert_eq!(entry.moves()[0].mv(), mv);
        assert_eq!(entry.ply(), pos.game_ply() + 2);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_ybb_flipped_lookup_flips_move_back() {
        board::init();
        let pos = hirate();
        let flipped = flipped_position(&pos).unwrap();
        let path = write_fixture(
            "flipped",
            0,
            vec![fixture_record(
                flipped.to_packed_sfen(),
                flipped.game_ply(),
                vec![fixture_move(Move::from_usi("3c3d").unwrap(), 7, None)],
            )],
        );

        let book = YbbBook::open_with_options(&path, YbbBookOpenOptions::new().with_flipped(true))
            .unwrap();
        let entry = book.lookup_position(&pos).unwrap().unwrap();

        assert!(entry.flipped());
        assert_eq!(entry.moves()[0].mv().to_usi(), "7g7f");
        assert_eq!(entry.moves()[0].eval(), 7);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_ybb_open_rejects_invalid_magic() {
        let path = temp_path("invalid-magic");
        let mut bytes = Vec::new();
        bytes.extend(*b"NOT-YANE-BOOK-V1");
        bytes.extend(0u64.to_le_bytes());
        bytes.extend(0u64.to_le_bytes());
        fs::write(&path, bytes).unwrap();

        assert!(matches!(YbbBook::open(&path), Err(BookError::InvalidFormat("invalid ybb magic"))));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_ybb_open_rejects_unknown_flags() {
        let path = write_fixture("unknown-flags", 1 << 1, Vec::new());

        assert!(matches!(YbbBook::open(&path), Err(BookError::Unsupported("unknown ybb flags"))));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_ybb_open_rejects_truncated_index() {
        let path = temp_path("truncated-index");
        let mut bytes = Vec::new();
        bytes.extend(*MAGIC);
        bytes.extend(1u64.to_le_bytes());
        bytes.extend(0u64.to_le_bytes());
        fs::write(&path, bytes).unwrap();

        assert!(matches!(
            YbbBook::open(&path),
            Err(BookError::InvalidFormat("truncated ybb index"))
        ));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_ybb_lookup_rejects_truncated_moves() {
        board::init();
        let pos = hirate();
        let path = temp_path("truncated-moves");
        let mut bytes = Vec::new();
        bytes.extend(*MAGIC);
        bytes.extend(1u64.to_le_bytes());
        bytes.extend(0u64.to_le_bytes());
        bytes.extend(*pos.to_packed_sfen().as_bytes());
        bytes.extend(0u64.to_le_bytes());
        bytes.extend(pos.game_ply().to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend([0u8; 2]);
        fs::write(&path, bytes).unwrap();

        let book = YbbBook::open(&path).unwrap();

        assert!(matches!(
            book.lookup_position(&pos),
            Err(BookError::InvalidFormat("truncated ybb moves"))
        ));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_ybb_lookup_uses_byte_order_not_sbk_word_order() {
        let byte_less_sbk_greater =
            PackedSfen::from_le_u32_words([0x0000_0100, 0, 0, 0, 0, 0, 0, 0]);
        let byte_greater_sbk_less =
            PackedSfen::from_le_u32_words([0x0000_00ff, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            byte_less_sbk_greater.cmp_sbk_words(&byte_greater_sbk_less),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            byte_less_sbk_greater.cmp_bytes(&byte_greater_sbk_less),
            std::cmp::Ordering::Less
        );

        let mv = Move::from_usi("7g7f").unwrap();
        let path = write_fixture(
            "byte-order",
            0,
            vec![
                fixture_record(byte_less_sbk_greater, 1, vec![fixture_move(mv, 101, None)]),
                fixture_record(
                    byte_greater_sbk_less,
                    1,
                    vec![fixture_move(Move::from_usi("2g2f").unwrap(), 202, None)],
                ),
            ],
        );

        let book = YbbBook::open(&path).unwrap();
        let entry = book.lookup_packed_sfen(byte_less_sbk_greater, 1).unwrap().unwrap();

        assert_eq!(entry.moves()[0].mv(), mv);
        assert_eq!(entry.moves()[0].eval(), 101);

        fs::remove_file(path).unwrap();
    }

    #[derive(Debug, Clone)]
    struct FixtureRecord {
        key: PackedSfen,
        ply: Ply,
        moves: Vec<FixtureMove>,
    }

    #[derive(Debug, Clone, Copy)]
    struct FixtureMove {
        mv: Move,
        eval: i16,
        depth: Option<u16>,
    }

    fn hirate() -> Position {
        Position::from_sfen(InitialPosition::Standard.to_sfen()).unwrap()
    }

    fn fixture_record(key: PackedSfen, ply: Ply, moves: Vec<FixtureMove>) -> FixtureRecord {
        FixtureRecord { key, ply, moves }
    }

    const fn fixture_move(mv: Move, eval: i16, depth: Option<u16>) -> FixtureMove {
        FixtureMove { mv, eval, depth }
    }

    fn write_fixture(name: &str, flags: u64, mut records: Vec<FixtureRecord>) -> PathBuf {
        records.sort_by(|lhs, rhs| lhs.key.cmp_bytes(&rhs.key));

        let path = temp_path(name);
        let has_depth = flags & FLAG_HAS_DEPTH != 0;
        let record_size =
            if has_depth { MOVE_RECORD_SIZE_WITH_DEPTH } else { MOVE_RECORD_SIZE_NO_DEPTH };
        let mut bytes = Vec::new();
        bytes.extend(*MAGIC);
        bytes.extend((records.len() as u64).to_le_bytes());
        bytes.extend(flags.to_le_bytes());

        let mut moves = Vec::new();
        let mut index = Vec::new();
        for record in records {
            index.extend(*record.key.as_bytes());
            index.extend((moves.len() as u64).to_le_bytes());
            index.extend(record.ply.to_le_bytes());
            index.extend((record.moves.len() as u16).to_le_bytes());
            for book_move in record.moves {
                moves.extend(book_move.mv.raw().to_le_bytes());
                moves.extend(book_move.eval.to_le_bytes());
                if has_depth {
                    moves.extend(book_move.depth.unwrap_or_default().to_le_bytes());
                }
            }
            debug_assert_eq!(moves.len() as u64 % record_size, 0);
        }

        bytes.extend(index);
        bytes.extend(moves);
        fs::write(&path, bytes).unwrap();
        path
    }

    fn temp_path(name: &str) -> PathBuf {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let seq = TEMP_BOOK_COUNTER.fetch_add(1, AtomicOrdering::Relaxed);
        std::env::temp_dir()
            .join(format!("rsshogi-ybb-{name}-{}-{stamp}-{seq}.ybb", std::process::id()))
    }
}
