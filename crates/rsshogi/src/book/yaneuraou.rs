use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::{BookControl, BookError};
use crate::board::{self, Position};
use crate::types::Move;

const HEADER: &str = "#YANEURAOU-DB2016 1.00";
const SFEN_PREFIX: &str = "sfen ";
const DEFAULT_VALIDATION_LIMIT: usize = 10_000;
const VALIDATION_PROGRESS_BYTE_INTERVAL: u64 = 1 << 20;
const VALIDATION_PROGRESS_ROW_INTERVAL: usize = 4096;

/// ファイルを全展開しない読み取り専用の YaneuraOu DB2016 リーダー。
#[derive(Debug, Clone)]
pub struct YaneuraOuBook {
    path: PathBuf,
    file_bytes: u64,
    ordering: YaneuraOuBookDiagnostics,
    lookup_mode: YaneuraOuLookupMode,
}

impl YaneuraOuBook {
    /// DB2016 テキスト棋譜ファイルをオンデマンド検索用に開く。
    ///
    /// リーダーは局面行の先頭一部を検証する。返された診断結果がソート済みだが
    /// 不完全な場合は、検索前に [`Self::validate_full`] を呼ぶか、
    /// [`Self::lookup_sfen_by_scan`] を使うこと。
    pub fn open(path: impl AsRef<Path>) -> Result<Self, BookError> {
        Self::open_with_options(path, YaneuraOuBookOpenOptions::default())
    }

    /// 検証・検索ポリシーを明示して DB2016 テキスト棋譜ファイルを開く。
    ///
    /// `Assume` を含むモードは、ファイル全体で証明されていない順序の仮定に
    /// 基づく二分探索を許可する。仮定が誤っている場合、二分探索は既存のエントリを
    /// 見落とす可能性がある。
    pub fn open_with_options(
        path: impl AsRef<Path>,
        options: YaneuraOuBookOpenOptions,
    ) -> Result<Self, BookError> {
        let path = path.as_ref().to_path_buf();
        let file_bytes = std::fs::metadata(&path)?.len();
        let (ordering, lookup_mode) = match options.access_mode {
            YaneuraOuAccessMode::SafeBinary { prefix_rows } => (
                validate_ordering(&path, Some(prefix_rows), file_bytes)?,
                YaneuraOuLookupMode::RequireComplete,
            ),
            YaneuraOuAccessMode::ValidateFullBeforeLookup => (
                validate_ordering(&path, None, file_bytes)?,
                YaneuraOuLookupMode::BinaryAssumedSorted,
            ),
            YaneuraOuAccessMode::AssumeSortedAfterPrefix { prefix_rows } => (
                validate_ordering(&path, Some(prefix_rows), file_bytes)?,
                YaneuraOuLookupMode::BinaryAssumedSorted,
            ),
            YaneuraOuAccessMode::AssumeSortedByCaller => {
                (YaneuraOuBookDiagnostics::Unvalidated, YaneuraOuLookupMode::BinaryAssumedSorted)
            }
            // ScanOnly でもヘッダ・UTF-8・順序の診断を早期に得るため、先頭部分の検証を行う。
            YaneuraOuAccessMode::ScanOnly => (
                validate_ordering(&path, Some(DEFAULT_VALIDATION_LIMIT), file_bytes)?,
                YaneuraOuLookupMode::Scan,
            ),
        };
        Ok(Self { path, file_bytes, ordering, lookup_mode })
    }

    /// オープン時に取得した先頭部分の順序検証結果を返す。
    #[must_use]
    pub const fn diagnostics(&self) -> &YaneuraOuBookDiagnostics {
        &self.ordering
    }

    /// 全局面行をスキャンし、診断結果を更新して返す。
    pub fn validate_full(&mut self) -> Result<YaneuraOuBookDiagnostics, BookError> {
        self.validate_full_with_control(|_| BookControl::Continue)
    }

    /// 進捗通知とキャンセル対応で全局面行をスキャンする。
    ///
    /// コールバックは一定バイト数または行数ごと、および完了時に呼ばれる。
    /// [`BookControl::Cancel`] を返すとスキャンを停止し、
    /// [`BookError::Cancelled`] を返す。
    pub fn validate_full_with_control(
        &mut self,
        on_progress: impl FnMut(YaneuraOuValidationProgress) -> BookControl,
    ) -> Result<YaneuraOuBookDiagnostics, BookError> {
        let ordering =
            validate_ordering_with_control(&self.path, None, self.file_bytes, on_progress)?;
        self.ordering = ordering.clone();
        Ok(ordering)
    }

