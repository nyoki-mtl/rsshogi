use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use super::crc::Crc32c;
use super::wire::{
    self, CommentIndex, Header, decode_candidate, encode_candidate, encode_metadata, read_u64,
};
use super::{
    NativeBookEntry, NativeBookMetadata, NativeBookPosition, add, invalid, mul, sync_parent,
    temporary_path,
};
use crate::book::BookError;

const BUFFER_BYTES: usize = 64 * 1024;
const DEFAULT_MEMORY_BYTES: usize = 256 * 1024 * 1024;
const MIN_MEMORY_BYTES: usize = 1024 * 1024;
const MERGE_FAN_IN: usize = 8;

struct Temporary {
    path: PathBuf,
    writer: Option<BufWriter<File>>,
}

impl Temporary {
    fn new(destination: &Path, label: &str) -> Result<Self, BookError> {
        let path = temporary_path(destination, label);
        let file = OpenOptions::new().write(true).read(true).create_new(true).open(&path)?;
        Ok(Self { path, writer: Some(BufWriter::with_capacity(BUFFER_BYTES, file)) })
    }

    fn writer(&mut self) -> &mut BufWriter<File> {
        self.writer.as_mut().expect("temporary writer is open")
    }

    fn close(&mut self) -> Result<(), BookError> {
        if let Some(mut writer) = self.writer.take() {
            writer.flush()?;
        }
        Ok(())
    }

    fn copy_to(&mut self, output: &mut impl Write, crc: &mut Crc32c) -> Result<(), BookError> {
        self.close()?;
        let mut input = File::open(&self.path)?;
        let mut buffer = [0; BUFFER_BYTES];
        loop {
            let len = input.read(&mut buffer)?;
            if len == 0 {
                return Ok(());
            }
            output.write_all(&buffer[..len])?;
            crc.update(&buffer[..len]);
        }
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        drop(self.writer.take());
        let _ = fs::remove_file(&self.path);
    }
}

fn snapshot_size(record: &NativeBookPosition) -> Result<u64, BookError> {
    let count = record.entry.candidates.len() as u64;
    let text_bytes =
        record.move_comments.values().try_fold(
            record.comment.as_ref().map_or(0, |text| text.len() as u64),
            |bytes, text| add(bytes, text.len() as u64),
        )?;
    add(add(16, mul(count, 16)?)?, add(mul(add(count, 2)?, 8)?, text_bytes)?)
}

/// ログと一時 run に共通する局面 snapshot を符号化する。
pub(super) fn encode_snapshot(record: &NativeBookPosition) -> Result<Vec<u8>, BookError> {
    let normalized = record.clone().normalized()?;
    encode_normalized_snapshot(&normalized)
}

fn encode_normalized_snapshot(record: &NativeBookPosition) -> Result<Vec<u8>, BookError> {
    let size = snapshot_size(record)?;
    if size > wire::MAX_TX_BYTES - 112 {
        return Err(invalid("native position snapshot exceeds transaction limit"));
    }
    let count = record.entry.candidates.len();
    let fixed_bytes = 16 + count * wire::MOVE_BYTES + (count + 2) * 8;
    let mut bytes = Vec::with_capacity(size as usize);
    bytes.extend_from_slice(&(count as u64).to_le_bytes());
    bytes.extend_from_slice(&(size - fixed_bytes as u64).to_le_bytes());
    for candidate in &record.entry.candidates {
        bytes.extend_from_slice(&encode_candidate(candidate)?);
    }
    let mut text_offset = 0u64;
    bytes.extend_from_slice(&text_offset.to_le_bytes());
    text_offset += record.comment.as_ref().map_or(0, |text| text.len() as u64);
    bytes.extend_from_slice(&text_offset.to_le_bytes());
    for candidate in &record.entry.candidates {
        text_offset +=
            record.move_comments.get(&candidate.mv.raw()).map_or(0, |text| text.len() as u64);
        bytes.extend_from_slice(&text_offset.to_le_bytes());
    }
    if let Some(text) = &record.comment {
        bytes.extend_from_slice(text.as_bytes());
    }
    for candidate in &record.entry.candidates {
        if let Some(text) = record.move_comments.get(&candidate.mv.raw()) {
            bytes.extend_from_slice(text.as_bytes());
        }
    }
    Ok(bytes)
}

