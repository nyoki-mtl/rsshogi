use std::borrow::Cow;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::board::{PackedSfen, Ply, Position};
use crate::book::{
    BookControl, BookDatabase, BookDatabaseEntry, BookEntryMetadata, BookError, BookMoveMetadata,
};
use crate::types::{Move, Square};

mod patch;

const MAGIC: &[u8; 16] = b"YANE-BINBOOK-V1\0";
const HEADER_BYTES: usize = 32;
const INDEX_BYTES: usize = 44;
const SCAN_BUFFER_BYTES: usize = 64 * 1024;
const WRITE_BUFFER_BYTES: usize = 64 * 1024;

/// YBB の全件検査で得た行数と重複の集計。
///
/// 同一キーの手数違いも重複局面に含む。候補手内容の一致や合法性は表さない。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct YbbDiagnostics {
    row_count: u64,
    position_count: u64,
    duplicate_position_count: u64,
    duplicate_ply_row_count: u64,
    varying_ply_position_count: u64,
}

impl YbbDiagnostics {
    /// 全索引行数を返す。
    #[must_use]
    pub const fn row_count(&self) -> u64 {
        self.row_count
    }

    /// 異なる局面キーの数を返す。
    #[must_use]
    pub const fn position_count(&self) -> u64 {
        self.position_count
    }

    /// 2 行以上を持つ局面キーの数を返す。
    #[must_use]
    pub const fn duplicate_position_count(&self) -> u64 {
        self.duplicate_position_count
    }

    /// 各局面キーの先頭以外の行数を返す。
    #[must_use]
    pub const fn duplicate_row_count(&self) -> u64 {
        self.row_count - self.position_count
    }

    /// 同じキーと手数を持つ行の、先頭以外の行数を返す。
    #[must_use]
    pub const fn duplicate_ply_row_count(&self) -> u64 {
        self.duplicate_ply_row_count
    }

    /// 複数種類の手数を持つ局面キーの数を返す。
    #[must_use]
    pub const fn varying_ply_position_count(&self) -> u64 {
        self.varying_ply_position_count
    }
}

/// YBB のキー順索引と候補手を参照する reader。
///
/// 既定ではファイル全体を検査してメモリに保持する。
/// [`YbbStorageMode::File`] を指定するとファイルを保持して部分読み込みする。
/// ファイル参照時の clone は同じハンドルと排他的な読み込みカーソルを共有する。
#[derive(Debug, Clone)]
pub struct YbbBook {
    storage: YbbStorage,
    move_area: u64,
    has_depth: bool,
    options: YbbBookOpenOptions,
}

#[derive(Debug, Clone)]
enum YbbStorage {
    Memory { bytes: Vec<u8>, rows: Vec<IndexRow> },
    File { file: Arc<Mutex<File>>, file_bytes: u64, row_count: usize },
}

#[derive(Debug, Clone, Copy)]
struct IndexRow {
    packed_sfen: PackedSfen,
    moves_offset: u64,
    ply: Ply,
    move_count: u16,
}

/// YBB のデータ保持方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum YbbStorageMode {
    /// ファイル全体と復号した索引をメモリに保持する。
    #[default]
    Memory,
    /// ファイルを保持し、参照した索引行と候補手だけを読み込む。
    ///
    /// reader とその clone の利用中は、ファイルを変更しないこと。
    File,
}

/// YBB の open 時と参照時の検査方針。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum YbbValidationMode {
    /// open 時に全索引の整列、各行の範囲、全候補手の符号化を検査する。
    #[default]
    Full,
    /// 各行の範囲と候補手の符号化を参照時に検査する。
    ///
    /// 検索には整列済み索引が必要である。全索引の整列は
    /// [`YbbBook::validate_full`] または全件走査で検査する。
    OnAccess,
}

/// YBB の検索、データ保持方式、検査方針の設定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct YbbBookOpenOptions {
    ignore_ply: bool,
    flipped: bool,
    storage_mode: YbbStorageMode,
    validation_mode: YbbValidationMode,
}

impl YbbBookOpenOptions {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ignore_ply: false,
            flipped: false,
            storage_mode: YbbStorageMode::Memory,
            validation_mode: YbbValidationMode::Full,
        }
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

    /// データ保持方式を指定する。検査方針は変更しない。
    #[must_use]
    pub const fn with_storage_mode(mut self, storage_mode: YbbStorageMode) -> Self {
        self.storage_mode = storage_mode;
        self
    }

    /// 検査方針を指定する。データ保持方式は変更しない。
    #[must_use]
    pub const fn with_validation_mode(mut self, validation_mode: YbbValidationMode) -> Self {
        self.validation_mode = validation_mode;
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

    #[must_use]
    pub const fn storage_mode(self) -> YbbStorageMode {
        self.storage_mode
    }

    #[must_use]
    pub const fn validation_mode(self) -> YbbValidationMode {
        self.validation_mode
    }
}

/// YBB の全件検査の進捗。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YbbValidationProgress {
    processed_row_count: u64,
    total_row_count: u64,
}

impl YbbValidationProgress {
    /// 検査済みの索引行数を返す。
    #[must_use]
    pub const fn processed_row_count(self) -> u64 {
        self.processed_row_count
    }