    /// SFEN で局面を 1 件検索する。
    ///
    /// 完全な順序検証が完了した場合にのみ二分探索を使用する。初期の先頭部分検証が
    /// 不完全な場合は先に [`Self::validate_full`] を呼ぶか、逐次スキャンを意図する
    /// 場合は [`Self::lookup_sfen_by_scan`] を使うこと。
    pub fn lookup_sfen(&self, sfen: &str) -> Result<Option<YaneuraOuBookEntry>, BookError> {
        if self.lookup_mode == YaneuraOuLookupMode::Scan {
            return self.lookup_sfen_by_scan(sfen);
        }
        match &self.ordering {
            YaneuraOuBookDiagnostics::Sorted { complete: true, .. } => {
                self.lookup_sfen_binary(sfen)
            }
            YaneuraOuBookDiagnostics::Sorted { complete: false, .. } => {
                if self.lookup_mode == YaneuraOuLookupMode::BinaryAssumedSorted {
                    self.lookup_sfen_binary(sfen)
                } else {
                    Err(BookError::Unsupported(
                        "yaneuraou db ordering was only partially validated; call validate_full or use lookup_sfen_by_scan",
                    ))
                }
            }
            YaneuraOuBookDiagnostics::Unsorted { .. } => {
                Err(BookError::Unsupported("yaneuraou db is not sorted; use lookup_sfen_by_scan"))
            }
            YaneuraOuBookDiagnostics::InvalidHeader => {
                Err(BookError::InvalidFormat("unsupported yaneuraou book header"))
            }
            YaneuraOuBookDiagnostics::Unvalidated => {
                if self.lookup_mode == YaneuraOuLookupMode::BinaryAssumedSorted {
                    self.lookup_sfen_binary(sfen)
                } else {
                    Err(BookError::Unsupported("yaneuraou db ordering was not validated"))
                }
            }
        }
    }

    /// [`Position`] で局面を 1 件検索する。
    pub fn lookup_position(
        &self,
        position: &Position,
    ) -> Result<Option<YaneuraOuBookEntry>, BookError> {
        self.lookup_sfen(&position.to_sfen(None))
    }

    /// ソート順に依存せず、ファイルを逐次スキャンして局面を 1 件検索する。
    pub fn lookup_sfen_by_scan(&self, sfen: &str) -> Result<Option<YaneuraOuBookEntry>, BookError> {
        board::init();
        let target = normalize_query_sfen(sfen)?;
        let mut reader = BufReader::new(File::open(&self.path)?);
        let mut raw = Vec::new();
        let mut line_number = 0usize;
        let mut current: Option<YaneuraOuBookEntry> = None;

        loop {
            raw.clear();
            let read = reader.read_until(b'\n', &mut raw)?;
            if read == 0 {
                return Ok(current.filter(|entry| entry.sfen == target));
            }
            line_number += 1;
            let line = decode_line(line_number, &raw)?;

            match parse_line(line_number, &line)? {
                YaneuraOuBookLine::Header | YaneuraOuBookLine::Blank => {}
                YaneuraOuBookLine::Comment(comment) => {
                    if let Some(entry) = &mut current {
                        entry.append_comment(comment);
                    }
                }
                YaneuraOuBookLine::Position { sfen, min_ply } => {
                    if matches!(current.as_ref(), Some(entry) if entry.sfen == target) {
                        return Ok(current);
                    }
                    current = Some(YaneuraOuBookEntry::new(sfen, min_ply));
                }
                YaneuraOuBookLine::Move(book_move) => {
                    if let Some(entry) = &mut current {
                        entry.moves.push(book_move);
                    }
                }
            }
        }
    }

    /// このリーダーに保存された棋譜ファイルパスを再オープンしてエントリを逐次返す。
    ///
    /// `.db` ファイルは数 GB に達することがあるため、明示的に呼び出す必要がある。
    pub fn iter_entries(&self) -> Result<YaneuraOuBookEntries, BookError> {
        Ok(YaneuraOuBookEntries {
            reader: BufReader::new(File::open(&self.path)?),
            raw: Vec::new(),
            line_number: 0,
            current: None,
            finished: false,
        })
    }

    fn lookup_sfen_binary(&self, sfen: &str) -> Result<Option<YaneuraOuBookEntry>, BookError> {
        board::init();
        let target = normalize_query_sfen(sfen)?;
        let mut file = File::open(&self.path)?;
        let mut begin = 0u64;
        let mut end = self.file_bytes;

        while begin < end {
            let mid = begin + (end - begin) / 2;
            let Some(row) = find_next_position_row(&mut file, mid, end)? else {
                end = mid;
                continue;
            };

            match target.as_str().cmp(row.sfen.as_str()) {
                Ordering::Equal => return read_entry_at(&mut file, row.after_line_offset, row),
                Ordering::Less => end = mid,
                Ordering::Greater => begin = row.after_line_offset,
            }
        }

        Ok(None)
    }
}

/// DB2016 エントリの逐次イテレータ。
#[derive(Debug)]
pub struct YaneuraOuBookEntries {
    reader: BufReader<File>,
    raw: Vec<u8>,
    line_number: usize,
    current: Option<YaneuraOuBookEntry>,
    finished: bool,
}

impl Iterator for YaneuraOuBookEntries {
    type Item = Result<YaneuraOuBookEntry, BookError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        loop {
            self.raw.clear();
            let read = match self.reader.read_until(b'\n', &mut self.raw) {
                Ok(read) => read,
                Err(err) => return Some(Err(BookError::Io(err))),
            };
            if read == 0 {
                self.finished = true;
                return self.current.take().map(Ok);
            }
            self.line_number += 1;
            let line = match decode_line(self.line_number, &self.raw) {
                Ok(line) => line,
                Err(err) => return Some(Err(err)),
            };
            match parse_line(self.line_number, &line) {
                Ok(YaneuraOuBookLine::Blank | YaneuraOuBookLine::Header) => {}
                Ok(YaneuraOuBookLine::Comment(comment)) => {
                    if let Some(entry) = &mut self.current {
                        entry.append_comment(comment);
                    }
                }
                Ok(YaneuraOuBookLine::Move(book_move)) => {
                    if let Some(entry) = &mut self.current {
                        entry.moves.push(book_move);
                    }
                }
                Ok(YaneuraOuBookLine::Position { sfen, min_ply }) => {
                    let next = YaneuraOuBookEntry::new(sfen, min_ply);
                    if let Some(previous) = self.current.replace(next) {
                        return Some(Ok(previous));
                    }
                }
                Err(err) => return Some(Err(err)),
            }
        }
    }
}