/// snapshot の構造と意味を検証して復号する。
pub(super) fn decode_snapshot(
    key: [u8; 32],
    bytes: &[u8],
) -> Result<NativeBookPosition, BookError> {
    if bytes.len() as u64 > wire::MAX_TX_BYTES - 112 {
        return Err(invalid("native position snapshot exceeds transaction limit"));
    }
    let count = read_u64(bytes, 0)?;
    let text_bytes = read_u64(bytes, 8)?;
    if count > u64::from(u16::MAX) {
        return Err(invalid("too many native snapshot candidates"));
    }
    let offsets_start = add(16, mul(count, wire::MOVE_BYTES as u64)?)?;
    let text_start = add(offsets_start, mul(add(count, 2)?, 8)?)?;
    if add(text_start, text_bytes)? != bytes.len() as u64 {
        return Err(invalid("native snapshot length mismatch"));
    }
    let mut candidates = Vec::with_capacity(count as usize);
    for index in 0..count as usize {
        let offset = 16 + index * wire::MOVE_BYTES;
        candidates.push(decode_candidate(&bytes[offset..offset + wire::MOVE_BYTES])?);
    }
    let entry = NativeBookEntry { key, candidates };
    entry.validate()?;
    let mut record = NativeBookPosition::new(entry);
    let mut previous = read_u64(bytes, offsets_start as usize)?;
    if previous != 0 {
        return Err(invalid("native snapshot first comment offset is not zero"));
    }
    for index in 0..=count as usize {
        let next = read_u64(bytes, offsets_start as usize + (index + 1) * 8)?;
        if next < previous || next > text_bytes || next - previous > wire::MAX_TEXT_BYTES as u64 {
            return Err(invalid("invalid native snapshot comment range"));
        }
        if next != previous {
            let start = (text_start + previous) as usize;
            let end = (text_start + next) as usize;
            let text = std::str::from_utf8(&bytes[start..end])
                .map_err(|_| invalid("invalid native snapshot comment utf-8"))?;
            if text.contains('\r') {
                return Err(invalid("noncanonical native snapshot newline"));
            }
            if index == 0 {
                record.comment = Some(text.to_owned());
            } else {
                record
                    .move_comments
                    .insert(record.entry.candidates[index - 1].mv.raw(), text.to_owned());
            }
        }
        previous = next;
    }
    if previous != text_bytes {
        return Err(invalid("native snapshot unreferenced text"));
    }
    Ok(record)
}

/// キー順の入力を本体へ順次書き出す。
pub(super) fn write_sorted(
    path: &Path,
    metadata: NativeBookMetadata,
    records: impl IntoIterator<Item = Result<NativeBookPosition, BookError>>,
    replace: bool,
) -> Result<(), BookError> {
    write_sorted_with_block_positions(path, metadata, records, replace, wire::BLOCK_POSITIONS)
}

/// 指定したブロック局面数で本体を書き出す。
pub(super) fn write_sorted_with_block_positions(
    path: &Path,
    metadata: NativeBookMetadata,
    records: impl IntoIterator<Item = Result<NativeBookPosition, BookError>>,
    replace: bool,
    block_positions: u32,
) -> Result<(), BookError> {
    write_sorted_with(path, metadata, records, replace, block_positions, |_| Ok(()))
}

