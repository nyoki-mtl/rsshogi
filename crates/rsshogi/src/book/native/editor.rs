use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use super::NativeBookRecords;
use super::builder::{encode_snapshot, write_sorted, write_sorted_with_block_positions};
use super::crc::crc32c;
use super::reader::Delta;
use super::wire::{MAX_TX_BYTES, encode_metadata};
use super::{
    NativeBook, NativeBookEntry, NativeBookMetadata, NativeBookPosition, NativeCandidate, add,
    invalid, sync_parent, temporary_path,
};
use crate::board::Position;
use crate::book::BookError;
use crate::types::Move;

fn key(position: &Position) -> Result<[u8; 32], BookError> {
    Ok(*position
        .try_to_packed_sfen()
        .map_err(|error| BookError::InvalidData(error.to_string()))?
        .as_bytes())
}

fn lock(path: &Path) -> Result<(PathBuf, File), BookError> {
    let path = if path.exists() {
        path.canonicalize()?
    } else {
        let parent =
            path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
        parent
            .canonicalize()?
            .join(path.file_name().ok_or_else(|| invalid("book path has no filename"))?)
    };
    let mut name =
        path.file_name().ok_or_else(|| invalid("book path has no filename"))?.to_os_string();
    name.push(".lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.with_file_name(name))?;
    match lock.try_lock() {
        Ok(()) => Ok((path, lock)),
        Err(std::fs::TryLockError::WouldBlock) => Err(BookError::Busy),
        Err(std::fs::TryLockError::Error(error)) => Err(BookError::Io(error)),
    }
}

/// 検証済み先頭部分を別ファイルへ救出した記録。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeBookSalvageReport {
    /// 原本の長さ。
    pub source_bytes: u64,
    /// 救出対象となった先頭部分の末尾。
    pub valid_end: u64,
    /// 救出した最後のtransaction連番。
    pub sequence: u64,
    /// 最初の異常位置。
    pub error_offset: u64,
    /// 新規出力先。
    pub output: PathBuf,
}

/// 1 writerの排他下で局面単位の差分を追記する。
pub struct NativeBookEditor {
    path: PathBuf,
    _lock: File,
    book: NativeBook,
    dirty: BTreeMap<[u8; 32], Option<NativeBookPosition>>,
    metadata: Option<NativeBookMetadata>,
    poisoned: bool,
}

impl NativeBookEditor {
    /// 既存の定跡を編集用に開く。
    pub fn open(path: impl AsRef<Path>) -> Result<Self, BookError> {
        let (path, lock) = lock(path.as_ref())?;
        let book = NativeBook::open(&path)?;
        if let Some(tail) = book.tail() {
            return Err(BookError::NativeLog {
                offset: tail.offset,
                valid_end: book.valid_end,
                sequence: book.sequence,
                reason: "incomplete transaction requires recovery",
            });
        }
        Ok(Self {
            path,
            _lock: lock,
            book,
            dirty: BTreeMap::new(),
            metadata: None,
            poisoned: false,
        })
    }

    /// 空の新規定跡を作り、編集ロックを保持する。
    pub fn create(path: impl AsRef<Path>, metadata: NativeBookMetadata) -> Result<Self, BookError> {
        let (path, lock) = lock(path.as_ref())?;
        write_sorted(&path, metadata, std::iter::empty(), false)?;
        let book = NativeBook::open(&path)?;
        Ok(Self {
            path,
            _lock: lock,
            book,
            dirty: BTreeMap::new(),
            metadata: None,
            poisoned: false,
        })
    }

    fn writable(&self) -> Result<(), BookError> {
        if self.poisoned { Err(BookError::SaveUncertain("reopen required".into())) } else { Ok(()) }
    }

    /// 最後に保存したsnapshot。未保存編集は含まない。
    pub fn book(&self) -> &NativeBook {
        &self.book
    }

    /// 未保存の変更があるかを返す。
    pub fn is_dirty(&self) -> bool {
        !self.dirty.is_empty() || self.metadata.is_some()
    }