/// DB2016 テキスト棋譜ファイルを開く際のオプション。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YaneuraOuBookOpenOptions {
    access_mode: YaneuraOuAccessMode,
}

impl Default for YaneuraOuBookOpenOptions {
    fn default() -> Self {
        Self {
            access_mode: YaneuraOuAccessMode::SafeBinary { prefix_rows: DEFAULT_VALIDATION_LIMIT },
        }
    }
}

impl YaneuraOuBookOpenOptions {
    /// 安全なデフォルトのアクセスモードでオプションを生成する。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 指定したアクセスモードでオプションを生成する。
    #[must_use]
    pub const fn with_access_mode(access_mode: YaneuraOuAccessMode) -> Self {
        Self { access_mode }
    }

    /// 設定されたアクセスモードを返す。
    #[must_use]
    pub const fn access_mode(self) -> YaneuraOuAccessMode {
        self.access_mode
    }
}

/// DB2016 アクセス時の検証作業と検索保証を組み合わせたモード。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YaneuraOuAccessMode {
    /// 先頭部分を検証し、二分探索の前に完全な検証を要求する。
    SafeBinary { prefix_rows: usize },
    /// オープン時にファイル全体を検証してから二分探索を許可する。
    ValidateFullBeforeLookup,
    /// 先頭部分を検証し、残りはソート済みと仮定して二分探索を許可する。
    AssumeSortedAfterPrefix { prefix_rows: usize },
    /// ソート検証をスキップし、ソート済みであることを呼び出し側の責任とする。
    ///
    /// このモードでは [`YaneuraOuBook::diagnostics`] が [`YaneuraOuBookDiagnostics::Unvalidated`]
    /// を返すが、呼び出し側の仮定により二分探索は許可される。
    AssumeSortedByCaller,
    /// `lookup_sfen` が明示的なスキャンパスを使うようにする。
    ///
    /// オープン時は診断と早期フォーマットエラー検出のため、先頭部分の検証を行う。
    ScanOnly,
}

/// DB2016 の全順序検証中に発行される進捗情報。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YaneuraOuValidationProgress {
    processed_bytes: u64,
    total_bytes: u64,
    processed_rows: u64,
}

impl YaneuraOuValidationProgress {
    /// ファイルから消費したバイト数を返す。
    #[must_use]
    pub const fn processed_bytes(self) -> u64 {
        self.processed_bytes
    }

    /// ファイルの総バイト数を返す。
    #[must_use]
    pub const fn total_bytes(self) -> u64 {
        self.total_bytes
    }

    /// 検証済みの局面行数を返す。
    #[must_use]
    pub const fn processed_rows(self) -> u64 {
        self.processed_rows
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum YaneuraOuLookupMode {
    RequireComplete,
    BinaryAssumedSorted,
    Scan,
}

/// DB2016 テキスト棋譜ファイルの順序検証結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YaneuraOuBookDiagnostics {
    /// スキャンした局面行が正規化 SFEN で厳密に昇順だった。
    Sorted { checked_rows: usize, complete: bool },
    /// 直前の正規化 SFEN より大きくない行が見つかった。
    Unsorted {
        checked_rows: usize,
        line_number: usize,
        previous_sfen: String,
        current_sfen: String,
    },
    /// ファイルが非対応の DB ヘッダで始まっている。
    InvalidHeader,
    /// 順序検証を実施していない。
    Unvalidated,
}

/// ロスレスなオンデマンドの DB2016 エントリ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YaneuraOuBookEntry {
    sfen: String,
    min_ply: u32,
    comment: String,
    moves: Vec<YaneuraOuBookMove>,
}

impl YaneuraOuBookEntry {
    fn new(sfen: String, min_ply: u32) -> Self {
        Self { sfen, min_ply, comment: String::new(), moves: Vec::new() }
    }

    /// 検索キーとして使う正規化 SFEN を返す。
    #[must_use]
    pub fn sfen(&self) -> &str {
        &self.sfen
    }

    /// 元の行の手数を返す。行に手数がない場合は `0` を返す。
    #[must_use]
    pub const fn min_ply(&self) -> u32 {
        self.min_ply
    }

    /// この局面に付属するコメントを返す。
    #[must_use]
    pub fn comment(&self) -> &str {
        &self.comment
    }

    /// ファイル順の候補手一覧を返す。
    #[must_use]
    pub fn moves(&self) -> &[YaneuraOuBookMove] {
        &self.moves
    }

    fn append_comment(&mut self, comment: String) {
        if self.moves.is_empty() {
            append_comment(&mut self.comment, &comment);
        } else if let Some(last) = self.moves.last_mut() {
            append_comment(&mut last.comment, &comment);
        }
    }
}

/// ロスレスなオンデマンドの DB2016 指し手レコード。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YaneuraOuBookMove {
    mv: Move,
    ponder: Move,
    score: Option<i32>,
    depth: Option<u32>,
    count: Option<u64>,
    comment: String,
}