fn write_sorted_with(
    path: &Path,
    metadata: NativeBookMetadata,
    records: impl IntoIterator<Item = Result<NativeBookPosition, BookError>>,
    replace: bool,
    block_positions: u32,
    mut checkpoint: impl FnMut(&'static str) -> Result<(), BookError>,
) -> Result<(), BookError> {
    if !(1..=wire::MAX_BLOCK_POSITIONS).contains(&block_positions) {
        return Err(invalid("native block position count must be between 1 and 16384"));
    }
    let metadata = metadata.normalized()?;
    let metadata_bytes = if metadata == NativeBookMetadata::default() {
        Vec::new()
    } else {
        encode_metadata(&metadata)?
    };
    let mut indexes = Temporary::new(path, "index")?;
    let mut blocks = Temporary::new(path, "blocks")?;
    let mut block_entries = Vec::with_capacity(block_positions as usize);
    let mut compressed_bytes = 0u64;
    let flush_block = |entries: &mut Vec<NativeBookEntry>,
                       indexes: &mut Temporary,
                       blocks: &mut Temporary,
                       offset: &mut u64|
     -> Result<(), BookError> {
        if entries.is_empty() {
            return Ok(());
        }
        let (row, compressed) = super::block::encode(entries, *offset)?;
        indexes.writer().write_all(&row.encode())?;
        blocks.writer().write_all(&compressed)?;
        *offset = add(*offset, compressed.len() as u64)?;
        entries.clear();
        Ok(())
    };
    let mut comments = Temporary::new(path, "comments")?;
    let mut text = Temporary::new(path, "text")?;
    let mut node_count = 0;
    let mut move_count = 0;
    let mut comment_count = 0;
    let mut text_bytes = 0;
    let mut previous_key = None;
    for record in records {
        let record = record?.normalized()?;
        if previous_key.is_some_and(|key| key >= record.entry.key) {
            return Err(invalid("native sorted input keys are duplicated or unordered"));
        }
        previous_key = Some(record.entry.key);
        move_count = add(move_count, record.entry.candidates.len() as u64)?;
        block_entries.push(record.entry);
        if block_entries.len() == block_positions as usize {
            flush_block(&mut block_entries, &mut indexes, &mut blocks, &mut compressed_bytes)?;
        }
        let mut write_comment = |target_kind, move_raw, body: &str| -> Result<(), BookError> {
            let index = CommentIndex {
                position_id: node_count,
                move_raw,
                target_kind,
                offset: text_bytes,
                len: body.len() as u64,
            };
            comments.writer().write_all(&index.encode()?)?;
            text.writer().write_all(body.as_bytes())?;
            comment_count = add(comment_count, 1)?;
            text_bytes = add(text_bytes, body.len() as u64)?;
            Ok(())
        };
        if let Some(body) = record.comment {
            write_comment(0, 0, &body)?;
        }
        for (mv, body) in record.move_comments {
            write_comment(1, mv, &body)?;
        }
        node_count = add(node_count, 1)?;
    }
    flush_block(&mut block_entries, &mut indexes, &mut blocks, &mut compressed_bytes)?;
    let mut header = Header::new(
        node_count,
        move_count,
        comment_count,
        text_bytes,
        metadata_bytes.len() as u64,
        compressed_bytes,
        block_positions,
    )?;
    header.depth_kind = metadata.depth_kind;
    let mut output = Temporary::new(path, "base")?;
    output.writer().write_all(&[0; wire::HEADER_BYTES])?;
    let mut crc = Crc32c::new();
    for section in [&mut indexes, &mut blocks, &mut comments, &mut text] {
        section.copy_to(output.writer(), &mut crc)?;
    }
    output.writer().write_all(&metadata_bytes)?;
    crc.update(&metadata_bytes);
    header.base_crc = crc.finish();
    output.writer().seek(SeekFrom::Start(0))?;
    output.writer().write_all(&header.encode())?;
    checkpoint("written")?;
    output.writer().flush()?;
    checkpoint("flushed")?;
    output.writer().get_ref().sync_all()?;
    checkpoint("synced")?;
    output.close()?;
    super::NativeBook::open(&output.path)?.validate_full()?;
    checkpoint("validated")?;
    if replace {
        fs::rename(&output.path, path)?;
    } else {
        fs::hard_link(&output.path, path)?;
        fs::remove_file(&output.path)
            .map_err(|error| BookError::SaveUncertain(error.to_string()))?;
    }
    checkpoint("published").map_err(|error| BookError::SaveUncertain(error.to_string()))?;
    sync_parent(path).map_err(|error| BookError::SaveUncertain(error.to_string()))?;
    Ok(())
}

struct RunCursor {
    input: BufReader<File>,
    _temporary: Temporary,
    next: Option<([u8; 32], u64)>,
}

impl RunCursor {
    fn new(mut temporary: Temporary) -> Result<Self, BookError> {
        temporary.close()?;
        let input = BufReader::with_capacity(BUFFER_BYTES, File::open(&temporary.path)?);
        let mut cursor = Self { input, _temporary: temporary, next: None };
        cursor.advance()?;
        Ok(cursor)
    }

    fn advance(&mut self) -> Result<(), BookError> {
        let mut header = [0; 40];
        let first = self.input.read(&mut header[..1])?;
        if first == 0 {
            self.next = None;
            return Ok(());
        }
        self.input.read_exact(&mut header[1..])?;
        let key = header[..32].try_into().expect("key bytes");
        let len = read_u64(&header, 32)?;
        if !(32..=wire::MAX_TX_BYTES - 112).contains(&len) {
            return Err(invalid("invalid native sort run record length"));
        }
        self.next = Some((key, len));
        Ok(())
    }
}

fn merge_runs(destination: &Path, runs: Vec<Temporary>) -> Result<Temporary, BookError> {
    let mut output = Temporary::new(destination, "merge")?;
    let mut cursors = runs.into_iter().map(RunCursor::new).collect::<Result<Vec<_>, _>>()?;
    let mut previous = None;
    let mut buffer = [0; BUFFER_BYTES];
    loop {
        let selected = cursors
            .iter()
            .enumerate()
            .filter_map(|(index, cursor)| cursor.next.map(|next| (next.0, index)))
            .min();
        let Some((key, index)) = selected else { break };
        if previous.is_some_and(|old| old >= key) {
            return Err(invalid("duplicate native input position"));
        }
        previous = Some(key);
        let cursor = &mut cursors[index];
        let (_, len) = cursor.next.expect("selected run record");
        output.writer().write_all(&key)?;
        output.writer().write_all(&len.to_le_bytes())?;
        let mut remaining = len;
        while remaining != 0 {
            let bytes = remaining.min(BUFFER_BYTES as u64) as usize;
            cursor.input.read_exact(&mut buffer[..bytes])?;
            output.writer().write_all(&buffer[..bytes])?;
            remaining -= bytes as u64;
        }
        cursor.advance()?;
    }
    output.close()?;
    Ok(output)
}

struct RunRecords {
    cursor: RunCursor,
    failed: bool,
}

impl Iterator for RunRecords {
    type Item = Result<NativeBookPosition, BookError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let (key, len) = self.cursor.next?;
        let result = (|| {
            let mut bytes = vec![0; len as usize];
            self.cursor.input.read_exact(&mut bytes)?;
            let record = decode_snapshot(key, &bytes)?;
            self.cursor.advance()?;
            Ok(record)
        })();
        self.failed = result.is_err();
        Some(result)
    }
}

