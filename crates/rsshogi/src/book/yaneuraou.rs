use std::cmp::Ordering;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::board::Position;
use crate::book::{BookControl, BookError};
use crate::types::Move;

pub const YANEURAOU_HEADER: &str = "#YANEURAOU-DB2016 1.00";
const DEFAULT_VALIDATION_LIMIT: usize = 10_000;

#[derive(Debug, Clone)]
pub struct YaneuraOuBook {
    path: PathBuf,
    diagnostics: YaneuraOuBookDiagnostics,
    options: YaneuraOuBookOpenOptions,
    total_bytes: u64,
}

#[derive(Debug)]
pub struct YaneuraOuBookEntries {
    reader: BufReader<File>,
    line_number: usize,
    current: Option<YaneuraOuBookEntry>,
    current_error: Option<BookError>,
    queued_error: Option<BookError>,
    last_error_sfen: Option<String>,
    finished: bool,
}

impl YaneuraOuBookEntries {
    fn finish_current(&mut self) -> Option<Result<YaneuraOuBookEntry, BookError>> {
        let entry = self.current.take()?;
        if let Some(error) = self.current_error.take() {
            self.last_error_sfen = Some(entry.sfen.clone());
            Some(Err(error))
        } else {
            self.last_error_sfen = None;
            Some(Ok(entry))
        }
    }
}

impl Iterator for YaneuraOuBookEntries {
    type Item = Result<YaneuraOuBookEntry, BookError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(error) = self.queued_error.take() {
            self.last_error_sfen = None;
            return Some(Err(error));
        }
        if self.finished {
            return None;
        }