    /// 検査対象の索引行数を返す。
    #[must_use]
    pub const fn total_row_count(self) -> u64 {
        self.total_row_count
    }
}

/// 保存時のキーと手数、候補手を持つ YBB の 1 行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YbbEntry {
    packed_sfen: PackedSfen,
    ply: Ply,
    flipped: bool,
    moves: Vec<YbbMove>,
}

impl YbbEntry {
    /// 保存時の向きで局面行を生成する。
    ///
    /// キーは reader と同様にバイト列として扱い、局面の復元や合法手検査は行わない。
    /// 候補手の符号化と個数を検査し、候補手順と重複を保持する。
    /// 各候補手の `mv()` を保存する指し手とし、`raw_move()` もその値に揃える。
    pub fn new(
        packed_sfen: PackedSfen,
        ply: Ply,
        mut moves: Vec<YbbMove>,
    ) -> Result<Self, BookError> {
        validate_move_count(moves.len())?;
        for candidate in &mut moves {
            *candidate = YbbMove::new(candidate.mv, candidate.eval, candidate.depth)?;
        }
        Ok(Self { packed_sfen, ply, flipped: false, moves })
    }

    /// 保存時の向きの局面キーを返す。
    #[must_use]
    pub const fn packed_sfen(&self) -> PackedSfen {
        self.packed_sfen
    }

    /// 索引行に保存された手数を返す。
    #[must_use]
    pub const fn ply(&self) -> Ply {
        self.ply
    }

    /// 反転検索によるヒットかどうかを返す。
    #[must_use]
    pub const fn flipped(&self) -> bool {
        self.flipped
    }

    /// 保存順の候補手を返す。
    #[must_use]
    pub fn moves(&self) -> &[YbbMove] {
        &self.moves
    }
}

/// YBB の候補手と手番視点の評価値、任意の深さ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YbbMove {
    mv: Move,
    raw_move: u16,
    eval: i16,
    depth: Option<u16>,
}

impl YbbMove {
    /// 指し手の符号化を検査して候補手を生成する。
    ///
    /// 局面に対する合法性は検査しない。
    pub fn new(mv: Move, eval: i16, depth: Option<u16>) -> Result<Self, BookError> {
        if !is_valid_raw_move(mv.raw()) {
            return Err(BookError::InvalidData(format!("invalid YBB raw move: {:#06x}", mv.raw())));
        }
        Ok(Self { mv, raw_move: mv.raw(), eval, depth })
    }

    /// 問い合わせ局面の向きに変換した指し手を返す。
    #[must_use]
    pub const fn mv(self) -> Move {
        self.mv
    }

    /// 保存時の向きの指し手の生値を返す。
    #[must_use]
    pub const fn raw_move(self) -> u16 {
        self.raw_move
    }

    /// 指す前の手番視点の評価値を返す。
    #[must_use]
    pub const fn eval(self) -> i16 {
        self.eval
    }

    /// 保存された深さを返す。深さなしの形式では `None` を返す。
    #[must_use]
    pub const fn depth(self) -> Option<u16> {
        self.depth
    }
}

impl YbbBook {
    /// ファイル全体を読み込み、索引順と候補手の符号化を検査する。
    pub fn open(path: impl AsRef<Path>) -> Result<Self, BookError> {
        Self::open_with_options(path, YbbBookOpenOptions::new())
    }

    /// 検索、データ保持方式、検査方針を指定して開く。
    ///
    /// データ保持方式によらず、ヘッダーと索引領域の長さを検査する。
    /// [`YbbValidationMode::Full`] では全件を検査してから返す。
    /// [`YbbValidationMode::OnAccess`] では行の範囲と候補手の符号化を参照時に検査する。
    /// 後者の検索には整列済みファイルを用いる。I/O 失敗もエラーとして返す。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rsshogi::book::{
    ///     BookControl, YbbBook, YbbBookOpenOptions, YbbStorageMode, YbbValidationMode,
    /// };
    ///
    /// let book = YbbBook::open_with_options(
    ///     "large.ybb",
    ///     YbbBookOpenOptions::new()
    ///         .with_storage_mode(YbbStorageMode::File)
    ///         .with_validation_mode(YbbValidationMode::OnAccess),
    /// )?;
    /// book.validate_full_with_control(|progress| {
    ///     println!("{}/{}", progress.processed_row_count(), progress.total_row_count());
    ///     BookControl::Continue
    /// })?;
    /// # Ok::<(), rsshogi::book::BookError>(())
    /// ```
    pub fn open_with_options(
        path: impl AsRef<Path>,
        options: YbbBookOpenOptions,
    ) -> Result<Self, BookError> {
        let book = if options.storage_mode() == YbbStorageMode::File {
            let mut file = File::open(path)?;
            let file_bytes = file.metadata()?.len();
            let mut header = [0; HEADER_BYTES];
            if file_bytes < HEADER_BYTES as u64 {
                return Err(BookError::InvalidFormat("truncated YBB header"));
            }
            file.read_exact(&mut header)?;
            let (row_count, move_area, has_depth) = parse_header(&header, file_bytes)?;
            Self {
                storage: YbbStorage::File {
                    file: Arc::new(Mutex::new(file)),
                    file_bytes,
                    row_count,
                },
                move_area,
                has_depth,
                options,
            }
        } else {
            let bytes = fs::read(path)?;
            let (row_count, move_area, has_depth) = parse_header(&bytes, bytes.len() as u64)?;
            let mut rows = Vec::with_capacity(row_count);
            for index in 0..row_count {
                let start = HEADER_BYTES + index * INDEX_BYTES;
                rows.push(decode_index_row(&bytes[start..start + INDEX_BYTES])?);
            }
            Self { storage: YbbStorage::Memory { bytes, rows }, move_area, has_depth, options }
        };
        if options.validation_mode() == YbbValidationMode::Full {
            book.validate_full()?;
        }
        Ok(book)
    }