/// メモリ予算内の run を同じ保存先ボリュームへ退避する定跡 builder。
pub struct NativeBookBuilder {
    path: PathBuf,
    metadata: NativeBookMetadata,
    memory_budget: usize,
    block_positions: u32,
    pending_bytes: usize,
    pending: Vec<([u8; 32], Vec<u8>)>,
    levels: Vec<Vec<Temporary>>,
    failed: bool,
}

impl NativeBookBuilder {
    /// 保存先と文書メタデータを指定し、既定 256 MiB の予算で構築を始める。
    ///
    /// # Errors
    /// メタデータが不正な場合、保存先が存在する場合に返す。
    pub fn new(path: impl AsRef<Path>, metadata: NativeBookMetadata) -> Result<Self, BookError> {
        let path = path.as_ref().to_path_buf();
        if path.try_exists()? {
            return Err(invalid("native builder destination already exists"));
        }
        Ok(Self {
            path,
            metadata: metadata.normalized()?,
            memory_budget: DEFAULT_MEMORY_BYTES,
            block_positions: wire::BLOCK_POSITIONS,
            pending_bytes: 0,
            pending: Vec::new(),
            levels: Vec::new(),
            failed: false,
        })
    }

    /// 一時 buffer と少なくとも 1 局面を含むメモリ予算を設定する。
    ///
    /// # Errors
    /// 予算が 1 MiB 未満、または入力開始後の場合に返す。
    pub fn with_memory_budget(mut self, bytes: usize) -> Result<Self, BookError> {
        if bytes < MIN_MEMORY_BYTES || !self.pending.is_empty() || !self.levels.is_empty() {
            return Err(invalid(
                "native builder memory budget must be set before input and at least 1 MiB",
            ));
        }
        self.memory_budget = bytes;
        Ok(self)
    }