impl YaneuraOuBookMove {
    /// この局面の指し手を返す。
    #[must_use]
    pub const fn mv(&self) -> Move {
        self.mv
    }

    /// ポンダー指し手を返す。存在しない場合は `Move::MOVE_NONE` を返す。
    #[must_use]
    pub const fn ponder(&self) -> Move {
        self.ponder
    }

    /// DB2016 の score フィールドを返す。
    #[must_use]
    pub const fn score(&self) -> Option<i32> {
        self.score
    }

    /// DB2016 の depth フィールドを返す。
    #[must_use]
    pub const fn depth(&self) -> Option<u32> {
        self.depth
    }

    /// DB2016 の count フィールドを返す。
    #[must_use]
    pub const fn count(&self) -> Option<u64> {
        self.count
    }

    /// この指し手に付属するコメントを返す。
    #[must_use]
    pub fn comment(&self) -> &str {
        &self.comment
    }
}

#[derive(Debug)]
struct PositionRow {
    sfen: String,
    min_ply: u32,
    after_line_offset: u64,
}

enum YaneuraOuBookLine {
    Blank,
    Header,
    Comment(String),
    Position { sfen: String, min_ply: u32 },
    Move(YaneuraOuBookMove),
}

fn validate_ordering(
    path: &Path,
    max_rows: Option<usize>,
    total_bytes: u64,
) -> Result<YaneuraOuBookDiagnostics, BookError> {
    validate_ordering_with_control(path, max_rows, total_bytes, |_| BookControl::Continue)
}

fn validate_ordering_with_control(
    path: &Path,
    max_rows: Option<usize>,
    total_bytes: u64,
    mut on_progress: impl FnMut(YaneuraOuValidationProgress) -> BookControl,
) -> Result<YaneuraOuBookDiagnostics, BookError> {
    board::init();
    let mut reader = BufReader::new(File::open(path)?);
    let mut raw = Vec::new();
    let mut previous: Option<String> = None;
    let mut checked_rows = 0usize;
    let mut line_number = 0usize;
    let mut processed_bytes = 0u64;
    let mut next_progress_bytes = VALIDATION_PROGRESS_BYTE_INTERVAL;
    let mut next_progress_rows = VALIDATION_PROGRESS_ROW_INTERVAL;

    loop {
        raw.clear();
        let read = reader.read_until(b'\n', &mut raw)?;
        if read == 0 {
            emit_validation_progress(&mut on_progress, processed_bytes, total_bytes, checked_rows)?;
            return Ok(YaneuraOuBookDiagnostics::Sorted { checked_rows, complete: true });
        }
        processed_bytes = processed_bytes.checked_add(read as u64).ok_or_else(|| {
            BookError::InvalidData("yaneuraou validation byte count overflow".into())
        })?;
        line_number += 1;
        let line = decode_line(line_number, &raw)?;
        if line.starts_with("#YANEURAOU-DB2016") && line != HEADER {
            return Ok(YaneuraOuBookDiagnostics::InvalidHeader);
        }
        if line.is_empty() || line == HEADER || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let Some(sfen) = line.strip_prefix(SFEN_PREFIX) else {
            continue;
        };
        let (sfen, _) = normalize_db_sfen(line_number, sfen)?;

        checked_rows += 1;
        if let Some(previous_sfen) = &previous
            && previous_sfen >= &sfen
        {
            return Ok(YaneuraOuBookDiagnostics::Unsorted {
                checked_rows,
                line_number,
                previous_sfen: previous_sfen.clone(),
                current_sfen: sfen,
            });
        }
        previous = Some(sfen);

        if processed_bytes >= next_progress_bytes || checked_rows >= next_progress_rows {
            emit_validation_progress(&mut on_progress, processed_bytes, total_bytes, checked_rows)?;
            while next_progress_bytes <= processed_bytes {
                next_progress_bytes =
                    next_progress_bytes.saturating_add(VALIDATION_PROGRESS_BYTE_INTERVAL);
            }
            while next_progress_rows <= checked_rows {
                next_progress_rows =
                    next_progress_rows.saturating_add(VALIDATION_PROGRESS_ROW_INTERVAL);
            }
        }

        if max_rows.is_some_and(|max_rows| checked_rows >= max_rows) {
            return Ok(YaneuraOuBookDiagnostics::Sorted { checked_rows, complete: false });
        }
    }
}

fn emit_validation_progress(
    on_progress: &mut impl FnMut(YaneuraOuValidationProgress) -> BookControl,
    processed_bytes: u64,
    total_bytes: u64,
    processed_rows: usize,
) -> Result<(), BookError> {
    match on_progress(YaneuraOuValidationProgress {
        processed_bytes,
        total_bytes,
        processed_rows: processed_rows as u64,
    }) {
        BookControl::Continue => Ok(()),
        BookControl::Cancel => Err(BookError::Cancelled),
    }
}