    #[must_use]
    pub const fn len(&self) -> u64 {
        match &self.storage {
            YbbStorage::Memory { rows, .. } => rows.len() as u64,
            YbbStorage::File { row_count, .. } => *row_count as u64,
        }
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub const fn has_depth(&self) -> bool {
        self.has_depth
    }

    #[must_use]
    pub const fn options(&self) -> YbbBookOpenOptions {
        self.options
    }

    /// 全索引の整列、各行の範囲、全候補手の符号化を検査する。
    ///
    /// データ保持方式と open 時の検査方針によらず全行を検査し、設定は変更しない。
    /// キーの局面復元と候補手の合法性は検査しない。
    pub fn validate_full(&self) -> Result<(), BookError> {
        self.validate_full_with_control(|_| BookControl::Continue)
    }

    /// 全行を検査し、同一キーの複数行と手数違いを集計する。
    ///
    /// 追加メモリは行数に比例しない。キーの復元、正準性、合法手は検査しない。
    /// 重複は件数として報告し、設定と lookup の選択順を変更しない。
    ///
    /// # Errors
    /// 構造破損、不正な指し手符号化、読み込み失敗を返す。
    pub fn diagnose_full(&self) -> Result<YbbDiagnostics, BookError> {
        self.diagnose_full_with_control(|_| BookControl::Continue)
    }

    /// 全件診断の開始時と各行の検査後に進捗を通知する。
    ///
    /// callback 中にファイルロックを保持しない。成功時だけ完全な集計を返す。
    ///
    /// # Errors
    /// [`Self::diagnose_full`] のエラーと、`Cancel` に対する `Cancelled` を返す。
    pub fn diagnose_full_with_control(
        &self,
        mut control: impl FnMut(YbbValidationProgress) -> BookControl,
    ) -> Result<YbbDiagnostics, BookError> {
        let mut report = YbbDiagnostics::default();
        let notify =
            |processed_row_count, control: &mut dyn FnMut(YbbValidationProgress) -> BookControl| {
                if control(YbbValidationProgress {
                    processed_row_count,
                    total_row_count: self.len(),
                }) == BookControl::Cancel
                {
                    Err(BookError::Cancelled)
                } else {
                    Ok(())
                }
            };
        notify(0, &mut control)?;
        let mut previous = None;
        let mut group_rows = 0u64;
        let mut distinct_plies = 0u32;
        let mut seen = [0u64; 1024];
        let mut touched = Vec::with_capacity(1024);
        for entry in self.iter_entries() {
            let entry = entry?;
            if previous != Some(entry.packed_sfen) {
                for word in touched.drain(..) {
                    seen[usize::from(word)] = 0;
                }
                previous = Some(entry.packed_sfen);
                report.position_count += 1;
                group_rows = 0;
                distinct_plies = 0;
            }
            group_rows += 1;
            if group_rows == 2 {
                report.duplicate_position_count += 1;
            }
            let word = usize::from(entry.ply) / 64;
            let bit = 1u64 << (entry.ply % 64);
            if seen[word] & bit != 0 {
                report.duplicate_ply_row_count += 1;
            } else {
                if seen[word] == 0 {
                    touched.push(word as u16);
                }
                seen[word] |= bit;
                distinct_plies += 1;
                if distinct_plies == 2 {
                    report.varying_ply_position_count += 1;
                }
            }
            report.row_count += 1;
            notify(report.row_count, &mut control)?;
        }
        Ok(report)
    }

    /// 同じ PackedSfen の全行を、手数と重複を含めて保存順で返す。
    ///
    /// 検索オプションを適用せず `flipped=false` で返す。整列済み索引を前提とし、
    /// `OnAccess` の未検査入力では全件検査または診断で整列を確認しておく。
    /// キーはバイト列として扱い、局面復元と合法手検査を行わない。
    ///
    /// # Errors
    /// 下限探索中の範囲検査と読み込みの失敗を返す。反復中のエラーは iterator が返す。
    pub fn iter_entries_by_packed_sfen(
        &self,
        key: PackedSfen,
    ) -> Result<YbbMatchingEntries<'_>, BookError> {
        let mut entries = self.iter_entries();
        entries.index = self.lower_bound(key)?;
        Ok(YbbMatchingEntries { entries, key: Some(key) })
    }

