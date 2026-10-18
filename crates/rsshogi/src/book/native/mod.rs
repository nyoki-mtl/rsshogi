//! 編集原本とエンジン参照を共有する Native Book v4。

mod block;
mod builder;
mod convert;
mod crc;
mod editor;
mod reader;
mod types;
mod wire;
#[cfg(test)]
mod tests;

pub use builder::NativeBookBuilder;
pub use convert::{NativeBookImportIssue, NativeBookImportOptions, NativeBookImportReport};
pub use editor::{NativeBookEditedRecords, NativeBookEditor, NativeBookSalvageReport};
pub use reader::{NativeBook, NativeBookEntries, NativeBookOpenOptions, NativeBookRecords};
pub use types::{NativeBookEntry, NativeBookMetadata, NativeBound, NativeCandidate, NativeScore};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::book::BookError;

/// コメントを含む1局面の所有snapshot。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeBookPosition {
    /// 局面キーと検索用候補手。
    pub entry: NativeBookEntry,
    /// 局面コメント。
    pub comment: Option<String>,
    /// 指し手raw値をキーとする候補手コメント。
    pub move_comments: BTreeMap<u16, String>,
}

impl NativeBookPosition {
    /// コメントなしの局面を作る。
    pub fn new(entry: NativeBookEntry) -> Self {
        Self { entry, comment: None, move_comments: BTreeMap::new() }
    }

    pub(super) fn normalized(mut self) -> Result<Self, BookError> {
        self.entry.validate()?;
        self.comment = types::normalize_text(self.comment)?;
        let mut comments = BTreeMap::new();
        for (mv, text) in self.move_comments {
            if !self.entry.candidates.iter().any(|candidate| candidate.mv.raw() == mv) {
                return Err(BookError::InvalidData("comment references absent move".into()));
            }
            if let Some(text) = types::normalize_text(Some(text))? {
                comments.insert(mv, text);
            }
        }
        self.move_comments = comments;
        Ok(self)
    }
}

/// 処理phaseごとの進捗。総量が不明ならtotalはNone。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeBookProgress {
    /// 現在の処理。
    pub phase: &'static str,
    /// 処理済みbyte数または件数。
    pub completed: u64,
    /// 当該phaseの総量。
    pub total: Option<u64>,
}

/// I/O要求の累計。OSの物理ディスクI/Oとは異なる。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeBookIoStats {
    /// read-at相当の回数。
    pub read_calls: u64,
    /// 要求したbyte数。
    pub read_bytes: u64,
    /// 標準 File API 境界での seek 呼出し数。物理 I/O 数ではない。
    pub seek_calls: u64,
    /// 標準 File API 境界での read 呼出し数。短い読込と再試行を含む。
    pub read_syscalls: u64,
}

/// 正常な開始ヘッダーに続く未完了tail。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeBookTail {
    /// 未完了frameの開始。
    pub offset: u64,
    /// open時点のファイル末尾。
    pub end: u64,
}

pub(super) fn invalid(reason: &'static str) -> BookError {
    BookError::InvalidFormat(reason)
}

pub(super) fn add(a: u64, b: u64) -> Result<u64, BookError> {
    a.checked_add(b).ok_or_else(|| invalid("native book size overflow"))
}

pub(super) fn mul(a: u64, b: u64) -> Result<u64, BookError> {
    a.checked_mul(b).ok_or_else(|| invalid("native book size overflow"))
}

static TEMP_ID: AtomicU64 = AtomicU64::new(0);

pub(super) fn temporary_path(path: &Path, label: &str) -> PathBuf {
    let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!(".{name}.{label}.{}.{id}.tmp", std::process::id()))
}

pub(super) fn sync_parent(path: &Path) -> Result<(), BookError> {
    #[cfg(unix)]
    std::fs::File::open(
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new(".")),
    )?
    .sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