fn find_next_position_row(
    file: &mut File,
    start: u64,
    end: u64,
) -> Result<Option<PositionRow>, BookError> {
    // `start` がライン途中なら、その不完全な行を読み飛ばして次の行頭に揃える。
    // ただし `start` がちょうど行頭（直前が改行 or ファイル先頭）の場合は、その行は
    // 完全な行なので読み飛ばしてはならない。二分探索の probe がある行の先頭に
    // 一致したときにその行（特に先頭/疎な position 行）を取りこぼすのを防ぐ。
    let skip_partial_line = start > 0 && !is_at_line_boundary(file, start)?;
    file.seek(SeekFrom::Start(start))?;
    if skip_partial_line {
        discard_line(file)?;
    }

    let mut reader = BufReader::new(file);
    let mut raw = Vec::new();
    let mut line_number = 0usize;

    loop {
        let offset = reader.stream_position()?;
        if offset >= end {
            return Ok(None);
        }
        raw.clear();
        let read = reader.read_until(b'\n', &mut raw)?;
        if read == 0 {
            return Ok(None);
        }
        line_number += 1;
        let after_line_offset = reader.stream_position()?;
        let line = decode_line(line_number, &raw)?;
        if let YaneuraOuBookLine::Position { sfen, min_ply } = parse_line(line_number, &line)? {
            return Ok(Some(PositionRow { sfen, min_ply, after_line_offset }));
        }
    }
}

fn read_entry_at(
    file: &mut File,
    offset: u64,
    row: PositionRow,
) -> Result<Option<YaneuraOuBookEntry>, BookError> {
    file.seek(SeekFrom::Start(offset))?;
    let mut reader = BufReader::new(file);
    let mut raw = Vec::new();
    let mut line_number = 0usize;
    let mut entry = YaneuraOuBookEntry::new(row.sfen, row.min_ply);

    loop {
        raw.clear();
        let read = reader.read_until(b'\n', &mut raw)?;
        if read == 0 {
            return Ok(Some(entry));
        }
        line_number += 1;
        let line = decode_line(line_number, &raw)?;
        match parse_line(line_number, &line)? {
            YaneuraOuBookLine::Blank | YaneuraOuBookLine::Header => {}
            YaneuraOuBookLine::Comment(comment) => entry.append_comment(comment),
            YaneuraOuBookLine::Move(book_move) => entry.moves.push(book_move),
            YaneuraOuBookLine::Position { .. } => return Ok(Some(entry)),
        }
    }
}

/// `offset` がファイル先頭、または直前のバイトが改行であれば行頭とみなす。
///
/// 呼び出し後、ファイル位置は `offset` に戻る。
fn is_at_line_boundary(file: &mut File, offset: u64) -> Result<bool, BookError> {
    if offset == 0 {
        return Ok(true);
    }
    file.seek(SeekFrom::Start(offset - 1))?;
    let mut byte = [0u8; 1];
    let read = file.read(&mut byte)?;
    // 直前バイトを 1 つ読んだので位置は `offset` に戻っている。
    Ok(read == 1 && byte[0] == b'\n')
}

fn discard_line(file: &mut File) -> Result<(), BookError> {
    let mut buf = [0u8; 1024];
    loop {
        let start = file.stream_position()?;
        let read = file.read(&mut buf)?;
        if read == 0 {
            return Ok(());
        }
        if let Some(newline) = buf[..read].iter().position(|&byte| byte == b'\n') {
            file.seek(SeekFrom::Start(start + newline as u64 + 1))?;
            return Ok(());
        }
    }
}

fn decode_line(line_number: usize, raw: &[u8]) -> Result<String, BookError> {
    let mut line = std::str::from_utf8(raw)
        .map_err(|err| {
            invalid_line(
                line_number,
                format!("invalid utf-8: {err}; CP932 / Shift_JIS input is not supported"),
            )
        })?
        .trim_end_matches(['\r', '\n'])
        .trim()
        .to_string();
    if line_number == 1 {
        line = line.trim_start_matches('\u{feff}').to_string();
    }
    Ok(line)
}

fn parse_line(line_number: usize, line: &str) -> Result<YaneuraOuBookLine, BookError> {
    if line.is_empty() {
        return Ok(YaneuraOuBookLine::Blank);
    }
    if line == HEADER {
        return Ok(YaneuraOuBookLine::Header);
    }
    if line.starts_with("#YANEURAOU-DB2016") {
        return Err(BookError::InvalidFormat("unsupported yaneuraou book header"));
    }
    if let Some(comment) = line.strip_prefix('#') {
        return Ok(YaneuraOuBookLine::Comment(comment.trim().to_string()));
    }
    if let Some(comment) = line.strip_prefix("//") {
        return Ok(YaneuraOuBookLine::Comment(comment.trim().to_string()));
    }
    if let Some(sfen) = line.strip_prefix(SFEN_PREFIX) {
        let (sfen, min_ply) = normalize_db_sfen(line_number, sfen)?;
        return Ok(YaneuraOuBookLine::Position { sfen, min_ply });
    }
    Ok(YaneuraOuBookLine::Move(parse_move_line(line_number, line)?))
}

fn normalize_query_sfen(sfen: &str) -> Result<String, BookError> {
    normalize_db_sfen(0, sfen).map(|(sfen, _)| sfen)
}

fn normalize_db_sfen(line_number: usize, sfen: &str) -> Result<(String, u32), BookError> {
    let fields: Vec<&str> = sfen.split_whitespace().collect();
    if fields.len() < 3 {
        return Err(invalid_line(line_number, "invalid sfen"));
    }

    let normalized = format!("{} {} {} 1", fields[0], fields[1], fields[2]);
    Position::from_sfen(&normalized)
        .map_err(|err| invalid_line(line_number, format!("invalid sfen: {err}")))?;
    let min_ply = fields.get(3).and_then(|ply| ply.parse::<u32>().ok()).unwrap_or(0);
    Ok((normalized, min_ply))
}