    /// 進捗通知とキャンセル付きで全件を検査する。
    ///
    /// 開始時と各行の検査後にコールバックを呼ぶ。`Cancel` には
    /// [`BookError::Cancelled`] を返し、reader の設定と検索結果は変更しない。
    /// 空の定跡でも開始時に通知する。コールバック中にファイルのロックは保持しない。
    pub fn validate_full_with_control(
        &self,
        mut control: impl FnMut(YbbValidationProgress) -> BookControl,
    ) -> Result<(), BookError> {
        let total_row_count = self.len();
        if control(YbbValidationProgress { processed_row_count: 0, total_row_count })
            == BookControl::Cancel
        {
            return Err(BookError::Cancelled);
        }
        for (index, entry) in self.iter_entries().enumerate() {
            let _ = entry?;
            if control(YbbValidationProgress {
                processed_row_count: index as u64 + 1,
                total_row_count,
            }) == BookControl::Cancel
            {
                return Err(BookError::Cancelled);
            }
        }
        Ok(())
    }

    /// 検索オプションを適用せず、索引順に全行を返す。
    ///
    /// 手数違いと重複行、候補手順を保持し、各行を保存時の向きで復号する。
    /// ファイル参照時は各行を読み込むときに範囲と候補手を検査し、索引の逆順も通知する。
    /// 索引と候補手領域を別々にバッファリングし、iterator ごとの読み込み位置を持つ。
    /// 追加メモリは行数に比例せず、候補手領域の非連続な offset にも対応する。
    pub fn iter_entries(&self) -> YbbEntries<'_> {
        YbbEntries {
            book: self,
            index: 0,
            previous: None,
            index_buffer: ScanBuffer::default(),
            move_buffer: ScanBuffer::default(),
        }
    }

    pub fn lookup_position(&self, pos: &Position) -> Result<Option<YbbEntry>, BookError> {
        let packed = packed_sfen_for_lookup(pos, "position")?;
        if let Some(entry) = self.lookup(packed, pos.game_ply(), false)? {
            return Ok(Some(entry));
        }
        if !self.options.flipped() {
            return Ok(None);
        }
        let flipped = pos.try_to_packed_sfen_flipped().map_err(|error| {
            BookError::InvalidData(format!("cannot flip YBB position: {error}"))
        })?;
        self.lookup(flipped, pos.game_ply(), true)
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
        let flipped = position.try_to_packed_sfen_flipped().map_err(|error| {
            BookError::InvalidData(format!("cannot flip YBB lookup key: {error}"))
        })?;
        self.lookup(flipped, ply, true)
    }

    fn lookup(
        &self,
        packed_sfen: PackedSfen,
        ply: Ply,
        flipped: bool,
    ) -> Result<Option<YbbEntry>, BookError> {
        let left = self.lower_bound(packed_sfen)?;
        for index in left..self.len() as usize {
            let row = self.index_row(index)?;
            if row.packed_sfen != packed_sfen {
                break;
            }
            if self.options.ignore_ply() || row.ply == ply {
                return Ok(Some(YbbEntry {
                    packed_sfen: row.packed_sfen,
                    ply: row.ply,
                    flipped,
                    moves: self.decode_moves(row, flipped)?,
                }));
            }
        }
        Ok(None)
    }

    fn lower_bound(&self, packed_sfen: PackedSfen) -> Result<usize, BookError> {
        let mut left = 0;
        let mut right = self.len() as usize;
        while left < right {
            let middle = left + (right - left) / 2;
            if self.index_row(middle)?.packed_sfen.cmp_bytes(&packed_sfen).is_lt() {
                left = middle + 1;
            } else {
                right = middle;
            }
        }
        Ok(left)
    }

    fn index_row(&self, index: usize) -> Result<IndexRow, BookError> {
        let row = match &self.storage {
            YbbStorage::Memory { rows, .. } => rows
                .get(index)
                .copied()
                .ok_or(BookError::InvalidFormat("YBB index out of range"))?,
            YbbStorage::File { row_count, .. } => {
                if index >= *row_count {
                    return Err(BookError::InvalidFormat("YBB index out of range"));
                }
                // ヘッダー検査で全索引の終端が u64 に収まることを確認済みである。
                let offset = HEADER_BYTES as u64 + index as u64 * INDEX_BYTES as u64;
                let mut bytes = [0; INDEX_BYTES];
                self.read_file_exact(offset, &mut bytes)?;
                decode_index_row(&bytes)?
            }
        };
        move_range(row, self.move_area, self.storage_bytes(), self.has_depth)?;
        Ok(row)
    }

    fn storage_bytes(&self) -> u64 {
        match &self.storage {
            YbbStorage::Memory { bytes, .. } => bytes.len() as u64,
            YbbStorage::File { file_bytes, .. } => *file_bytes,
        }
    }

    fn read_file_exact(&self, offset: u64, bytes: &mut [u8]) -> Result<(), BookError> {
        let YbbStorage::File { file, file_bytes, .. } = &self.storage else {
            unreachable!("file reads require file-backed YBB storage");
        };
        if offset.checked_add(bytes.len() as u64).is_none_or(|end| end > *file_bytes) {
            return Err(BookError::InvalidFormat("YBB read exceeds captured length"));
        }
        let mut file =
            file.lock().map_err(|_| BookError::InvalidData("YBB file lock poisoned".into()))?;
        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(bytes)?;
        Ok(())
    }

    fn decode_moves(&self, row: IndexRow, flipped: bool) -> Result<Vec<YbbMove>, BookError> {
        let (start, byte_count) =
            move_range(row, self.move_area, self.storage_bytes(), self.has_depth)?;
        let bytes = match &self.storage {
            YbbStorage::Memory { bytes, .. } => {
                Cow::Borrowed(&bytes[start as usize..start as usize + byte_count])
            }
            YbbStorage::File { .. } => {
                let mut bytes = vec![0; byte_count];
                self.read_file_exact(start, &mut bytes)?;
                Cow::Owned(bytes)
            }
        };
        decode_moves(&bytes, row.move_count, self.has_depth, flipped)
    }
}