    /// ブロックあたりの局面数を指定する。既定と推奨値は 64。
    ///
    /// # Errors
    /// 1..=16384 の範囲外、または入力開始後の場合に返す。
    pub fn with_block_positions(mut self, block_positions: u32) -> Result<Self, BookError> {
        if !(1..=wire::MAX_BLOCK_POSITIONS).contains(&block_positions)
            || !self.pending.is_empty()
            || !self.levels.is_empty()
        {
            return Err(invalid(
                "native block position count must be set before input and between 1 and 16384",
            ));
        }
        self.block_positions = block_positions;
        Ok(self)
    }

    /// 1 局面を追加し、必要に応じて整列済み run へ退避する。
    ///
    /// # Errors
    /// 局面が不正、単一局面が予算外、run の書込が失敗した場合に返す。
    pub fn push(&mut self, record: NativeBookPosition) -> Result<(), BookError> {
        if self.failed {
            return Err(invalid("native builder previously failed"));
        }
        let result = self.push_inner(record);
        self.failed = result.is_err();
        result
    }

    fn push_inner(&mut self, record: NativeBookPosition) -> Result<(), BookError> {
        let size = snapshot_size(&record)?;
        let charge = usize::try_from(size)
            .ok()
            .and_then(|bytes| bytes.checked_mul(4))
            .and_then(|bytes| bytes.checked_add(128))
            .ok_or_else(|| invalid("native builder record size overflow"))?;
        let available = self.memory_budget - BUFFER_BYTES * (MERGE_FAN_IN + 2);
        if charge > available {
            return Err(invalid("native builder record exceeds memory budget"));
        }
        if self.pending_bytes.checked_add(charge).is_none_or(|total| total > available) {
            self.spill()?;
        }
        let record = record.normalized()?;
        let bytes = encode_normalized_snapshot(&record)?;
        self.pending_bytes += charge;
        self.pending.push((record.entry.key, bytes));
        Ok(())
    }

    fn spill(&mut self) -> Result<(), BookError> {
        if self.pending.is_empty() {
            return Ok(());
        }
        self.pending.sort_unstable_by_key(|record| record.0);
        let mut run = Temporary::new(&self.path, "run")?;
        let mut previous = None;
        for (key, bytes) in self.pending.drain(..) {
            if previous == Some(key) {
                return Err(invalid("duplicate native input position"));
            }
            previous = Some(key);
            run.writer().write_all(&key)?;
            run.writer().write_all(&(bytes.len() as u64).to_le_bytes())?;
            run.writer().write_all(&bytes)?;
        }
        run.close()?;
        self.pending_bytes = 0;
        let mut level = 0;
        loop {
            if level == self.levels.len() {
                self.levels.push(Vec::new());
            }
            self.levels[level].push(run);
            if self.levels[level].len() < MERGE_FAN_IN {
                break;
            }
            let group = std::mem::take(&mut self.levels[level]);
            run = merge_runs(&self.path, group)?;
            level += 1;
        }
        Ok(())
    }