    /// 未保存編集も含め、コメント付きの局面をキー順に走査する。
    pub fn records(&self) -> NativeBookEditedRecords<'_> {
        NativeBookEditedRecords {
            saved: self.book.records().peekable(),
            dirty: self.dirty.iter().peekable(),
        }
    }

    /// 未保存編集を含むsnapshotを別の新規ファイルへ書く。
    pub fn write_to(&self, path: impl AsRef<Path>) -> Result<(), BookError> {
        self.write_to_with_replace(path, false)
    }

    /// 出力先の排他下でsnapshotを書き出す。replaceは既存出力の置換を許可する。
    pub fn write_to_with_replace(
        &self,
        path: impl AsRef<Path>,
        replace: bool,
    ) -> Result<(), BookError> {
        self.writable()?;
        let (path, _lock) = lock(path.as_ref())?;
        write_sorted_with_block_positions(
            &path,
            self.metadata()?,
            self.records(),
            replace,
            self.book.block_positions(),
        )
    }

    /// 未保存編集を含む局面を取得する。
    pub fn record(&self, position: &Position) -> Result<Option<NativeBookPosition>, BookError> {
        let key = key(position)?;
        match self.dirty.get(&key) {
            Some(record) => Ok(record.clone()),
            None => self.book.record_key(key),
        }
    }

    /// 未保存編集を含む文書メタデータ。
    pub fn metadata(&self) -> Result<NativeBookMetadata, BookError> {
        self.metadata.clone().map_or_else(|| self.book.metadata(), Ok)
    }

    /// 文書メタデータを変更する。
    pub fn set_metadata(&mut self, metadata: NativeBookMetadata) -> Result<(), BookError> {
        self.writable()?;
        let metadata = metadata.normalized()?;
        if metadata != self.metadata()? {
            self.metadata = Some(metadata);
        }
        Ok(())
    }

    /// コメントを含む局面全体を置換する。
    pub fn upsert_position(&mut self, record: NativeBookPosition) -> Result<(), BookError> {
        self.writable()?;
        let record = record.normalized()?;
        let existing = self
            .dirty
            .get(&record.entry.key)
            .cloned()
            .unwrap_or(self.book.record_key(record.entry.key)?);
        if existing.as_ref() != Some(&record) {
            self.dirty.insert(record.entry.key, Some(record));
        }
        Ok(())
    }

    /// 局面と付属する全データを削除する。
    pub fn remove_position(&mut self, position: &Position) -> Result<bool, BookError> {
        self.writable()?;
        if self.record(position)?.is_none() {
            return Ok(false);
        }
        self.dirty.insert(key(position)?, None);
        Ok(true)
    }

    /// 候補手を追加または置換する。commentがNoneなら既存コメントを保持する。
    pub fn upsert_candidate(
        &mut self,
        position: &Position,
        mut candidate: NativeCandidate,
        comment: Option<String>,
    ) -> Result<(), BookError> {
        self.writable()?;
        let mut record = self
            .record(position)?
            .unwrap_or(NativeBookPosition::new(NativeBookEntry::from_position(position, vec![])?));
        let move_raw = candidate.mv.raw();
        if let Some(existing) =
            record.entry.candidates.iter_mut().find(|existing| existing.mv == candidate.mv)
        {
            candidate.display_order = existing.display_order;
            *existing = candidate;
        } else {
            candidate.display_order = u16::try_from(record.entry.candidates.len())
                .map_err(|_| invalid("too many candidates"))?;
            record.entry.candidates.push(candidate);
            record.entry.candidates.sort_by_key(|candidate| candidate.mv.raw());
        }
        if let Some(comment) = comment {
            record.move_comments.insert(move_raw, comment);
        }
        self.upsert_position(record)
    }

    /// 候補手とそのコメントを削除し、局面は残す。
    pub fn remove_candidate(&mut self, position: &Position, mv: Move) -> Result<bool, BookError> {
        self.writable()?;
        let Some(mut record) = self.record(position)? else {
            return Ok(false);
        };
        let Some(index) = record.entry.candidates.iter().position(|candidate| candidate.mv == mv)
        else {
            return Ok(false);
        };
        let removed = record.entry.candidates.remove(index);
        for candidate in &mut record.entry.candidates {
            if candidate.display_order > removed.display_order {
                candidate.display_order -= 1;
            }
        }
        record.move_comments.remove(&mv.raw());
        self.upsert_position(record)?;
        Ok(true)
    }

    /// 候補手の表示順位だけを変更する。
    pub fn reorder_candidate(
        &mut self,
        position: &Position,
        mv: Move,
        target: u16,
    ) -> Result<bool, BookError> {
        self.writable()?;
        let Some(mut record) = self.record(position)? else {
            return Ok(false);
        };
        if usize::from(target) >= record.entry.candidates.len() {
            return Err(BookError::InvalidData("display rank out of range".into()));
        }
        let Some(old) = record
            .entry
            .candidates
            .iter()
            .find(|candidate| candidate.mv == mv)
            .map(|candidate| candidate.display_order)
        else {
            return Ok(false);
        };
        if old == target {
            return Ok(false);
        }
        for candidate in &mut record.entry.candidates {
            if candidate.mv == mv {
                candidate.display_order = target;
            } else if old < target
                && candidate.display_order > old
                && candidate.display_order <= target
            {
                candidate.display_order -= 1;
            } else if old > target
                && candidate.display_order >= target
                && candidate.display_order < old
            {
                candidate.display_order += 1;
            }
        }
        self.upsert_position(record)?;
        Ok(true)
    }

    /// 局面コメントを変更する。不在局面なら候補手ゼロで追加する。
    pub fn set_position_comment(
        &mut self,
        position: &Position,
        comment: Option<String>,
    ) -> Result<(), BookError> {
        self.writable()?;
        let mut record = self
            .record(position)?
            .unwrap_or(NativeBookPosition::new(NativeBookEntry::from_position(position, vec![])?));
        record.comment = comment;
        self.upsert_position(record)
    }

    /// 変更局面だけを1 transactionで追記し、同期後にdirtyを解除する。
    pub fn save(&mut self) -> Result<bool, BookError> {
        self.save_with(|path, start, frame| {
            let mut file = OpenOptions::new().append(true).open(path)?;
            if file.metadata()?.len() != start {
                return Err(invalid("native file changed outside writer"));
            }
            file.write_all(frame)?;
            file.sync_all()?;
            Ok(())
        })
    }

    pub(super) fn save_with(
        &mut self,
        append: impl FnOnce(&Path, u64, &[u8]) -> Result<(), BookError>,
    ) -> Result<bool, BookError> {
        self.writable()?;
        if !self.is_dirty() {
            return Ok(false);
        }
        let sequence = self
            .book
            .sequence
            .checked_add(1)
            .ok_or_else(|| invalid("transaction sequence overflow"))?;
        let start = self.book.valid_end;
        let mut payload = Vec::new();
        let mut positions = Vec::new();
        let mut metadata_location = None;
        if let Some(metadata) = &self.metadata {
            let data = encode_metadata(metadata)?;
            append_operation(&mut payload, 3, [0; 32], &data)?;
            metadata_location = Some((start + 40 + 48, data.len() as u64));
        }
        for (key, record) in &self.dirty {
            let data = record.as_ref().map(encode_snapshot).transpose()?;
            let data_offset = add(start + 40, payload.len() as u64 + 48)?;
            match data {
                Some(data) => {
                    append_operation(&mut payload, 1, *key, &data)?;
                    positions.push((
                        *key,
                        Delta::Snapshot { offset: data_offset, len: data.len() as u64 },
                    ));
                }
                None => {
                    append_operation(&mut payload, 2, *key, &[])?;
                    positions.push((*key, Delta::Deleted));
                }
            }
        }
        let mut frame = Vec::with_capacity(payload.len() + 64);
        frame.extend_from_slice(b"RSSHGTX4");
        frame.extend_from_slice(&sequence.to_le_bytes());
        frame.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        frame.extend_from_slice(
            &(self.dirty.len() as u64 + u64::from(self.metadata.is_some())).to_le_bytes(),
        );
        frame.extend_from_slice(&[0; 8]);
        let header_crc = crc32c(&frame);
        frame[32..36].copy_from_slice(&header_crc.to_le_bytes());
        frame.extend_from_slice(&payload);
        let crc = crc32c(&frame);
        frame.extend_from_slice(b"RSSHGCMT");
        frame.extend_from_slice(&sequence.to_le_bytes());
        frame.extend_from_slice(&crc.to_le_bytes());
        frame.extend_from_slice(&[0; 4]);
        let end = add(start, frame.len() as u64)?;
        let result = (|| -> Result<(), BookError> {
            append(&self.path, start, &frame)?;
            self.book.accept_saved(&self.path, sequence, end, positions, metadata_location)?;
            Ok(())
        })();
        if let Err(error) = result {
            self.poisoned = true;
            return Err(BookError::SaveUncertain(error.to_string()));
        }
        self.dirty.clear();
        self.metadata = None;
        Ok(true)
    }

    /// 保存済みsnapshotをログなしの本体へ原子的に統合する。
    pub fn compact(&mut self) -> Result<(), BookError> {
        self.compact_with(|path| NativeBook::open(path))
    }

    pub(super) fn compact_with(
        &mut self,
        reopen: impl FnOnce(&Path) -> Result<NativeBook, BookError>,
    ) -> Result<(), BookError> {
        self.save()?;
        self.book.validate_full()?;
        let metadata = self.book.metadata()?;
        let result = (|| {
            write_sorted_with_block_positions(
                &self.path,
                metadata,
                self.book.records(),
                true,
                self.book.block_positions(),
            )?;
            reopen(&self.path)
        })();
        match result {
            Ok(book) => self.book = book,
            Err(error) => {
                self.poisoned = true;
                return Err(BookError::SaveUncertain(error.to_string()));
            }
        }
        Ok(())
    }

    /// 未完了tailを別ファイルへ保全してから切り詰める。
    pub fn recover_tail(path: impl AsRef<Path>) -> Result<PathBuf, BookError> {
        Self::recover_tail_with(path.as_ref(), |backup| Ok(backup.sync_all()?))
    }

    pub(super) fn recover_tail_with(
        path: &Path,
        sync_backup: impl FnOnce(&File) -> Result<(), BookError>,
    ) -> Result<PathBuf, BookError> {
        let (path, _lock) = lock(path)?;
        let book = NativeBook::open(&path)?;
        let tail = book.tail().ok_or_else(|| invalid("no recoverable incomplete tail"))?;
        let backup_path = temporary_path(&path, "recovery");
        let mut backup = OpenOptions::new().write(true).create_new(true).open(&backup_path)?;
        backup.write_all(b"RSSHGBAD")?;
        backup.write_all(&tail.offset.to_le_bytes())?;
        backup.write_all(&tail.end.to_le_bytes())?;
        let mut offset = tail.offset;
        while offset < tail.end {
            let len = (tail.end - offset).min(64 * 1024);
            backup.write_all(&book.file.read(offset, len)?)?;
            offset += len;
        }
        sync_backup(&backup)?;
        sync_parent(&backup_path)?;
        let file = OpenOptions::new().write(true).open(&path)?;
        (|| -> std::io::Result<()> {
            file.set_len(tail.offset)?;
            file.sync_all()
        })()
        .map_err(|error| BookError::SaveUncertain(error.to_string()))?;
        Ok(backup_path)
    }

    /// 原本を変更せず、検証済み先頭snapshotを新規ファイルへ救出する。
    pub fn salvage_prefix(
        path: impl AsRef<Path>,
        output: impl AsRef<Path>,
    ) -> Result<NativeBookSalvageReport, BookError> {
        let (path, _lock) = lock(path.as_ref())?;
        let source_bytes = std::fs::metadata(&path)?.len();
        let (book, mut error_offset) = match NativeBook::open(&path) {
            Ok(book) => {
                let offset = book.tail().map_or(book.valid_end, |tail| tail.offset);
                (NativeBook::open_prefix(&path, book.valid_end)?, offset)
            }
            Err(BookError::NativeLog { offset, valid_end, .. }) => {
                (NativeBook::open_prefix(&path, valid_end)?, offset)
            }
            Err(error) => return Err(error),
        };
        let book = match book.validate_full() {
            Ok(()) => book,
            Err(BookError::NativeLog { offset, valid_end, .. }) => {
                error_offset = offset;
                let prefix = NativeBook::open_prefix(&path, valid_end)?;
                prefix.validate_full()?;
                prefix
            }
            Err(error) => return Err(error),
        };
        write_sorted_with_block_positions(
            output.as_ref(),
            book.metadata()?,
            book.records(),
            false,
            book.block_positions(),
        )?;
        Ok(NativeBookSalvageReport {
            source_bytes,
            valid_end: book.valid_end,
            sequence: book.sequence,
            error_offset,
            output: output.as_ref().to_path_buf(),
        })
    }
}