fn decode_moves(
    bytes: &[u8],
    move_count: u16,
    has_depth: bool,
    flipped: bool,
) -> Result<Vec<YbbMove>, BookError> {
    let record_bytes = if has_depth { 6usize } else { 4usize };
    let mut moves = Vec::with_capacity(usize::from(move_count));
    for index in 0..usize::from(move_count) {
        let base = index * record_bytes;
        let raw_move = read_u16(&bytes[base..base + 2])?;
        if !is_valid_raw_move(raw_move) {
            return Err(BookError::InvalidData(format!("invalid YBB raw move: {raw_move:#06x}")));
        }
        let eval = read_i16(&bytes[base + 2..base + 4])?;
        let depth = if has_depth { Some(read_u16(&bytes[base + 4..base + 6])?) } else { None };
        let mv =
            if flipped { flip_move(Move::from_raw(raw_move)) } else { Move::from_raw(raw_move) };
        moves.push(YbbMove { mv, raw_move, eval, depth });
    }
    Ok(moves)
}

/// 共有ファイルのカーソルに依存せず、連続する参照をまとめて読み込む。
#[derive(Default)]
struct ScanBuffer {
    start: u64,
    bytes: Vec<u8>,
}

impl ScanBuffer {
    fn get(&mut self, book: &YbbBook, start: u64, len: usize) -> Result<&[u8], BookError> {
        if len == 0 {
            return Ok(&[]);
        }
        let end = start
            .checked_add(len as u64)
            .filter(|&end| end <= book.storage_bytes())
            .ok_or(BookError::InvalidFormat("YBB scan exceeds captured length"))?;
        if start < self.start || end > self.start + self.bytes.len() as u64 {
            // 最大候補手数の行も一度に読める。通常行では64 KiBを再利用する。
            let read_bytes =
                (SCAN_BUFFER_BYTES.max(len) as u64).min(book.storage_bytes() - start) as usize;
            self.bytes.resize(read_bytes, 0);
            // 読み込み失敗後に部分更新した buffer を有効と見なさない。
            self.start = u64::MAX;
            if let Err(error) = book.read_file_exact(start, &mut self.bytes) {
                self.bytes.clear();
                return Err(error);
            }
            self.start = start;
        }
        let offset = (start - self.start) as usize;
        Ok(&self.bytes[offset..offset + len])
    }
}

/// YBB の索引順で、候補手を1行ずつ復号する iterator。
pub struct YbbEntries<'a> {
    book: &'a YbbBook,
    index: usize,
    previous: Option<PackedSfen>,
    index_buffer: ScanBuffer,
    move_buffer: ScanBuffer,
}

impl Iterator for YbbEntries<'_> {
    type Item = Result<YbbEntry, BookError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.book.len() as usize {
            return None;
        }
        let row = self.index_row();
        self.index += 1;
        Some(row.and_then(|row| self.decode_entry(row)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.book.len() as usize - self.index;
        (remaining, Some(remaining))
    }
}

impl YbbEntries<'_> {
    fn index_row(&mut self) -> Result<IndexRow, BookError> {
        match &self.book.storage {
            YbbStorage::Memory { .. } => self.book.index_row(self.index),
            YbbStorage::File { .. } => {
                let offset = HEADER_BYTES as u64 + self.index as u64 * INDEX_BYTES as u64;
                let row =
                    decode_index_row(self.index_buffer.get(self.book, offset, INDEX_BYTES)?)?;
                move_range(
                    row,
                    self.book.move_area,
                    self.book.storage_bytes(),
                    self.book.has_depth,
                )?;
                Ok(row)
            }
        }
    }

    fn decode_entry(&mut self, row: IndexRow) -> Result<YbbEntry, BookError> {
        let previous = self.previous.replace(row.packed_sfen);
        if previous.is_some_and(|key| key.cmp_bytes(&row.packed_sfen).is_gt()) {
            return Err(BookError::InvalidFormat("YBB index is not sorted"));
        }
        let (start, len) =
            move_range(row, self.book.move_area, self.book.storage_bytes(), self.book.has_depth)?;
        let moves = match &self.book.storage {
            YbbStorage::Memory { .. } => self.book.decode_moves(row, false),
            YbbStorage::File { .. } => self
                .move_buffer
                .get(self.book, start, len)
                .and_then(|bytes| decode_moves(bytes, row.move_count, self.book.has_depth, false)),
        };
        moves.map(|moves| YbbEntry {
            packed_sfen: row.packed_sfen,
            ply: row.ply,
            flipped: false,
            moves,
        })
    }
}