        loop {
            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) => {
                    self.finished = true;
                    return self.finish_current();
                }
                Ok(_) => self.line_number += 1,
                Err(error) => {
                    self.finished = true;
                    return Some(Err(error.into()));
                }
            }
            let line = clean_line(&line, self.line_number);
            if line.trim().is_empty() || is_header(line, self.line_number) {
                continue;
            }
            if let Some(comment) = strip_comment(line) {
                if let Some(entry) = self.current.as_mut() {
                    if let Some(book_move) = entry.moves.last_mut() {
                        append_comment(&mut book_move.comment, comment);
                    } else {
                        append_comment(&mut entry.comment, comment);
                    }
                }
                continue;
            }
            if let Some(sfen) = line.strip_prefix("sfen ") {
                match parse_position(sfen) {
                    Ok((normalized, min_ply, original_ply)) => {
                        let next = YaneuraOuBookEntry {
                            sfen: normalized,
                            min_ply,
                            original_ply,
                            comment: String::new(),
                            moves: Vec::new(),
                        };
                        if self.current.is_some() {
                            let completed = self.finish_current();
                            self.current = Some(next);
                            return completed;
                        }
                        self.current = Some(next);
                    }
                    Err(error) => {
                        if self.current.is_some() {
                            let completed = self.finish_current();
                            self.queued_error = Some(error);
                            return completed;
                        }
                        return Some(Err(error));
                    }
                }
                continue;
            }
            let Some(entry) = self.current.as_mut() else {
                return Some(Err(BookError::InvalidData(format!(
                    "DB2016 data before first position at line {}",
                    self.line_number
                ))));
            };
            match parse_move(line) {
                Ok(book_move) => entry.moves.push(book_move),
                Err(error) => {
                    if self.current_error.is_none() {
                        self.current_error = Some(error);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YaneuraOuBookOpenOptions {
    access_mode: YaneuraOuAccessMode,
}

impl YaneuraOuBookOpenOptions {
    #[must_use]
    pub const fn new() -> Self {
        Self::with_access_mode(YaneuraOuAccessMode::SafeBinary { prefix_rows: 10_000 })
    }

    #[must_use]
    pub const fn with_access_mode(access_mode: YaneuraOuAccessMode) -> Self {
        Self { access_mode }
    }

    #[must_use]
    pub const fn access_mode(self) -> YaneuraOuAccessMode {
        self.access_mode
    }
}

impl Default for YaneuraOuBookOpenOptions {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YaneuraOuAccessMode {
    SafeBinary { prefix_rows: usize },
    ValidateFullBeforeLookup,
    AssumeSortedAfterPrefix { prefix_rows: usize },
    AssumeSortedByCaller,
    ScanOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YaneuraOuValidationProgress {
    processed_bytes: u64,
    total_bytes: u64,
    processed_rows: u64,
}

impl YaneuraOuValidationProgress {
    #[must_use]
    pub const fn processed_bytes(self) -> u64 {
        self.processed_bytes
    }

    #[must_use]
    pub const fn total_bytes(self) -> u64 {
        self.total_bytes
    }

    #[must_use]
    pub const fn processed_rows(self) -> u64 {
        self.processed_rows
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YaneuraOuBookDiagnostics {
    Sorted {
        checked_rows: usize,
        complete: bool,
    },
    Unsorted {
        checked_rows: usize,
        line_number: usize,
        previous_sfen: String,
        current_sfen: String,
    },
    InvalidHeader,
    Unvalidated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YaneuraOuBookEntry {
    sfen: String,
    min_ply: u32,
    original_ply: Option<u32>,
    comment: String,
    moves: Vec<YaneuraOuBookMove>,
}

impl YaneuraOuBookEntry {
    #[must_use]
    pub fn sfen(&self) -> &str {
        &self.sfen
    }

    #[must_use]
    pub const fn min_ply(&self) -> u32 {
        self.min_ply
    }

    #[must_use]
    pub(crate) const fn original_ply(&self) -> Option<u32> {
        self.original_ply
    }

    #[must_use]
    pub fn comment(&self) -> &str {
        &self.comment
    }

    #[must_use]
    pub fn moves(&self) -> &[YaneuraOuBookMove] {
        &self.moves
    }
}

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
    #[must_use]
    pub const fn mv(&self) -> Move {
        self.mv
    }

    #[must_use]
    pub const fn ponder(&self) -> Move {
        self.ponder
    }

    #[must_use]
    pub const fn score(&self) -> Option<i32> {
        self.score
    }

    #[must_use]
    pub const fn depth(&self) -> Option<u32> {
        self.depth
    }

    #[must_use]
    pub const fn count(&self) -> Option<u64> {
        self.count
    }

    #[must_use]
    pub fn comment(&self) -> &str {
        &self.comment
    }
}

impl YaneuraOuBook {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, BookError> {
        Self::open_with_options(path, YaneuraOuBookOpenOptions::new())
    }

    pub fn open_with_options(
        path: impl AsRef<Path>,
        options: YaneuraOuBookOpenOptions,
    ) -> Result<Self, BookError> {
        let path = path.as_ref().to_path_buf();
        let total_bytes = fs::metadata(&path)?.len();
        validate_header(&path)?;
        let mut book =
            Self { path, diagnostics: YaneuraOuBookDiagnostics::Unvalidated, options, total_bytes };
        match options.access_mode() {
            YaneuraOuAccessMode::SafeBinary { prefix_rows }
            | YaneuraOuAccessMode::AssumeSortedAfterPrefix { prefix_rows } => {
                book.diagnostics =
                    book.scan_ordering(Some(prefix_rows), |_| BookControl::Continue)?;
            }
            YaneuraOuAccessMode::ValidateFullBeforeLookup => {
                let _ = book.validate_full()?;
            }
            YaneuraOuAccessMode::ScanOnly => {
                book.diagnostics =
                    book.scan_ordering(Some(DEFAULT_VALIDATION_LIMIT), |_| BookControl::Continue)?;
            }
            YaneuraOuAccessMode::AssumeSortedByCaller => {}
        }
        Ok(book)
    }

    #[must_use]
    pub fn diagnostics(&self) -> &YaneuraOuBookDiagnostics {
        &self.diagnostics
    }

    pub fn validate_full(&mut self) -> Result<YaneuraOuBookDiagnostics, BookError> {
        self.validate_full_with_control(|_| BookControl::Continue)
    }

    pub fn validate_full_with_control(
        &mut self,
        on_progress: impl FnMut(YaneuraOuValidationProgress) -> BookControl,
    ) -> Result<YaneuraOuBookDiagnostics, BookError> {
        let diagnostics = self.scan_ordering(None, on_progress)?;
        self.diagnostics = diagnostics.clone();
        Ok(diagnostics)
    }

    pub fn lookup_sfen(&self, sfen: &str) -> Result<Option<YaneuraOuBookEntry>, BookError> {
        let normalized = normalize_sfen(sfen)?;
        if self.options.access_mode() == YaneuraOuAccessMode::ScanOnly {
            return self.lookup_normalized_by_scan(&normalized);
        }
        match &self.diagnostics {
            YaneuraOuBookDiagnostics::Sorted { complete: true, .. } => {
                self.lookup_normalized_by_binary(&normalized)
            }
            YaneuraOuBookDiagnostics::Sorted { complete: false, .. }
                if !matches!(
                    self.options.access_mode(),
                    YaneuraOuAccessMode::SafeBinary { .. }
                ) =>
            {
                self.lookup_normalized_by_binary(&normalized)
            }
            YaneuraOuBookDiagnostics::Sorted { complete: false, .. } => {
                Err(BookError::Unsupported(
                    "DB2016 ordering is only partially validated; validate fully or scan explicitly",
                ))
            }
            YaneuraOuBookDiagnostics::Unsorted { .. } => {
                Err(BookError::Unsupported("DB2016 is not sorted; scan explicitly"))
            }
            YaneuraOuBookDiagnostics::InvalidHeader => {
                Err(BookError::InvalidFormat("unsupported DB2016 header"))
            }
            YaneuraOuBookDiagnostics::Unvalidated => self.lookup_normalized_by_binary(&normalized),
        }
    }

    pub fn lookup_position(
        &self,
        position: &Position,
    ) -> Result<Option<YaneuraOuBookEntry>, BookError> {
        self.lookup_sfen(&position.to_sfen(Some(1)))
    }

    pub fn lookup_sfen_by_scan(&self, sfen: &str) -> Result<Option<YaneuraOuBookEntry>, BookError> {
        self.lookup_normalized_by_scan(&normalize_sfen(sfen)?)
    }

    pub fn iter_entries(&self) -> Result<YaneuraOuBookEntries, BookError> {
        Ok(YaneuraOuBookEntries {
            reader: BufReader::new(File::open(&self.path)?),
            line_number: 0,
            current: None,
            current_error: None,
            queued_error: None,
            last_error_sfen: None,
            finished: false,
        })
    }

    fn lookup_normalized_by_scan(
        &self,
        normalized: &str,
    ) -> Result<Option<YaneuraOuBookEntry>, BookError> {
        let mut entries = self.iter_entries()?;
        while let Some(result) = entries.next() {
            match result {
                Ok(entry) if entry.sfen == normalized => return Ok(Some(entry)),
                Ok(_) => {}
                Err(error) if entries.last_error_sfen.as_deref() == Some(normalized) => {
                    return Err(error);
                }
                Err(_) => {}
            }
        }
        Ok(None)
    }

    fn lookup_normalized_by_binary(
        &self,
        normalized: &str,
    ) -> Result<Option<YaneuraOuBookEntry>, BookError> {
        let mut reader = BufReader::new(File::open(&self.path)?);
        let mut low = 0u64;
        let mut high = self.total_bytes;
        while high.saturating_sub(low) > 64 * 1024 {
            let midpoint = low + (high - low) / 2;
            let Some((offset, current)) = find_next_position(&mut reader, midpoint)? else {
                high = midpoint;
                continue;
            };
            match current.as_bytes().cmp(normalized.as_bytes()) {
                Ordering::Less => low = offset.saturating_add(1),
                Ordering::Equal => return self.read_entry_at(offset).map(Some),
                Ordering::Greater => high = midpoint,
            }
        }

        let mut offset = low;
        while let Some((current_offset, current)) = find_next_position(&mut reader, offset)? {
            if current_offset >= high && high < self.total_bytes {
                return Ok(None);
            }
            match current.as_bytes().cmp(normalized.as_bytes()) {
                Ordering::Less => offset = current_offset.saturating_add(1),
                Ordering::Equal => return self.read_entry_at(current_offset).map(Some),
                Ordering::Greater => return Ok(None),
            }
        }
        Ok(None)
    }

    fn read_entry_at(&self, offset: u64) -> Result<YaneuraOuBookEntry, BookError> {
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(offset))?;
        let mut entries = YaneuraOuBookEntries {
            reader: BufReader::new(file),
            line_number: 1,
            current: None,
            current_error: None,
            queued_error: None,
            last_error_sfen: None,
            finished: false,
        };
        entries.next().ok_or(BookError::InvalidFormat("missing DB2016 entry at indexed offset"))?
    }

    fn scan_ordering(
        &self,
        limit: Option<usize>,
        mut on_progress: impl FnMut(YaneuraOuValidationProgress) -> BookControl,
    ) -> Result<YaneuraOuBookDiagnostics, BookError> {
        let mut reader = BufReader::new(File::open(&self.path)?);
        let mut line_number = 0usize;
        let mut consumed_bytes = 0u64;
        let mut checked_rows = 0usize;
        let mut previous: Option<String> = None;
        loop {
            let mut line = String::new();
            let bytes = reader.read_line(&mut line)?;
            if bytes == 0 {
                return Ok(YaneuraOuBookDiagnostics::Sorted { checked_rows, complete: true });
            }
            line_number += 1;
            consumed_bytes += bytes as u64;
            let line = clean_line(&line, line_number);
            let Some(sfen) = line.strip_prefix("sfen ") else {
                continue;
            };
            if limit.is_some_and(|limit| checked_rows >= limit) {
                return Ok(YaneuraOuBookDiagnostics::Sorted { checked_rows, complete: false });
            }
            let (current, _, _) = parse_position(sfen)?;
            checked_rows += 1;
            if on_progress(YaneuraOuValidationProgress {
                processed_bytes: consumed_bytes,
                total_bytes: self.total_bytes,
                processed_rows: checked_rows as u64,
            }) == BookControl::Cancel
            {
                return Err(BookError::Cancelled);
            }
            if let Some(previous) = previous.as_ref()
                && previous.as_bytes() >= current.as_bytes()
            {
                return Ok(YaneuraOuBookDiagnostics::Unsorted {
                    checked_rows,
                    line_number,
                    previous_sfen: previous.clone(),
                    current_sfen: current,
                });
            }
            previous = Some(current);
            if limit.is_some_and(|limit| checked_rows >= limit) {
                return Ok(YaneuraOuBookDiagnostics::Sorted { checked_rows, complete: false });
            }
        }
    }
}

fn find_next_position(
    reader: &mut BufReader<File>,
    offset: u64,
) -> Result<Option<(u64, String)>, BookError> {
    reader.seek(SeekFrom::Start(offset))?;
    if offset > 0 {
        reader.seek(SeekFrom::Start(offset - 1))?;
        let mut previous = [0u8; 1];
        reader.read_exact(&mut previous)?;
        reader.seek(SeekFrom::Start(offset))?;
        if previous[0] != b'\n' {
            let mut partial = String::new();
            let _ = reader.read_line(&mut partial)?;
        }
    }
    loop {
        let line_offset = reader.stream_position()?;
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let line = clean_line(&line, usize::from(line_offset == 0));
        if let Some(sfen) = line.strip_prefix("sfen ") {
            let (normalized, _, _) = parse_position(sfen)?;
            return Ok(Some((line_offset, normalized)));
        }
    }
}

fn validate_header(path: &Path) -> Result<(), BookError> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(());
    }
    let line = clean_line(&line, 1);
    if line.starts_with("#YANEURAOU-DB2016") && line != YANEURAOU_HEADER {
        return Err(BookError::Unsupported("unsupported DB2016 header"));
    }
    Ok(())
}

fn clean_line(line: &str, line_number: usize) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line).trim_end_matches('\r');
    if line_number == 1 { line.strip_prefix('\u{feff}').unwrap_or(line) } else { line }
}

fn is_header(line: &str, line_number: usize) -> bool {
    line_number == 1 && line == YANEURAOU_HEADER
}

fn normalize_sfen(sfen: &str) -> Result<String, BookError> {
    parse_position(sfen).map(|(normalized, _, _)| normalized)
}

fn parse_position(value: &str) -> Result<(String, u32, Option<u32>), BookError> {
    let fields: Vec<_> = value.split_ascii_whitespace().collect();
    if fields.len() < 3 {
        return Err(BookError::InvalidData(
            "DB2016 position has fewer than three SFEN fields".into(),
        ));
    }
    let normalized_input = format!("{} {} {} 1", fields[0], fields[1], fields[2]);
    let position = Position::from_sfen(&normalized_input)
        .map_err(|error| BookError::InvalidData(format!("invalid DB2016 SFEN: {error}")))?;
    let original_ply = match fields.get(3) {
        Some(field) => {
            Some(field.parse().map_err(|_| BookError::InvalidData("invalid DB2016 ply".into()))?)
        }
        None => Some(0),
    };
    let min_ply = original_ply.unwrap_or(0);
    Ok((position.to_sfen(Some(1)), min_ply, original_ply))
}

fn parse_move(line: &str) -> Result<YaneuraOuBookMove, BookError> {
    let mut rest = line.trim_start();
    let mut tokens = Vec::new();
    while tokens.len() < 5 && !rest.is_empty() && strip_comment(rest).is_none() {
        let (token, next) = take_token(rest);
        tokens.push(token);
        rest = next;
    }
    if tokens.first().is_some_and(|token| *token == "move") {
        tokens.remove(0);
        while tokens.len() < 5 && !rest.is_empty() && strip_comment(rest).is_none() {
            let (token, next) = take_token(rest);
            tokens.push(token);
            rest = next;
        }
    }
    let mv = parse_move_token(
        tokens.first().copied().ok_or(BookError::InvalidData("missing DB2016 move".into()))?,
    )?;
    let ponder = tokens
        .get(1)
        .map_or(Ok(Move::MOVE_NONE), |token| parse_move_token(token))
        .map_err(|_| BookError::InvalidData("invalid DB2016 ponder move".into()))?;
    let score = parse_optional(tokens.get(2).copied())?;
    let depth = parse_optional(tokens.get(3).copied())?;
    let count = parse_optional(tokens.get(4).copied())?;
    Ok(YaneuraOuBookMove {
        mv,
        ponder,
        score,
        depth,
        count,
        comment: strip_comment(rest).unwrap_or(rest).to_string(),
    })
}

fn take_token(value: &str) -> (&str, &str) {
    let value = value.trim_start();
    match value.find(char::is_whitespace) {
        Some(index) => (&value[..index], value[index..].trim_start()),
        None => (value, ""),
    }
}

fn parse_move_token(token: &str) -> Result<Move, BookError> {
    if is_none(token) {
        Ok(Move::MOVE_NONE)
    } else if token == "resign" {
        Ok(Move::MOVE_RESIGN)
    } else {
        let mv = Move::from_usi(token)
            .ok_or_else(|| BookError::InvalidData("invalid DB2016 move".into()))?;
        if is_valid_db2016_move(mv) {
            Ok(mv)
        } else {
            Err(BookError::InvalidData("invalid DB2016 move".into()))
        }
    }
}

fn is_valid_db2016_move(mv: Move) -> bool {
    if !mv.is_normal() || mv.to_sq().raw() < 0 || mv.to_sq().raw() >= 81 {
        return false;
    }
    if mv.is_drop() {
        !mv.is_promotion() && matches!((mv.raw() >> 7) & 0x7f, 1..=7)
    } else {
        mv.from_sq().raw() >= 0 && mv.from_sq().raw() < 81 && mv.from_sq() != mv.to_sq()
    }
}

fn parse_optional<T: std::str::FromStr>(token: Option<&str>) -> Result<Option<T>, BookError> {
    match token {
        None | Some("none" | "None") => Ok(None),
        Some(token) => token
            .parse()
            .map(Some)
            .map_err(|_| BookError::InvalidData("invalid DB2016 numeric field".into())),
    }
}

fn is_none(value: &str) -> bool {
    value == "none" || value == "None"
}

fn strip_comment(line: &str) -> Option<&str> {
    line.strip_prefix("//").or_else(|| line.strip_prefix('#')).map(str::trim_start)
}

fn append_comment(destination: &mut String, comment: &str) {
    if !destination.is_empty() {
        destination.push('\n');
    }
    destination.push_str(comment);
}
