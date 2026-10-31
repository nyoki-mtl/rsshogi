//! 一意な局面キーを持つ YBB の差分整列マージ出力。

use std::io::{Seek, Write};

use super::{
    HEADER_BYTES, INDEX_BYTES, PackedSfen, Position, YbbBook, YbbEntries, YbbEntry,
    YbbWriteOptions, YbbWriter, database_ybb_entry,
};
use crate::book::patch::validate_patch_entry;
use crate::book::{
    BookControl, BookDatabaseEdit, BookDatabasePatch, BookError, BookPatchWritePhase,
    BookPatchWriteProgress, BookPosition,
};

impl YbbBook {
    /// 一意な元キーと局面差分を byte 順にマージし、別の空の出力先へ保存する。
    ///
    /// 元の depth 有無、未変更の手数、評価値、候補手順を保持する。
    /// 全差分を検査してから、全元行の構造、符号化、キーの厳密な昇順を出力前に検査する。
    /// 元キーはバイト列として扱い、未変更行の局面復元や合法手検査は行わない。
    /// upsert には明示手数と評価値を要求し、局面、候補手の一意性と合法性も検査する。
    /// 表現できない情報の破棄と値の補完は行わない。
    ///
    /// 入出力は別の保存領域を使い、完了まで元ファイルを変更してはいけない。
    /// 成功時は flush するが、永続化とファイル置換は呼び出し側が行う。
    /// 追加メモリは差分と最大行、固定バッファに比例する。
    ///
    /// # Errors
    /// 重複、逆順、不正または表現不能な差分は `InvalidData`、構造破損は既存 reader の
    /// エラー、読み書きと seek と flush の失敗は `Io` を返す。
    /// 事前検査中の失敗では出力に触れない。出力開始後は部分出力が残り得る。
    /// 差分と reader を変更せず、新しい出力先へ再試行できる。
    pub fn write_ybb_with_patch<W: Write + Seek>(
        &self,
        output: &mut W,
        patch: &BookDatabasePatch,
    ) -> Result<(), BookError> {
        self.write_ybb_with_patch_with_control(output, patch, |_| BookControl::Continue)
    }

    /// 事前検査と出力の進捗通知、協調キャンセルを伴う YBB 差分保存。
    ///
    /// `Validating` と `Writing` の開始時、各元行と追加差分の処理後、最終 finish 前に
    /// 通知する。差分の準備中にも元行処理数 0 で通知する。
    /// 総数は元行数であり、出力数や全体の完了率ではない。段階切替で処理数は 0 に戻る。
    /// 追加差分の処理中は元行処理数を据え置く。
    /// callback 中にファイルロックを保持せず、単一行処理とソートの内部中断は保証しない。
    /// 入出力と情報保持の契約は [`Self::write_ybb_with_patch`] と同じ。
    ///
    /// # Errors
    /// 通常版と同じエラーと、callback の `Cancel` に対する `Cancelled` を返す。
    /// 検査中のキャンセルでは出力に触れず、書き込み中は部分出力が残り得る。
    pub fn write_ybb_with_patch_with_control<W: Write + Seek>(
        &self,
        output: &mut W,
        patch: &BookDatabasePatch,
        mut control: impl FnMut(BookPatchWriteProgress) -> BookControl,
    ) -> Result<(), BookError> {
        let mut progress = BookPatchWriteProgress {
            phase: BookPatchWritePhase::Validating,
            processed_positions: 0,
            total_positions: Some(self.len()),
        };
        progress.notify(&mut control)?;
        let mut edits = Vec::with_capacity(patch.entries.len());
        let options = YbbWriteOptions::new().with_depth(self.has_depth);
        for (sfen, edit) in &patch.entries {
            let position = match edit {
                BookDatabaseEdit::Upsert(entry) => {
                    validate_patch_entry(entry)?;
                    database_ybb_entry(entry, options)?;
                    entry.position().clone()
                }
                BookDatabaseEdit::Delete => BookPosition::from_sfen(sfen, None)?,
            };
            let key = position.packed_sfen().ok_or_else(|| {
                BookError::InvalidData(format!("unencodable YBB patch position: {sfen}"))
            })?;
            edits.push(Edit { key, edit });
            progress.notify(&mut control)?;
        }
        edits.sort_by(|a, b| a.key.cmp_bytes(&b.key));
        if let Some(rows) = edits.windows(2).find(|rows| rows[0].key == rows[1].key) {
            return Err(BookError::InvalidData(format!(
                "duplicate YBB patch key: {}",
                key_description(rows[1].key)
            )));
        }
        progress.notify(&mut control)?;
        let mut output_count = 0u64;
        let mut move_bytes = 0u64;
        let width = if self.has_depth { 6 } else { 4 };
        merge(self, &edits, &mut progress, &mut control, |entry| {
            output_count = output_count.checked_add(1).ok_or_else(size_overflow)?;
            move_bytes = move_bytes
                .checked_add(entry.moves.len() as u64 * width)
                .ok_or_else(size_overflow)?;
            Ok(())
        })?;
        output_count
            .checked_mul(INDEX_BYTES as u64)
            .and_then(|n| n.checked_add(HEADER_BYTES as u64))
            .and_then(|n| n.checked_add(move_bytes))
            .ok_or_else(size_overflow)?;
        progress.notify(&mut control)?;

        progress.phase = BookPatchWritePhase::Writing;
        progress.processed_positions = 0;
        progress.notify(&mut control)?;
        let mut writer = YbbWriter::new(
            output,
            output_count,
            YbbWriteOptions::new().with_depth(self.has_depth),
        )?;
        merge(self, &edits, &mut progress, &mut control, |entry| writer.write_entry(entry))?;
        progress.notify(&mut control)?;
        writer.finish()?;
        Ok(())
    }
}