impl ExactSizeIterator for YbbEntries<'_> {}
impl std::iter::FusedIterator for YbbEntries<'_> {}

/// 同じ局面キーの行を保存順に返す iterator。
///
/// 最初のエラーを返した後は終了する。全一致行をメモリへ蓄積しない。
pub struct YbbMatchingEntries<'a> {
    entries: YbbEntries<'a>,
    key: Option<PackedSfen>,
}

impl Iterator for YbbMatchingEntries<'_> {
    type Item = Result<YbbEntry, BookError>;

    fn next(&mut self) -> Option<Self::Item> {
        let key = self.key?;
        if self.entries.index == self.entries.book.len() as usize {
            self.key = None;
            return None;
        }
        let row = self.entries.index_row();
        self.entries.index += 1;
        let result = match row {
            Ok(row) if row.packed_sfen == key => self.entries.decode_entry(row),
            Ok(_) => {
                self.key = None;
                return None;
            }
            Err(error) => Err(error),
        };
        if result.is_err() {
            self.key = None;
        }
        Some(result)
    }
}

impl std::iter::FusedIterator for YbbMatchingEntries<'_> {}

/// YBB の depth 有無と、所有 IR からの変換時の情報破棄を指定する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct YbbWriteOptions {
    has_depth: bool,
    allow_lossy: bool,
}

impl YbbWriteOptions {
    /// depth なし、情報破棄を許可しない既定設定を返す。
    #[must_use]
    pub const fn new() -> Self {
        Self { has_depth: false, allow_lossy: false }
    }

    /// ファイル全体で depth を保存するか指定する。
    #[must_use]
    pub const fn with_depth(mut self, has_depth: bool) -> Self {
        self.has_depth = has_depth;
        self
    }

    /// `BookDatabase` 出力で、メタデータと保存対象外の depth の破棄を許可する。
    ///
    /// 必須値の欠落、保存する数値の範囲外、不正入力は許可しない。
    /// `YbbWriter` へ直接渡す行では情報破棄を行わない。
    #[must_use]
    pub const fn with_allow_lossy(mut self, allow_lossy: bool) -> Self {
        self.allow_lossy = allow_lossy;
        self
    }
}

/// 行数が既知の YBB を順次出力する writer。
///
/// 空の出力先を必要とし、索引と候補手を上限付きのブロックで出力する。
/// 追加メモリは全行数に比例せず、固定バッファと最大の単一行に比例する。
/// `finish` が索引とヘッダーを確定して flush する。永続化や保存先の置換は行わない。
/// いずれかの操作が失敗した後は使用できず、未完成の出力を破棄する必要がある。
/// `BufWriter<File>` は主に `finish` 時のヘッダーフィールドの小さな書き込みをまとめる。
///
/// ```
/// use std::io::Cursor;
/// use rsshogi::board::hirate_position;
/// use rsshogi::book::{YbbEntry, YbbMove, YbbWriteOptions, YbbWriter};
/// use rsshogi::types::Move;
///
/// let entry = YbbEntry::new(
///     hirate_position().to_packed_sfen(), 1,
///     vec![YbbMove::new(Move::from_usi("7g7f").expect("valid USI"), -25, Some(12))?],
/// )?;
/// let mut writer = YbbWriter::new(
///     Cursor::new(Vec::new()), 1, YbbWriteOptions::new().with_depth(true),
/// )?;
/// writer.write_entry(&entry)?;
/// let output = writer.finish()?;
/// assert_eq!(output.into_inner().len(), 82);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct YbbWriter<W> {
    output: W,
    entry_count: u64,
    options: YbbWriteOptions,
    index: Vec<[u8; INDEX_BYTES]>,
    moves: Vec<u8>,
    written_row_count: u64,
    flushed_row_count: u64,
    flushed_move_bytes: u64,
    previous: Option<PackedSfen>,
    move_area: u64,
    move_bytes: u64,
    failed: bool,
}

impl<W: Write + Seek> YbbWriter<W> {
    /// 空で先頭に位置する出力先と、出力予定の行数を指定する。
    pub fn new(
        mut output: W,
        entry_count: u64,
        options: YbbWriteOptions,
    ) -> Result<Self, BookError> {
        let move_area = entry_count
            .checked_mul(INDEX_BYTES as u64)
            .and_then(|bytes| bytes.checked_add(HEADER_BYTES as u64))
            .ok_or(BookError::InvalidData("YBB index size overflow".into()))?;
        if output.stream_position()? != 0 || output.seek(SeekFrom::End(0))? != 0 {
            return Err(BookError::InvalidData(
                "YBB output must be empty and positioned at zero".into(),
            ));
        }
        output.seek(SeekFrom::Start(move_area))?;
        Ok(Self {
            output,
            entry_count,
            options,
            index: Vec::with_capacity(WRITE_BUFFER_BYTES / INDEX_BYTES),
            moves: Vec::with_capacity(WRITE_BUFFER_BYTES),
            written_row_count: 0,
            flushed_row_count: 0,
            flushed_move_bytes: 0,
            previous: None,
            move_area,
            move_bytes: 0,
            failed: false,
        })
    }

