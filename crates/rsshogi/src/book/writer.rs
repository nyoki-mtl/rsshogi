use std::collections::{HashMap, HashSet};
use std::io::Write;

use crate::book::{BookCandidate, BookDatabase, BookDatabaseEntry, BookError};

const HEADER: &str = "#YANEURAOU-DB2016 1.00";

/// DB2016 出力の候補手順、手数、重複処理と件数ヘッダーの設定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YaneuraOuDb2016WriteOptions {
    preserve_move_order: bool,
    fixed_ply: Option<u32>,
    omit_ply: bool,
    keep_last_on_duplicate: bool,
    emit_noe: bool,
}

impl YaneuraOuDb2016WriteOptions {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            preserve_move_order: false,
            fixed_ply: None,
            omit_ply: false,
            keep_last_on_duplicate: false,
            emit_noe: false,
        }
    }

    #[must_use]
    pub const fn with_preserved_move_order(mut self) -> Self {
        self.preserve_move_order = true;
        self
    }

    #[must_use]
    pub const fn with_fixed_ply(mut self, ply: u32) -> Self {
        self.fixed_ply = Some(ply);
        self
    }

    #[must_use]
    pub const fn with_omitted_ply(mut self) -> Self {
        self.omit_ply = true;
        self
    }

    #[must_use]
    pub const fn with_keep_last_on_duplicate(mut self) -> Self {
        self.keep_last_on_duplicate = true;
        self
    }

    #[must_use]
    pub const fn with_noe(mut self, emit: bool) -> Self {
        self.emit_noe = emit;
        self
    }
}

impl Default for YaneuraOuDb2016WriteOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// 正規化 SFEN の昇順で DB2016 エントリを逐次出力する writer。
///
/// メモリ使用量は最大エントリのサイズに比例する。I/O エラー後は再利用できない。
/// コメントの CRLF と単独 CR は LF に正規化し、各行をコメントとして出力する。
pub struct YaneuraOuDb2016Writer<W: Write> {
    writer: W,
    options: YaneuraOuDb2016WriteOptions,
    expected_entry_count: Option<u64>,
    entry_count: u64,
    previous_sfen: Option<String>,
    candidate_moves: HashSet<crate::types::Move>,
    buffer: String,
    poisoned: bool,
}

impl<W: Write> YaneuraOuDb2016Writer<W> {
    /// ヘッダーと、指定された場合は NOE を直ちに出力する。
    ///
    /// # Errors
    ///
    /// keep-last が指定された場合、NOE に件数がない場合、I/O 失敗時に返す。
    pub fn new(
        mut writer: W,
        options: YaneuraOuDb2016WriteOptions,
        expected_entry_count: Option<u64>,
    ) -> Result<Self, BookError> {
        if options.keep_last_on_duplicate {
            return Err(BookError::Unsupported(
                "streaming DB2016 writer cannot keep last duplicate",
            ));
        }
        if options.emit_noe && expected_entry_count.is_none() {
            return Err(BookError::InvalidData("DB2016 NOE requires expected entry count".into()));
        }
        write_header(&mut writer, &options, expected_entry_count.unwrap_or(0))?;
        Ok(Self {
            writer,
            options,
            expected_entry_count,
            entry_count: 0,
            previous_sfen: None,
            candidate_moves: HashSet::new(),
            buffer: String::new(),
            poisoned: false,
        })
    }