struct Edit<'a> {
    key: PackedSfen,
    edit: &'a BookDatabaseEdit,
}

fn size_overflow() -> BookError {
    BookError::InvalidData("ybb patch output size overflow".into())
}

fn key_description(key: PackedSfen) -> String {
    let mut position = Position::empty();
    let sfen = position
        .set_packed_sfen(&key, false, 1)
        .map(|()| position.to_sfen(Some(1)))
        .unwrap_or_else(|_| "unavailable".to_owned());
    format!("key={:02x?}, sfen={sfen}", key.as_bytes())
}

fn merge(
    book: &YbbBook,
    edits: &[Edit<'_>],
    progress: &mut BookPatchWriteProgress,
    control: &mut impl FnMut(BookPatchWriteProgress) -> BookControl,
    mut visit: impl FnMut(&YbbEntry) -> Result<(), BookError>,
) -> Result<(), BookError> {
    let mut edits = edits.iter().peekable();
    let options = YbbWriteOptions::new().with_depth(book.has_depth);
    for source in (StrictEntries { entries: book.iter_entries() }) {
        // 削除・置換される行も復号し、構造破損を差分で隠さない。
        let source = source?;
        while edits.peek().is_some_and(|edit| edit.key.cmp_bytes(&source.packed_sfen).is_lt()) {
            emit(edits.next().expect("peeked edit").edit, options, &mut visit)?;
            progress.notify(control)?;
        }
        if edits.peek().is_some_and(|edit| edit.key == source.packed_sfen) {
            emit(edits.next().expect("peeked edit").edit, options, &mut visit)?;
        } else {
            visit(&source)?;
        }
        progress.processed_positions += 1;
        progress.notify(control)?;
    }
    for edit in edits {
        emit(edit.edit, options, &mut visit)?;
        progress.notify(control)?;
    }
    Ok(())
}

fn emit(
    edit: &BookDatabaseEdit,
    options: YbbWriteOptions,
    visit: &mut impl FnMut(&YbbEntry) -> Result<(), BookError>,
) -> Result<(), BookError> {
    if let BookDatabaseEdit::Upsert(entry) = edit {
        visit(&database_ybb_entry(entry, options)?)?;
    }
    Ok(())
}

struct StrictEntries<'a> {
    entries: YbbEntries<'a>,
}

impl Iterator for StrictEntries<'_> {
    type Item = Result<YbbEntry, BookError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.entries.index == self.entries.book.len() as usize {
            return None;
        }
        let index = self.entries.index;
        let row = self.entries.index_row();
        self.entries.index += 1;
        Some(row.and_then(|row| {
            if self.entries.previous.is_some_and(|key| key.cmp_bytes(&row.packed_sfen).is_ge()) {
                return Err(BookError::InvalidData(format!(
                    "ybb patch source keys must be strictly increasing at row {index}: {}",
                    key_description(row.packed_sfen),
                )));
            }
            self.entries.decode_entry(row)
        }))
    }
}

#[cfg(test)]
mod tests;
