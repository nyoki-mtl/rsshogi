//! 局面単位の差分集合と編集保存の進捗。

use std::collections::{BTreeMap, HashSet};

use crate::book::{BookControl, BookDatabaseEntry, BookError, BookPosition};

/// 正規化 SFEN をキーとする局面単位の差分集合。
///
/// 同じ局面への最後の操作を保持する。保存の成功、失敗、キャンセルで内容は変化しない。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BookDatabasePatch {
    pub(crate) entries: BTreeMap<String, BookDatabaseEdit>,
}

/// 局面の置換または削除。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookDatabaseEdit {
    /// 候補手とメタデータを含む局面全体の置換。
    Upsert(Box<BookDatabaseEntry>),
    /// 元ファイルに存在しない局面にも記録できる削除指定。
    Delete,
}

impl BookDatabasePatch {
    /// 空の差分集合を生成する。
    #[must_use]
    pub const fn new() -> Self {
        Self { entries: BTreeMap::new() }
    }

    /// 局面と候補手を検査して置換を記録する。
    ///
    /// # Errors
    /// 不正な局面、候補手の重複、不正な指し手符号化は `InvalidData` を返す。
    /// 失敗時は以前の差分を保持する。形式固有の表現可能性は保存時に検査する。
    pub fn upsert_entry(&mut self, entry: BookDatabaseEntry) -> Result<(), BookError> {
        validate_patch_entry(&entry)?;
        self.entries
            .insert(entry.position().sfen().to_owned(), BookDatabaseEdit::Upsert(Box::new(entry)));
        Ok(())
    }

    /// 手数を除いて正規化した SFEN に削除を記録する。
    ///
    /// # Errors
    /// SFEN が不正なら `InvalidData` を返し、以前の差分を保持する。
    pub fn remove_entry_by_sfen(&mut self, sfen: &str) -> Result<(), BookError> {
        let position = BookPosition::from_sfen(sfen, None)?;
        self.entries.insert(position.sfen().to_owned(), BookDatabaseEdit::Delete);
        Ok(())
    }

    /// 手数を除いて正規化した SFEN の差分を返す。
    ///
    /// # Errors
    /// SFEN が不正なら `InvalidData` を返す。
    pub fn edit_by_sfen(&self, sfen: &str) -> Result<Option<&BookDatabaseEdit>, BookError> {
        let position = BookPosition::from_sfen(sfen, None)?;
        Ok(self.entries.get(position.sfen()))
    }
}

pub(crate) fn validate_patch_entry(entry: &BookDatabaseEntry) -> Result<(), BookError> {
    let position = BookPosition::from_sfen(entry.position().sfen(), None)?;
    if position.sfen() != entry.position().sfen() {
        return Err(BookError::InvalidData("book patch position is not normalized".into()));
    }
    let mut moves = HashSet::new();
    for candidate in entry.candidates() {
        if !candidate.mv().is_normal()
            || candidate.ponder().is_some_and(|ponder| !ponder.is_normal())
        {
            return Err(BookError::InvalidData("invalid book patch move encoding".into()));
        }
        if !moves.insert(candidate.mv()) {
            return Err(BookError::InvalidData("duplicate book patch candidate".into()));
        }
    }
    Ok(())
}

/// 編集保存で通知する処理段階。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookPatchWritePhase {
    /// SBK または YBB の保存前検査。準備処理中の処理済み件数は 0 のままとなる。
    Validating,
    /// 符号化と逐次出力。
    Writing,
}

/// 編集保存の進捗。件数は処理済みの元局面数を表す。
///
/// 出力件数や完了率ではなく、段階間で件数は戻り得る。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BookPatchWriteProgress {
    pub(crate) phase: BookPatchWritePhase,
    pub(crate) processed_positions: u64,
    pub(crate) total_positions: Option<u64>,
}

impl BookPatchWriteProgress {
    /// 現在の処理段階を返す。
    #[must_use]
    pub const fn phase(self) -> BookPatchWritePhase {
        self.phase
    }

    /// 現在の段階で処理した元局面数を返す。
    #[must_use]
    pub const fn processed_positions(self) -> u64 {
        self.processed_positions
    }

    /// 元局面の総数を返す。DB2016 では `None` となる。
    #[must_use]
    pub const fn total_positions(self) -> Option<u64> {
        self.total_positions
    }

    pub(crate) fn notify(
        self,
        on_progress: &mut impl FnMut(Self) -> BookControl,
    ) -> Result<(), BookError> {
        if on_progress(self) == BookControl::Cancel { Err(BookError::Cancelled) } else { Ok(()) }
    }
}