    /// 1 エントリ全体を検証してから出力する。
    ///
    /// # Errors
    ///
    /// 件数超過、局面の重複または逆順、候補手の重複や不正な符号化、I/O 失敗時に返す。
    /// 検証エラーは出力と内部件数を変更しない。I/O 失敗後の操作もエラーになる。
    pub fn write_entry(&mut self, entry: &BookDatabaseEntry) -> Result<(), BookError> {
        self.ensure_usable()?;
        if self.expected_entry_count.is_some_and(|count| self.entry_count >= count) {
            return Err(BookError::InvalidData("DB2016 expected entry count exceeded".into()));
        }
        let sfen = entry.position().sfen();
        if self
            .previous_sfen
            .as_deref()
            .is_some_and(|previous| previous.as_bytes() >= sfen.as_bytes())
        {
            return Err(BookError::InvalidData(
                "DB2016 positions must be strictly increasing".into(),
            ));
        }
        self.candidate_moves.clear();
        for candidate in entry.candidates() {
            if !self.candidate_moves.insert(candidate.mv()) {
                return Err(BookError::InvalidData(format!(
                    "duplicate book database candidate: {}",
                    candidate.mv().to_usi()
                )));
            }
        }
        self.buffer.clear();
        write_entry(&mut self.buffer, entry, &self.options)?;
        if let Err(error) = self.writer.write_all(self.buffer.as_bytes()) {
            self.poisoned = true;
            return Err(error.into());
        }
        sfen.clone_into(self.previous_sfen.get_or_insert_default());
        self.entry_count += 1;
        Ok(())
    }

    /// 件数を検査し、flush 後に出力先を返す。
    ///
    /// # Errors
    ///
    /// 件数不一致、I/O 失敗、または先行する I/O 失敗がある場合に返す。
    pub fn finish(mut self) -> Result<W, BookError> {
        self.ensure_usable()?;
        if self.expected_entry_count.is_some_and(|count| self.entry_count != count) {
            return Err(BookError::InvalidData("DB2016 expected entry count mismatch".into()));
        }
        self.writer.flush()?;
        Ok(self.writer)
    }