    /// run を逐次 merge して新しい本体を公開する。
    ///
    /// # Errors
    /// 重複局面、I/O エラー、保存先の競合、以前の入力失敗がある場合に返す。
    pub fn finish(mut self) -> Result<(), BookError> {
        if self.failed {
            return Err(invalid("native builder previously failed"));
        }
        self.spill()?;
        let mut runs: Vec<_> = self.levels.drain(..).flatten().collect();
        while runs.len() > 1 {
            let count = runs.len().min(MERGE_FAN_IN);
            let group = runs.split_off(runs.len() - count);
            runs.push(merge_runs(&self.path, group)?);
        }
        if let Some(run) = runs.pop() {
            let records = RunRecords { cursor: RunCursor::new(run)?, failed: false };
            write_sorted_with_block_positions(
                &self.path,
                self.metadata,
                records,
                false,
                self.block_positions,
            )
        } else {
            write_sorted_with_block_positions(
                &self.path,
                self.metadata,
                std::iter::empty(),
                false,
                self.block_positions,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{self, MoveList, NonEvasionsAll, generate_moves};
    use crate::book::native::{NativeBook, NativeCandidate};
    use crate::types::Move;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = temporary_path(&std::env::temp_dir().join("rsshogi-native-builder"), "test");
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            for name in [
                "sorted.book",
                "merged.book",
                "existing.book",
                "bad.book",
                "empty.book",
                "replace.book",
            ] {
                let _ = fs::remove_file(self.path(name));
            }
            fs::remove_dir(&self.0).expect("builder must clean its temporary files");
        }
    }

    fn fixture_record() -> NativeBookPosition {
        let position = board::hirate_position();
        let mut first = NativeCandidate::new(Move::from_usi("7g7f").unwrap());
        first.display_order = 1;
        let second = NativeCandidate::new(Move::from_usi("2g2f").unwrap());
        let entry = NativeBookEntry::from_position(&position, vec![first, second]).unwrap();
        let mut record = NativeBookPosition::new(entry);
        record.comment = Some("position\r\ncomment".into());
        record.move_comments.insert(Move::from_usi("7g7f").unwrap().raw(), "move comment".into());
        record
    }

    #[test]
    fn snapshot_exact_layout_and_malformed_ranges() {
        let record = fixture_record();
        let bytes = encode_snapshot(&record).unwrap();
        assert_eq!(read_u64(&bytes, 0).unwrap(), 2);
        assert_eq!(read_u64(&bytes, 8).unwrap(), 28);
        assert_eq!(bytes.len(), 16 + 32 + 32 + 28);
        let offsets: Vec<_> =
            (0..4).map(|index| read_u64(&bytes, 48 + index * 8).unwrap()).collect();
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[1], 16);
        assert_eq!(offsets[3], 28);
        assert_eq!(
            decode_snapshot(record.entry.key, &bytes).unwrap(),
            record.normalized().unwrap()
        );
        for len in 0..bytes.len() {
            assert!(decode_snapshot(fixture_record().entry.key, &bytes[..len]).is_err());
        }
        for (offset, value) in [(0, u64::MAX), (8, 0), (48, 1), (56, 29), (72, 27)] {
            let mut corrupt = bytes.clone();
            corrupt[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
            assert!(decode_snapshot(fixture_record().entry.key, &corrupt).is_err());
        }
        let mut corrupt = bytes;
        corrupt[80] = 255;
        assert!(decode_snapshot(fixture_record().entry.key, &corrupt).is_err());
        corrupt[80] = b'\r';
        assert!(decode_snapshot(fixture_record().entry.key, &corrupt).is_err());
    }

    #[test]
    fn sorted_base_preserves_fields_and_refuses_overwrite_or_bad_order() {
        let directory = TestDirectory::new();
        let path = directory.path("sorted.book");
        let metadata =
            NativeBookMetadata { title: Some("title\r\nline".into()), ..Default::default() };
        let record = fixture_record();
        write_sorted(&path, metadata.clone(), [Ok(record.clone())], false).unwrap();
        let book = NativeBook::open(&path).unwrap();
        book.validate_full().unwrap();
        assert_eq!(book.metadata().unwrap(), metadata.normalized().unwrap());
        assert_eq!(book.records().next().unwrap().unwrap(), record.clone().normalized().unwrap());
        let bytes = fs::read(&path).unwrap();
        assert!(
            write_sorted(&path, NativeBookMetadata::default(), std::iter::empty(), false).is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), bytes);
        let bad = directory.path("bad.book");
        assert!(write_sorted(&bad, metadata, [Ok(record.clone()), Ok(record)], false).is_err());
        assert!(!bad.exists());
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }

    #[test]
    fn external_merge_matches_sorted_bytes_and_cleans_runs() {
        let directory = TestDirectory::new();
        let position = board::hirate_position();
        let mut moves = MoveList::new();
        generate_moves::<NonEvasionsAll>(&position, &mut moves);
        let mut records = Vec::new();
        for &mv in moves.iter() {
            if !position.is_legal_move(mv) {
                continue;
            }
            let mut child = position.clone();
            child.apply_move32(child.move32_from_move(mv));
            let mut record =
                NativeBookPosition::new(NativeBookEntry::from_position(&child, vec![]).unwrap());
            record.comment = Some("x".repeat(64 * 1024));
            records.push(record);
        }
        assert!(records.len() > MERGE_FAN_IN);
        records.sort_by_key(|record| record.entry.key);
        let sorted = directory.path("sorted.book");
        write_sorted(
            &sorted,
            NativeBookMetadata::default(),
            records.iter().cloned().map(Ok),
            false,
        )
        .unwrap();
        let merged = directory.path("merged.book");
        let mut builder = NativeBookBuilder::new(&merged, NativeBookMetadata::default())
            .unwrap()
            .with_memory_budget(MIN_MEMORY_BYTES)
            .unwrap();
        for record in records.into_iter().rev() {
            builder.push(record).unwrap();
        }
        assert!(!builder.levels.is_empty());
        builder.finish().unwrap();
        assert_eq!(fs::read(&sorted).unwrap(), fs::read(&merged).unwrap());
        let book = NativeBook::open(&merged).unwrap();
        book.validate_full().unwrap();
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
    }

    #[test]
    fn duplicate_across_runs_and_drop_leave_no_output() {
        let directory = TestDirectory::new();
        let path = directory.path("bad.book");
        let mut record = fixture_record();
        record.comment = Some("x".repeat(64 * 1024));
        let mut builder = NativeBookBuilder::new(&path, NativeBookMetadata::default())
            .unwrap()
            .with_memory_budget(MIN_MEMORY_BYTES)
            .unwrap();
        builder.push(record.clone()).unwrap();
        builder.push(record).unwrap();
        assert!(builder.finish().is_err());
        assert!(!path.exists());
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
        let mut builder = NativeBookBuilder::new(&path, NativeBookMetadata::default()).unwrap();
        builder.push(fixture_record()).unwrap();
        builder.spill().unwrap();
        drop(builder);
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
    }