fn parse_move_line(line_number: usize, line: &str) -> Result<YaneuraOuBookMove, BookError> {
    let (fields, comment) = split_fields_and_comment(line, 5);
    let move_text = fields.first().ok_or_else(|| invalid_line(line_number, "missing move"))?;
    let ponder_text =
        fields.get(1).ok_or_else(|| invalid_line(line_number, "missing ponder move"))?;

    let mv = parse_yaneuraou_move(line_number, move_text, "move")?;
    let ponder = parse_yaneuraou_move(line_number, ponder_text, "ponder move")?;
    let score = parse_optional_i32(line_number, fields.get(2).copied(), "score")?;
    let depth = parse_optional_u32(line_number, fields.get(3).copied(), "depth")?;
    let count = parse_optional_u64(line_number, fields.get(4).copied(), "count")?;

    Ok(YaneuraOuBookMove { mv, ponder, score, depth, count, comment })
}

fn split_fields_and_comment(line: &str, max_fields: usize) -> (Vec<&str>, String) {
    let mut fields = Vec::new();
    let mut cursor = 0usize;
    let bytes = line.as_bytes();

    while fields.len() < max_fields {
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            break;
        }
        let start = cursor;
        while cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        fields.push(&line[start..cursor]);
    }

    let comment = line[cursor..]
        .trim()
        .strip_prefix("//")
        .or_else(|| line[cursor..].trim().strip_prefix('#'))
        .unwrap_or_else(|| line[cursor..].trim())
        .to_string();
    (fields, comment)
}

fn parse_optional_i32(
    line_number: usize,
    value: Option<&str>,
    field_name: &str,
) -> Result<Option<i32>, BookError> {
    match value {
        None | Some("") | Some("none") | Some("None") => Ok(None),
        Some(value) => value
            .parse::<i32>()
            .map(Some)
            .map_err(|err| invalid_line(line_number, format!("invalid {field_name}: {err}"))),
    }
}

fn parse_optional_u32(
    line_number: usize,
    value: Option<&str>,
    field_name: &str,
) -> Result<Option<u32>, BookError> {
    match value {
        None | Some("") | Some("none") | Some("None") => Ok(None),
        Some(value) => value
            .parse::<u32>()
            .map(Some)
            .map_err(|err| invalid_line(line_number, format!("invalid {field_name}: {err}"))),
    }
}

fn parse_optional_u64(
    line_number: usize,
    value: Option<&str>,
    field_name: &str,
) -> Result<Option<u64>, BookError> {
    match value {
        None | Some("") | Some("none") | Some("None") => Ok(None),
        Some(value) => value
            .parse::<u64>()
            .map(Some)
            .map_err(|err| invalid_line(line_number, format!("invalid {field_name}: {err}"))),
    }
}

fn parse_yaneuraou_move(
    line_number: usize,
    text: &str,
    field_name: &str,
) -> Result<Move, BookError> {
    let normalized = match text {
        "None" | "none" => "none",
        "resign" => "resign",
        _ => text,
    };
    Move::from_usi(normalized)
        .ok_or_else(|| invalid_line(line_number, format!("invalid {field_name}: {text}")))
}

fn append_comment(base: &mut String, next: &str) {
    if next.is_empty() {
        return;
    }
    if !base.is_empty() {
        base.push('\n');
    }
    base.push_str(next);
}