    fn ensure_usable(&self) -> Result<(), BookError> {
        if self.poisoned {
            return Err(BookError::InvalidData(
                "DB2016 writer is poisoned by an I/O failure".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn write_preamble_comment(&mut self, comment: &str) -> Result<(), BookError> {
        self.ensure_usable()?;
        if let Err(error) = writeln!(self.writer, "# {comment}") {
            self.poisoned = true;
            return Err(error.into());
        }
        Ok(())
    }
}

impl BookDatabase {
    /// 全件の形式検証と整列後に、DB2016 をエントリ単位で書き出す。
    ///
    /// # Errors
    /// 重複局面、不正な候補手または I/O 失敗時に返す。
    pub fn write_yaneuraou_db2016(
        &self,
        w: &mut impl Write,
        options: &YaneuraOuDb2016WriteOptions,
    ) -> Result<(), BookError> {
        let entries = sorted_entries(self, options)?;
        let mut buffer = String::new();
        // 出力先を変更する前に、全エントリの形式検証を済ませる。
        for entry in &entries {
            buffer.clear();
            write_entry(&mut buffer, entry, options)?;
        }
        write_header(w, options, entries.len() as u64)?;
        for entry in entries {
            buffer.clear();
            write_entry(&mut buffer, entry, options)?;
            w.write_all(buffer.as_bytes())?;
        }
        Ok(())
    }

    /// 全件を整列した DB2016 の文字列を返す。
    ///
    /// # Errors
    /// 重複局面または不正な候補手がある場合に返す。
    pub fn to_yaneuraou_db2016_string(
        &self,
        options: &YaneuraOuDb2016WriteOptions,
    ) -> Result<String, BookError> {
        let entries = sorted_entries(self, options)?;
        let mut output = String::new();
        output.push_str(HEADER);
        output.push('\n');
        if options.emit_noe {
            output.push_str(&format!("# NOE:{}\n", entries.len()));
        }
        for entry in entries {
            write_entry(&mut output, entry, options)?;
        }
        Ok(output)
    }
}

fn write_header(
    writer: &mut impl Write,
    options: &YaneuraOuDb2016WriteOptions,
    entry_count: u64,
) -> Result<(), BookError> {
    writeln!(writer, "{HEADER}")?;
    if options.emit_noe {
        writeln!(writer, "# NOE:{entry_count}")?;
    }
    Ok(())
}

fn sorted_entries<'a>(
    database: &'a BookDatabase,
    options: &YaneuraOuDb2016WriteOptions,
) -> Result<Vec<&'a BookDatabaseEntry>, BookError> {
    let mut by_sfen: HashMap<&str, &BookDatabaseEntry> = HashMap::new();
    for entry in database.entries() {
        let key = entry.position().sfen();
        if by_sfen.contains_key(key) && !options.keep_last_on_duplicate {
            return Err(BookError::InvalidData(format!("duplicate DB2016 position: {key}")));
        }
        by_sfen.insert(key, entry);
    }

    let mut entries: Vec<_> = by_sfen.into_values().collect();
    entries.sort_by(|left, right| {
        left.position().sfen().as_bytes().cmp(right.position().sfen().as_bytes())
    });

    Ok(entries)
}

fn write_entry(
    output: &mut String,
    entry: &BookDatabaseEntry,
    options: &YaneuraOuDb2016WriteOptions,
) -> Result<(), BookError> {
    let position = entry.position();
    let mut fields = position.sfen().split_ascii_whitespace();
    let board = fields.next().ok_or(BookError::InvalidData("missing normalized board".into()))?;
    let side = fields.next().ok_or(BookError::InvalidData("missing normalized side".into()))?;
    let hands = fields.next().ok_or(BookError::InvalidData("missing normalized hands".into()))?;
    output.push_str("sfen ");
    output.push_str(board);
    output.push(' ');
    output.push_str(side);
    output.push(' ');
    output.push_str(hands);
    if !options.omit_ply
        && let Some(ply) = options.fixed_ply.or(position.original_ply())
    {
        output.push(' ');
        output.push_str(&ply.to_string());
    }
    output.push('\n');

    if let Some(metadata) = entry.metadata().yaneuraou() {
        write_comments(output, metadata.comment());
    }

    if options.preserve_move_order {
        return write_candidates(output, entry.candidates().iter());
    }
    let mut candidates: Vec<_> = entry.candidates().iter().collect();
    candidates.sort_by(|left, right| match (left.score(), right.score()) {
        (Some(left), Some(right)) => right.cmp(&left),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    write_candidates(output, candidates.into_iter())
}

fn write_candidates<'a>(
    output: &mut String,
    candidates: impl Iterator<Item = &'a BookCandidate>,
) -> Result<(), BookError> {
    for candidate in candidates {
        if !is_valid_db2016_move(candidate.mv()) {
            return Err(BookError::InvalidData(format!(
                "invalid DB2016 move: {:#06x}",
                candidate.mv().raw()
            )));
        }
        if candidate.ponder().is_some_and(|ponder| !is_valid_db2016_move(ponder)) {
            return Err(BookError::InvalidData("invalid DB2016 ponder move".into()));
        }
        output.push_str(&candidate.mv().to_usi());
        output.push(' ');
        output.push_str(&candidate.ponder().map_or_else(|| "none".to_string(), |mv| mv.to_usi()));
        output.push(' ');
        write_optional(output, candidate.score());
        output.push(' ');
        write_optional(output, candidate.depth());
        if let Some(count) = candidate.count() {
            output.push(' ');
            output.push_str(&count.to_string());
        }
        output.push('\n');
        if let Some(metadata) = candidate.metadata().yaneuraou() {
            write_comments(output, metadata.comment());
        }
    }
    Ok(())
}

fn write_optional<T: std::fmt::Display>(output: &mut String, value: Option<T>) {
    match value {
        Some(value) => output.push_str(&value.to_string()),
        None => output.push_str("none"),
    }
}

fn write_comments(output: &mut String, mut comment: &str) {
    if comment.is_empty() {
        return;
    }
    loop {
        let end = comment.find(['\r', '\n']).unwrap_or(comment.len());
        output.push_str("# ");
        output.push_str(&comment[..end]);
        output.push('\n');
        if end == comment.len() {
            break;
        }
        let newline_len = if comment[end..].starts_with("\r\n") { 2 } else { 1 };
        comment = &comment[end + newline_len..];
    }
}

fn is_valid_db2016_move(mv: crate::types::Move) -> bool {
    if !mv.is_normal() || mv.to_sq().raw() < 0 || mv.to_sq().raw() >= 81 {
        return false;
    }
    if mv.is_drop() {
        !mv.is_promotion() && matches!((mv.raw() >> 7) & 0x7f, 1..=7)
    } else {
        mv.from_sq().raw() >= 0 && mv.from_sq().raw() < 81 && mv.from_sq() != mv.to_sq()
    }
}