    #[test]
    fn block_positions_defaults_boundaries_and_configuration_timing() {
        let directory = TestDirectory::new();
        let path = directory.path("default.book");
        NativeBookBuilder::new(&path, NativeBookMetadata::default()).unwrap().finish().unwrap();
        assert_eq!(NativeBook::open(&path).unwrap().block_positions(), 64);
        fs::remove_file(&path).unwrap();
        for size in [0, 16_385, u32::MAX] {
            let path = directory.path(&format!("invalid-{size}.book"));
            assert!(
                NativeBookBuilder::new(&path, NativeBookMetadata::default())
                    .unwrap()
                    .with_block_positions(size)
                    .is_err()
            );
            assert!(!path.exists());
        }
        for size in [1, 256, 1024, 1025, 16_384] {
            let path = directory.path(&format!("empty-{size}.book"));
            NativeBookBuilder::new(&path, NativeBookMetadata::default())
                .unwrap()
                .with_block_positions(size)
                .unwrap()
                .finish()
                .unwrap();
            let book = NativeBook::open(&path).unwrap();
            book.validate_full().unwrap();
            assert_eq!(book.block_positions(), size);
            drop(book);
            fs::remove_file(&path).unwrap();
        }
        let path = directory.path("started.book");
        let mut builder = NativeBookBuilder::new(&path, NativeBookMetadata::default()).unwrap();
        builder.push(fixture_record()).unwrap();
        assert!(builder.with_block_positions(256).is_err());
        assert!(!path.exists());
        let mut builder = NativeBookBuilder::new(&path, NativeBookMetadata::default()).unwrap();
        builder.push(fixture_record()).unwrap();
        builder.spill().unwrap();
        assert!(builder.with_block_positions(256).is_err());
    }

    #[test]
    fn empty_builder_and_budget_errors() {
        let directory = TestDirectory::new();
        let path = directory.path("empty.book");
        NativeBookBuilder::new(&path, NativeBookMetadata::default()).unwrap().finish().unwrap();
        assert_eq!(fs::metadata(&path).unwrap().len(), 128);
        NativeBook::open(&path).unwrap().validate_full().unwrap();
        assert!(NativeBookBuilder::new(&path, NativeBookMetadata::default()).is_err());
        let path = directory.path("bad.book");
        assert!(
            NativeBookBuilder::new(&path, NativeBookMetadata::default())
                .unwrap()
                .with_memory_budget(0)
                .is_err()
        );
        let mut builder = NativeBookBuilder::new(&path, NativeBookMetadata::default())
            .unwrap()
            .with_memory_budget(MIN_MEMORY_BYTES)
            .unwrap();
        let mut large = fixture_record();
        large.comment = Some("x".repeat(256 * 1024));
        assert!(builder.push(large).is_err());
        assert!(builder.finish().is_err());
        assert!(!path.exists());
    }

    #[test]
    fn failed_replacement_retains_complete_old_or_new_snapshot() {
        let directory = TestDirectory::new();
        let path = directory.path("replace.book");
        for fail_at in ["written", "flushed", "synced", "validated", "published"] {
            write_sorted(&path, NativeBookMetadata::default(), [Ok(fixture_record())], true)
                .unwrap();
            let old = fs::read(&path).unwrap();
            let mut changed = fixture_record();
            changed.comment = Some("replacement".into());
            let expected = changed.clone().normalized().unwrap();
            let error = write_sorted_with(
                &path,
                NativeBookMetadata::default(),
                [Ok(changed)],
                true,
                wire::BLOCK_POSITIONS,
                |phase| {
                    if phase == fail_at {
                        Err(std::io::Error::other("injected publication failure").into())
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
            let book = NativeBook::open(&path).unwrap();
            book.validate_full().unwrap();
            if fail_at == "published" {
                assert!(matches!(error, BookError::SaveUncertain(_)));
                assert_eq!(book.records().next().unwrap().unwrap(), expected);
            } else {
                assert_eq!(fs::read(&path).unwrap(), old);
            }
            assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
        }
    }
}