    /// キーの byte 辞書順で 1 行出力する。同一キーの行は入力順に保持する。
    ///
    /// `flipped=true` の検索結果、予定行数の超過、depth 有無の不一致を拒否する。
    /// キーの復元と局面に対する合法手検査は行わない。
    pub fn write_entry(&mut self, entry: &YbbEntry) -> Result<(), BookError> {
        if self.failed {
            return Err(BookError::InvalidData("YBB writer has failed".into()));
        }
        self.failed = true;
        self.write_entry_inner(entry)?;
        self.failed = false;
        Ok(())
    }

    fn write_entry_inner(&mut self, entry: &YbbEntry) -> Result<(), BookError> {
        if self.written_row_count >= self.entry_count {
            return Err(BookError::InvalidData("too many YBB entries".into()));
        }
        if entry.flipped {
            return Err(BookError::InvalidData("cannot write flipped YBB lookup entry".into()));
        }
        if self.previous.is_some_and(|previous| previous.cmp_bytes(&entry.packed_sfen).is_gt()) {
            return Err(BookError::InvalidData("YBB entries are not sorted".into()));
        }
        let move_count = validate_move_count(entry.moves.len())?;
        let width = if self.options.has_depth { 6 } else { 4 };
        let next_bytes = self
            .move_bytes
            .checked_add(u64::from(move_count) * width)
            .filter(|bytes| self.move_area.checked_add(*bytes).is_some())
            .ok_or(BookError::InvalidData("YBB move offset overflow".into()))?;
        for candidate in &entry.moves {
            if candidate.depth.is_some() != self.options.has_depth {
                return Err(BookError::InvalidData(
                    "YBB depth presence does not match header".into(),
                ));
            }
        }
        let row_bytes = (u64::from(move_count) * width) as usize;
        if self.index.len() == WRITE_BUFFER_BYTES / INDEX_BYTES
            || self.moves.len() + row_bytes > WRITE_BUFFER_BYTES
        {
            self.flush_block()?;
        }
        // 単一巨大行を受けても、Vec の倍増による過剰な予約を避ける。
        if row_bytes > self.moves.capacity() {
            self.moves.reserve_exact(row_bytes - self.moves.len());
        }
        for candidate in &entry.moves {
            self.moves.extend_from_slice(&candidate.raw_move.to_le_bytes());
            self.moves.extend_from_slice(&candidate.eval.to_le_bytes());
            if let Some(depth) = candidate.depth {
                self.moves.extend_from_slice(&depth.to_le_bytes());
            }
        }
        let mut row = [0u8; INDEX_BYTES];
        row[..32].copy_from_slice(entry.packed_sfen.as_bytes());
        row[32..40].copy_from_slice(&self.move_bytes.to_le_bytes());
        row[40..42].copy_from_slice(&entry.ply.to_le_bytes());
        row[42..44].copy_from_slice(&move_count.to_le_bytes());
        self.index.push(row);
        self.previous = Some(entry.packed_sfen);
        self.move_bytes = next_bytes;
        self.written_row_count += 1;
        if self.moves.len() >= WRITE_BUFFER_BYTES {
            self.flush_block()?;
        }
        Ok(())
    }

    fn flush_block(&mut self) -> Result<(), BookError> {
        if self.index.is_empty() {
            return Ok(());
        }
        self.output.seek(SeekFrom::Start(
            HEADER_BYTES as u64 + self.flushed_row_count * INDEX_BYTES as u64,
        ))?;
        self.output.write_all(self.index.as_flattened())?;
        self.output.seek(SeekFrom::Start(self.move_area + self.flushed_move_bytes))?;
        self.output.write_all(&self.moves)?;
        self.flushed_row_count += self.index.len() as u64;
        self.flushed_move_bytes += self.moves.len() as u64;
        self.index.clear();
        self.moves.clear();
        Ok(())
    }

    /// 行数を検査し、索引、ヘッダー、flush が成功した出力先を返す。
    pub fn finish(mut self) -> Result<W, BookError> {
        if self.failed || self.written_row_count != self.entry_count {
            return Err(BookError::InvalidData("failed or incomplete YBB writer".into()));
        }
        self.flush_block()?;
        self.output.seek(SeekFrom::Start(0))?;
        self.output.write_all(MAGIC)?;
        self.output.write_all(&self.entry_count.to_le_bytes())?;
        self.output.write_all(&u64::from(self.options.has_depth).to_le_bytes())?;
        self.output.seek(SeekFrom::Start(self.move_area + self.move_bytes))?;
        self.output.flush()?;
        Ok(self.output)
    }
}

fn validate_move_count(count: usize) -> Result<u16, BookError> {
    u16::try_from(count).map_err(|_| BookError::InvalidData("YBB move count exceeds u16".into()))
}