/// 保存済みの順次走査と未保存差分をmergeするiterator。
pub struct NativeBookEditedRecords<'a> {
    saved: std::iter::Peekable<NativeBookRecords<'a>>,
    dirty: std::iter::Peekable<
        std::collections::btree_map::Iter<'a, [u8; 32], Option<NativeBookPosition>>,
    >,
}

impl Iterator for NativeBookEditedRecords<'_> {
    type Item = Result<NativeBookPosition, BookError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let saved_key = match self.saved.peek() {
                Some(Ok(record)) => Some(record.entry.key),
                Some(Err(_)) => return self.saved.next(),
                None => None,
            };
            if let Some((key, _)) = self.dirty.peek()
                && saved_key.is_none_or(|saved_key| **key <= saved_key)
            {
                let (key, record) = self.dirty.next().expect("peeked dirty entry");
                if saved_key == Some(*key) {
                    self.saved.next();
                }
                if let Some(record) = record {
                    return Some(Ok(record.clone()));
                }
                continue;
            }
            return self.saved.next();
        }
    }
}

fn append_operation(
    payload: &mut Vec<u8>,
    opcode: u8,
    key: [u8; 32],
    data: &[u8],
) -> Result<(), BookError> {
    let len = payload
        .len()
        .checked_add(48)
        .and_then(|len| len.checked_add(data.len()))
        .and_then(|len| len.checked_add(64));
    if len.is_none_or(|len| len as u64 > MAX_TX_BYTES) {
        return Err(invalid("native transaction exceeds 256MiB"));
    }
    payload.push(opcode);
    payload.extend_from_slice(&[0; 7]);
    payload.extend_from_slice(&key);
    payload.extend_from_slice(&(data.len() as u64).to_le_bytes());
    payload.extend_from_slice(data);
    Ok(())
}