fn invalid_line(line_number: usize, message: impl Into<String>) -> BookError {
    if line_number == 0 {
        BookError::InvalidData(message.into())
    } else {
        BookError::InvalidData(format!("line {line_number}: {}", message.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitialPosition;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
    use std::time::{SystemTime, UNIX_EPOCH};

    const A: &str = "+B3g3l/5rgk1/pB+P1ppn1p/n4spp1/1G1SP3P/K2P5/1+pS3P2/P2+l+r4/LNP6 b SNL2Pg2p";
    const B: &str = "+P1kg3nl/1ps2b3/+P3p3p/2pgsr1p1/s2p1pP2/2P1P1pR1/1SNG1P2P/1KG6/7NL w N2LPb2p";
    const C: &str = "lnB4nl/4k1g2/p3pps2/1G5rp/1pPPsb3/3p2P1P/PPS1PP3/2G1KSGP1/LN5NL w R2Pp";

    static TEMP_BOOK_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_book(contents: &str) -> PathBuf {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let seq = TEMP_BOOK_COUNTER.fetch_add(1, AtomicOrdering::Relaxed);
        let path = std::env::temp_dir()
            .join(format!("rsshogi-yaneuraou-{}-{stamp}-{seq}.db", std::process::id()));
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn test_yaneuraou_db_on_demand_lookup_sorted() {
        let path = temp_book(&format!(
            "{HEADER}\n\
             sfen {A} 1\n\
             9f9e 8g7g 0 32 1 #main move\n\
             #move note\n\
             sfen {B} 78\n\
             #entry note\n\
             4e4f 9c9d 120 40 2\n\
             6e6f 6g6h -32 32 0\n\
             sfen {C} 64\n\
             4e7h+ none 540 38 1\n",
        ));
        let book = YaneuraOuBook::open(&path).unwrap();
        assert_eq!(
            book.diagnostics(),
            &YaneuraOuBookDiagnostics::Sorted { checked_rows: 3, complete: true }
        );

        let entry = book.lookup_sfen(&format!("{B} 99")).unwrap().unwrap();
        assert_eq!(entry.sfen(), &format!("{B} 1"));
        assert_eq!(entry.min_ply(), 78);
        assert_eq!(entry.comment(), "entry note");
        assert_eq!(entry.moves().len(), 2);
        assert_eq!(entry.moves()[0].mv().to_usi(), "4e4f");
        assert_eq!(entry.moves()[0].ponder().to_usi(), "9c9d");
        assert_eq!(entry.moves()[0].score(), Some(120));
        assert_eq!(entry.moves()[0].depth(), Some(40));
        assert_eq!(entry.moves()[0].count(), Some(2));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_yaneuraou_db_on_demand_supports_no_header_bom_crlf_and_none_fields() {
        let path = temp_book(&format!(
            "\u{feff}sfen {A} 1\r\n\
             9f9e none none none\r\n",
        ));
        let book = YaneuraOuBook::open(&path).unwrap();
        let entry = book.lookup_sfen(&format!("{A} 1")).unwrap().unwrap();

        assert_eq!(entry.moves()[0].ponder(), Move::MOVE_NONE);
        assert_eq!(entry.moves()[0].score(), None);
        assert_eq!(entry.moves()[0].depth(), None);
        assert_eq!(entry.moves()[0].count(), None);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_yaneuraou_db_on_demand_unsorted_requires_explicit_linear_lookup() {
        let path = temp_book(&format!(
            "{HEADER}\n\
             sfen {B} 78\n\
             4e4f 9c9d 120 40 2\n\
             sfen {A} 1\n\
             9f9e 8g7g 0 32 1\n",
        ));
        let book = YaneuraOuBook::open(&path).unwrap();
        assert!(matches!(book.diagnostics(), YaneuraOuBookDiagnostics::Unsorted { .. }));
        assert!(matches!(book.lookup_sfen(&format!("{A} 1")), Err(BookError::Unsupported(_))));
        assert_eq!(
            book.lookup_sfen_by_scan(&format!("{A} 1")).unwrap().unwrap().moves()[0].mv().to_usi(),
            "9f9e"
        );

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_yaneuraou_db_partial_validation_rejects_binary_lookup_until_full_validation() {
        let path = temp_book(&format!(
            "{HEADER}\n\
             sfen {A} 1\n\
             9f9e 8g7g 0 32 1\n\
             sfen {B} 78\n\
             4e4f 9c9d 120 40 2\n",
        ));
        let file_bytes = fs::metadata(&path).unwrap().len();
        let ordering = validate_ordering(&path, Some(1), file_bytes).unwrap();
        assert_eq!(ordering, YaneuraOuBookDiagnostics::Sorted { checked_rows: 1, complete: false });
        let mut book = YaneuraOuBook {
            path: path.clone(),
            file_bytes,
            ordering,
            lookup_mode: YaneuraOuLookupMode::RequireComplete,
        };

        assert!(matches!(
            book.lookup_sfen(&format!("{B} 1")),
            Err(BookError::Unsupported(message)) if message.contains("partially validated")
        ));
        assert_eq!(
            book.validate_full().unwrap(),
            YaneuraOuBookDiagnostics::Sorted { checked_rows: 2, complete: true }
        );
        assert_eq!(
            book.lookup_sfen(&format!("{B} 1")).unwrap().unwrap().moves()[0].mv().to_usi(),
            "4e4f"
        );

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_yaneuraou_db_assume_sorted_after_prefix_allows_binary_lookup() {
        let path = temp_book(&format!(
            "{HEADER}\n\
             sfen {A} 1\n\
             9f9e 8g7g 0 32 1\n\
             sfen {B} 78\n\
             4e4f 9c9d 120 40 2\n",
        ));
        let book = YaneuraOuBook::open_with_options(
            &path,
            YaneuraOuBookOpenOptions::with_access_mode(
                YaneuraOuAccessMode::AssumeSortedAfterPrefix { prefix_rows: 1 },
            ),
        )
        .unwrap();

        assert_eq!(
            book.diagnostics(),
            &YaneuraOuBookDiagnostics::Sorted { checked_rows: 1, complete: false }
        );
        assert_eq!(
            book.lookup_sfen(&format!("{B} 1")).unwrap().unwrap().moves()[0].mv().to_usi(),
            "4e4f"
        );

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_yaneuraou_db_assume_sorted_by_caller_reports_unvalidated() {
        let path = temp_book(&format!(
            "{HEADER}\n\
             sfen {A} 1\n\
             9f9e 8g7g 0 32 1\n",
        ));
        let book = YaneuraOuBook::open_with_options(
            &path,
            YaneuraOuBookOpenOptions::with_access_mode(YaneuraOuAccessMode::AssumeSortedByCaller),
        )
        .unwrap();

        assert_eq!(book.diagnostics(), &YaneuraOuBookDiagnostics::Unvalidated);
        assert_eq!(
            book.lookup_sfen(&format!("{A} 1")).unwrap().unwrap().moves()[0].mv().to_usi(),
            "9f9e"
        );

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_yaneuraou_db_scan_only_makes_lookup_sfen_scan() {
        let path = temp_book(&format!(
            "{HEADER}\n\
             sfen {B} 78\n\
             4e4f 9c9d 120 40 2\n\
             sfen {A} 1\n\
             9f9e 8g7g 0 32 1\n",
        ));
        let book = YaneuraOuBook::open_with_options(
            &path,
            YaneuraOuBookOpenOptions::with_access_mode(YaneuraOuAccessMode::ScanOnly),
        )
        .unwrap();

        assert!(matches!(book.diagnostics(), YaneuraOuBookDiagnostics::Unsorted { .. }));
        assert_eq!(
            book.lookup_sfen(&format!("{A} 1")).unwrap().unwrap().moves()[0].mv().to_usi(),
            "9f9e"
        );

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_yaneuraou_db_iter_entries_streams_opened_path() {
        let path = temp_book(&format!(
            "{HEADER}\n\
             sfen {A} 1\n\
             9f9e 8g7g 0 32 1\n\
             sfen {B} 78\n\
             #entry note\n\
             4e4f 9c9d 120 40 2\n",
        ));
        let book = YaneuraOuBook::open(&path).unwrap();
        let entries = book.iter_entries().unwrap().collect::<Result<Vec<_>, _>>().unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].sfen(), &format!("{A} 1"));
        assert_eq!(entries[0].moves()[0].mv().to_usi(), "9f9e");
        assert_eq!(entries[1].sfen(), &format!("{B} 1"));
        assert_eq!(entries[1].comment(), "entry note");

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_yaneuraou_db_full_validation_reports_progress_and_cancels() {
        let path = temp_book(&format!(
            "{HEADER}\n\
             sfen {A} 1\n\
             9f9e 8g7g 0 32 1\n\
             sfen {B} 78\n\
             4e4f 9c9d 120 40 2\n",
        ));
        let mut book = YaneuraOuBook::open(&path).unwrap();
        let mut progress = Vec::new();
        let result = book.validate_full_with_control(|step| {
            progress.push((step.processed_bytes(), step.total_bytes(), step.processed_rows()));
            BookControl::Cancel
        });

        assert!(matches!(result, Err(BookError::Cancelled)));
        assert!(!progress.is_empty());

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_yaneuraou_db_open_ignores_move_payload_errors_during_diagnostics() {
        let path = temp_book(&format!(
            "{HEADER}\n\
             sfen {A} 1\n\
             malformed-move-line\n\
             sfen {B} 78\n\
             4e4f 9c9d 120 40 2\n",
        ));
        let book = YaneuraOuBook::open(&path).unwrap();
        assert_eq!(
            book.diagnostics(),
            &YaneuraOuBookDiagnostics::Sorted { checked_rows: 2, complete: true }
        );
        assert!(matches!(
            book.lookup_sfen(&format!("{A} 1")),
            Err(BookError::InvalidData(message)) if message.contains("missing ponder move")
        ));
        assert_eq!(
            book.lookup_sfen(&format!("{B} 1")).unwrap().unwrap().moves()[0].mv().to_usi(),
            "4e4f"
        );

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_yaneuraou_db_binary_lookup_finds_first_position_with_trailing_comments() {
        // Regression: コメント行で position 行が疎になると、二分探索の probe が
        // 先頭 position 行の行頭にちょうど一致したときに discard_line がその行を
        // 飛ばし、先頭局面が見つからなくなる不具合があった。
        let path = temp_book(&format!(
            "{HEADER}\n\
             sfen {A} 1\n\
             9f9e 8g7g 0 32 1 #main move\n\
             #move note\n\
             sfen {B} 78\n\
             #entry note\n\
             4e4f 9c9d 120 40 2\n\
             6e6f 6g6h -32 32 0\n\
             sfen {C} 64\n\
             4e7h+ none 540 38 1\n",
        ));
        let book = YaneuraOuBook::open(&path).unwrap();
        assert!(matches!(
            book.diagnostics(),
            YaneuraOuBookDiagnostics::Sorted { complete: true, .. }
        ));
        // 二分探索でも全 position が見つかる。
        for sfen in [A, B, C] {
            assert!(
                book.lookup_sfen(&format!("{sfen} 1")).unwrap().is_some(),
                "binary lookup must find {sfen}"
            );
        }

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_yaneuraou_db_on_demand_rejects_invalid_header() {
        let path = temp_book(&format!("#YANEURAOU-DB2016 2.00\nsfen {A} 1\n"));
        let book = YaneuraOuBook::open(&path).unwrap();
        assert_eq!(book.diagnostics(), &YaneuraOuBookDiagnostics::InvalidHeader);
        assert!(matches!(book.lookup_sfen(&format!("{A} 1")), Err(BookError::InvalidFormat(_))));

        fs::remove_file(path).unwrap();
    }

    #[test]
    #[ignore = "requires RSSHOGI_LARGE_YANEURAOU_DB"]
    fn test_yaneuraou_db_large_local_smoke() {
        let path = std::env::var_os("RSSHOGI_LARGE_YANEURAOU_DB")
            .map(PathBuf::from)
            .expect("set RSSHOGI_LARGE_YANEURAOU_DB to a large DB2016 file");
        assert!(path.exists(), "missing local fixture: {}", path.display());
        let mut book = YaneuraOuBook::open(&path).expect("open large db");
        assert!(matches!(book.diagnostics(), YaneuraOuBookDiagnostics::Sorted { .. }));
        assert!(matches!(
            book.validate_full().expect("full ordering validation"),
            YaneuraOuBookDiagnostics::Sorted { complete: true, .. }
        ));
        let entry = book.lookup_sfen(InitialPosition::Standard.to_sfen()).expect("lookup hirate");
        assert!(entry.is_some(), "large YaneuraOu DB should contain hirate");
    }
}
