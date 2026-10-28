//! DB2016 の差分整列マージ出力。

use std::io::Write;

use super::{YaneuraOuBook, clean_line, is_header, strip_comment};
use crate::book::patch::validate_patch_entry;
use crate::book::{
    BookControl, BookDatabaseEdit, BookDatabaseEntry, BookDatabasePatch, BookError,
    BookPatchWritePhase, BookPatchWriteProgress, YaneuraOuDb2016WriteOptions,
    YaneuraOuDb2016Writer,
};

impl YaneuraOuBook {
    /// 元 DB2016 と差分を正規化 SFEN 順にマージして出力する。
    ///
    /// 元局面を逐次検査し、候補手順、手数の省略と明示的な 0、コメントを保持する。
    /// 前置コメントは保持し、NOE は省略する。成功時に writer を flush する。
    /// 入力と出力には別の保存領域を使い、完了まで元ファイルを変更してはいけない。
    ///
    /// # Errors
    /// 元局面の重複、逆順、不正行は `InvalidData`、差分の SBK metadata または
    /// weight は `Unsupported`、読み書きと flush の失敗は `Io` を返す。
    /// 失敗時は部分出力が残り得る。差分と reader は変更せず、先頭から再試行できる。
    pub fn write_yaneuraou_db2016_with_patch(
        &self,
        writer: &mut impl Write,
        patch: &BookDatabasePatch,
    ) -> Result<(), BookError> {
        self.write_yaneuraou_db2016_with_patch_with_control(writer, patch, |_| {
            BookControl::Continue
        })
    }

    /// 進捗通知と協調キャンセルを伴う DB2016 の差分保存を行う。
    ///
    /// `Writing` の開始時、元局面と追加局面の処理後、最終 flush 前に通知する。
    /// 総数は `None` とし、追加局面の処理中は処理済み元局面数を据え置く。
    /// 単一局面の処理中までの中断は保証しない。
    /// 入出力と情報保持の契約は [`Self::write_yaneuraou_db2016_with_patch`] と同じ。
    ///
    /// # Errors
    /// 通常版と同じエラーに加え、コールバックの `Cancel` で `Cancelled` を返す。
    /// キャンセル後も差分と reader を再利用できる。部分出力は呼び出し側が扱う。
    pub fn write_yaneuraou_db2016_with_patch_with_control(
        &self,
        writer: &mut impl Write,
        patch: &BookDatabasePatch,
        mut on_progress: impl FnMut(BookPatchWriteProgress) -> BookControl,
    ) -> Result<(), BookError> {
        for edit in patch.entries.values() {
            if let BookDatabaseEdit::Upsert(entry) = edit {
                validate_db_patch(entry)?;
            }
        }
        let mut progress = BookPatchWriteProgress {
            phase: BookPatchWritePhase::Writing,
            processed_positions: 0,
            total_positions: None,
        };
        progress.notify(&mut on_progress)?;
        let mut output = YaneuraOuDb2016Writer::new(
            writer,
            YaneuraOuDb2016WriteOptions::new().with_preserved_move_order(),
            None,
        )?;
        self.copy_patch_preamble(&mut output)?;
        let mut edits = patch.entries.iter().peekable();
        let mut previous: Option<String> = None;
        for source in self.iter_entries()? {
            let source = source?;
            let key = source.sfen();
            if previous.as_deref().is_some_and(|previous| previous >= key) {
                return Err(BookError::InvalidData(
                    "DB2016 source positions must be strictly increasing".into(),
                ));
            }
            // 削除・置換される行も検査し、差分で不正入力を隠さない。
            let entry = BookDatabaseEntry::from_yaneuraou(&source)?;
            validate_patch_entry(&entry)?;
            while edits.peek().is_some_and(|(patch_key, _)| patch_key.as_str() < key) {
                let (_, edit) = edits.next().expect("peeked patch entry");
                write_edit(&mut output, edit)?;
                progress.notify(&mut on_progress)?;
            }
            if edits.peek().is_some_and(|(patch_key, _)| patch_key.as_str() == key) {
                let (_, edit) = edits.next().expect("peeked patch entry");
                write_edit(&mut output, edit)?;
            } else {
                output.write_entry(&entry)?;
            }
            previous = Some(key.to_owned());
            progress.processed_positions += 1;
            progress.notify(&mut on_progress)?;
        }
        for (_, edit) in edits {
            write_edit(&mut output, edit)?;
            progress.notify(&mut on_progress)?;
        }
        progress.notify(&mut on_progress)?;
        output.finish()?;
        Ok(())
    }

    fn copy_patch_preamble<W: Write>(
        &self,
        output: &mut YaneuraOuDb2016Writer<W>,
    ) -> Result<(), BookError> {
        use std::io::BufRead;

        let mut input = std::io::BufReader::new(std::fs::File::open(&self.path)?);
        let mut line = String::new();
        let mut line_number = 0;
        loop {
            line.clear();
            if input.read_line(&mut line)? == 0 {
                break;
            }
            line_number += 1;
            let line = clean_line(&line, line_number);
            if line.starts_with("sfen ") {
                break;
            }
            if is_header(line, line_number) || line.trim().is_empty() {
                continue;
            }
            if let Some(comment) = strip_comment(line) {
                if !comment.starts_with("NOE:") {
                    output.write_preamble_comment(comment)?;
                }
            } else {
                return Err(BookError::InvalidData(format!(
                    "DB2016 data before first position at line {line_number}"
                )));
            }
        }
        Ok(())
    }
}

fn validate_db_patch(entry: &BookDatabaseEntry) -> Result<(), BookError> {
    if entry.metadata().sbk().is_some()
        || entry
            .candidates()
            .iter()
            .any(|candidate| candidate.weight().is_some() || candidate.metadata().sbk().is_some())
    {
        return Err(BookError::Unsupported("DB2016 patch cannot represent SBK metadata or weight"));
    }
    Ok(())
}

fn write_edit<W: Write>(
    output: &mut YaneuraOuDb2016Writer<W>,
    edit: &BookDatabaseEdit,
) -> Result<(), BookError> {
    if let BookDatabaseEdit::Upsert(entry) = edit {
        output.write_entry(entry)?;
    }
    Ok(())
}