impl BookDatabase {
    /// 所有 IR をキー順の YBB として空の出力先へ書き出す。
    ///
    /// 候補手順を保持する。手数は `original_ply`、評価値は `score` を必須とする。
    /// depth を保存する場合は全候補手に値を必要とする。整数は切り詰めない。
    /// 形式固有メタデータ、ponder、weight、count、保存しない depth があれば、
    /// `with_allow_lossy(true)` の明示指定を必要とする。
    /// 局面の符号化と候補手の合法性を検査する。
    /// ソート用のキーと参照に O(局面数) の追加メモリを使う。
    /// 入力の検査は出力開始前に行う。I/O エラー時は未完成の出力が残り得る。
    pub fn write_ybb<W: Write + Seek>(
        &self,
        output: &mut W,
        options: &YbbWriteOptions,
    ) -> Result<(), BookError> {
        self.validate()?;
        let mut entries = Vec::with_capacity(self.len());
        for entry in self.entries() {
            let converted = database_ybb_entry(entry, *options)?;
            entries.push((converted.packed_sfen, entry));
        }
        entries.sort_by(|left, right| left.0.cmp_bytes(&right.0));
        let mut writer = YbbWriter::new(output, self.len() as u64, *options)?;
        for (_, entry) in entries {
            writer.write_entry(&database_ybb_entry(entry, *options)?)?;
        }
        writer.finish()?;
        Ok(())
    }
}

fn database_ybb_entry(
    entry: &BookDatabaseEntry,
    options: YbbWriteOptions,
) -> Result<YbbEntry, BookError> {
    let position = Position::from_sfen(entry.position().sfen())
        .map_err(|error| BookError::InvalidData(format!("invalid YBB position: {error}")))?;
    let packed_sfen = position
        .try_to_packed_sfen()
        .map_err(|error| BookError::InvalidData(format!("unencodable YBB position: {error}")))?;
    let ply = entry
        .position()
        .original_ply()
        .ok_or_else(|| BookError::InvalidData("missing YBB ply".into()))?;
    let ply =
        u16::try_from(ply).map_err(|_| BookError::InvalidData("YBB ply exceeds u16".into()))?;
    if !options.allow_lossy && entry.metadata() != &BookEntryMetadata::default() {
        return Err(BookError::InvalidData(
            "ybb cannot store entry metadata without information loss".into(),
        ));
    }
    validate_move_count(entry.candidates().len())?;
    let mut moves = Vec::with_capacity(entry.candidates().len());
    for candidate in entry.candidates() {
        let eval = candidate
            .score()
            .ok_or_else(|| BookError::InvalidData("missing YBB evaluation".into()))?;
        let eval = i16::try_from(eval)
            .map_err(|_| BookError::InvalidData("YBB evaluation exceeds i16".into()))?;
        let depth = if options.has_depth {
            let depth = candidate
                .depth()
                .ok_or_else(|| BookError::InvalidData("missing YBB depth".into()))?;
            Some(
                u16::try_from(depth)
                    .map_err(|_| BookError::InvalidData("YBB depth exceeds u16".into()))?,
            )
        } else {
            None
        };
        if !options.allow_lossy
            && (candidate.ponder().is_some()
                || candidate.weight().is_some()
                || candidate.count().is_some()
                || candidate.metadata() != &BookMoveMetadata::default()
                || (!options.has_depth && candidate.depth().is_some()))
        {
            return Err(BookError::InvalidData(
                "ybb cannot store candidate metadata or omitted depth without information loss"
                    .into(),
            ));
        }
        let converted = YbbMove::new(candidate.mv(), eval, depth)?;
        if !position.is_legal_move32(position.move32_from_move(candidate.mv())) {
            return Err(BookError::InvalidData(format!(
                "illegal YBB candidate: {}",
                candidate.mv().to_usi()
            )));
        }
        moves.push(converted);
    }
    YbbEntry::new(packed_sfen, ply, moves)
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

fn packed_sfen_for_lookup(pos: &Position, description: &str) -> Result<PackedSfen, BookError> {
    pos.try_to_packed_sfen().map_err(|error| {
        BookError::InvalidData(format!("invalid YBB lookup {description}: {error}"))
    })
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

fn parse_header(bytes: &[u8], file_len: u64) -> Result<(usize, u64, bool), BookError> {
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
    let move_area = count
        .checked_mul(INDEX_BYTES as u64)
        .and_then(|size| size.checked_add(HEADER_BYTES as u64))
        .ok_or(BookError::InvalidFormat("YBB index size overflow"))?;
    if move_area > file_len {
        return Err(BookError::InvalidFormat("truncated YBB index"));
    }
    Ok((count_usize, move_area, flags & 1 != 0))
}

fn decode_index_row(bytes: &[u8]) -> Result<IndexRow, BookError> {
    Ok(IndexRow {
        packed_sfen: PackedSfen::from_bytes(bytes[..32].try_into().expect("fixed YBB index slice")),
        moves_offset: read_u64(&bytes[32..40])?,
        ply: read_u16(&bytes[40..42])?,
        move_count: read_u16(&bytes[42..44])?,
    })
}

fn move_range(
    row: IndexRow,
    move_area: u64,
    file_len: u64,
    has_depth: bool,
) -> Result<(u64, usize), BookError> {
    let byte_count = usize::from(row.move_count) * if has_depth { 6 } else { 4 };
    let start = move_area
        .checked_add(row.moves_offset)
        .ok_or(BookError::InvalidFormat("YBB move range overflow"))?;
    let end = start
        .checked_add(byte_count as u64)
        .ok_or(BookError::InvalidFormat("YBB move range overflow"))?;
    if end > file_len {
        return Err(BookError::InvalidFormat("truncated YBB move data"));
    }
    Ok((start, byte_count))
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

#[cfg(test)]
mod tests;
