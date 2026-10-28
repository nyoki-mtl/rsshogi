use std::collections::{BTreeMap, btree_map};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::block::BlockIndex;
use super::crc::{Crc32c, crc32c};
use super::wire::{self, CommentIndex, Header, decode_candidate, decode_metadata, read_u64};
use super::{
    NativeBookEntry, NativeBookIoStats, NativeBookMetadata, NativeBookPosition, NativeBookProgress,
    NativeBookTail, add, invalid, mul,
};
use crate::board::{self, Position};
use crate::book::{BookControl, BookError};

#[derive(Debug)]
pub(super) struct ReadFile {
    file: Mutex<File>,
    pub len: u64,
    calls: AtomicU64,
    bytes: AtomicU64,
    seeks: AtomicU64,
    reads: AtomicU64,
}

impl ReadFile {
    fn open(path: &Path) -> Result<Self, BookError> {
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        Ok(Self {
            file: Mutex::new(file),
            len,
            calls: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            seeks: AtomicU64::new(0),
            reads: AtomicU64::new(0),
        })
    }

    pub(super) fn read(&self, offset: u64, len: u64) -> Result<Vec<u8>, BookError> {
        if add(offset, len)? > self.len {
            return Err(invalid("native book read exceeds captured length"));
        }
        let len = usize::try_from(len).map_err(|_| invalid("native book size exceeds usize"))?;
        let mut bytes = vec![0; len];
        let mut file = self.file.lock().map_err(|_| invalid("native book file lock poisoned"))?;
        self.seeks.fetch_add(1, Ordering::Relaxed);
        file.seek(SeekFrom::Start(offset))?;
        let mut remaining = bytes.as_mut_slice();
        while !remaining.is_empty() {
            self.reads.fetch_add(1, Ordering::Relaxed);
            match file.read(remaining) {
                Ok(0) => return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into()),
                Ok(count) => remaining = &mut remaining[count..],
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.bytes.fetch_add(len as u64, Ordering::Relaxed);
        Ok(bytes)
    }

    pub(super) fn checksum(&self, start: u64, end: u64) -> Result<u32, BookError> {
        let mut crc = Crc32c::new();
        let mut offset = start;
        while offset < end {
            let len = (end - offset).min(64 * 1024);
            crc.update(&self.read(offset, len)?);
            offset += len;
        }
        Ok(crc.finish())
    }
}

/// Native Bookのopen時設定。
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeBookOpenOptions {
    /// open時に本体全件も検証する。
    pub validate_full: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Delta {
    Deleted,
    Snapshot { offset: u64, len: u64 },
}

type BlockCache = Mutex<Option<(usize, Vec<NativeBookEntry>)>>;

/// 本体をRAMへ展開せず、open時点のsnapshotを部分読込する。
#[derive(Clone, Debug)]
pub struct NativeBook {
    pub(super) file: Arc<ReadFile>,
    pub(super) header: Header,
    blocks: Arc<Vec<BlockIndex>>,
    block_cache: Arc<BlockCache>,
    pub(super) delta: BTreeMap<[u8; 32], Delta>,
    metadata_delta: Option<(u64, u64)>,
    pub(super) sequence: u64,
    pub(super) valid_end: u64,
    tail: Option<NativeBookTail>,
}

impl NativeBook {
    /// ファイルに保存されたブロックあたりの局面数を返す。
    #[must_use]
    pub fn block_positions(&self) -> u32 {
        self.header.block_positions
    }

    /// v4 の snapshotを開く。未完了tailは診断に残す。
    pub fn open(path: impl AsRef<Path>) -> Result<Self, BookError> {
        Self::open_with_options(path, NativeBookOpenOptions::default())
    }

    /// 検証設定を指定して開く。
    pub fn open_with_options(
        path: impl AsRef<Path>,
        options: NativeBookOpenOptions,
    ) -> Result<Self, BookError> {
        Self::open_captured(path.as_ref(), options, None)
    }

    pub(super) fn open_prefix(path: &Path, end: u64) -> Result<Self, BookError> {
        Self::open_captured(path, NativeBookOpenOptions::default(), Some(end))
    }

    fn open_captured(
        path: &Path,
        options: NativeBookOpenOptions,
        end: Option<u64>,
    ) -> Result<Self, BookError> {
        board::init();
        let mut file = ReadFile::open(path)?;
        if let Some(end) = end {
            if end > file.len {
                return Err(invalid("prefix exceeds file length"));
            }
            file.len = end;
        }
        let file = Arc::new(file);
        if file.len < 128 {
            return Err(invalid("truncated native book header"));
        }
        let header = Header::decode(&file.read(0, 128)?)?;
        if header.base_end > file.len {
            return Err(invalid("truncated native book base"));
        }
        let block_count = header.node_count.div_ceil(u64::from(header.block_positions));
        let index_bytes = file
            .read(header.block_index_offset, mul(block_count, wire::BLOCK_INDEX_BYTES as u64)?)?;
        let mut blocks = Vec::new();
        let mut expected_offset = 0;
        let mut previous = None;
        for bytes in index_bytes.chunks_exact(wire::BLOCK_INDEX_BYTES) {
            let row = BlockIndex::decode(bytes)?;
            if row.offset != expected_offset || previous.is_some_and(|key| key >= row.key) {
                return Err(invalid("invalid native block index ordering"));
            }
            expected_offset = add(expected_offset, u64::from(row.compressed_bytes))?;
            if expected_offset > header.blocks_bytes {
                return Err(invalid("native block exceeds data region"));
            }
            previous = Some(row.key);
            blocks.push(row);
        }
        if expected_offset != header.blocks_bytes {
            return Err(invalid("unreferenced native compressed bytes"));
        }
        let mut book = Self {
            blocks: Arc::new(blocks),
            block_cache: Arc::new(Mutex::new(None)),
            valid_end: header.base_end,
            file,
            header,
            delta: BTreeMap::new(),
            metadata_delta: None,
            sequence: 0,
            tail: None,
        };
        book.replay()?;
        if options.validate_full {
            book.validate_full()?;
        }
        Ok(book)
    }

    fn log_error(&self, offset: u64, reason: &'static str) -> BookError {
        BookError::NativeLog { offset, valid_end: self.valid_end, sequence: self.sequence, reason }
    }

    fn replay(&mut self) -> Result<(), BookError> {
        let mut offset = self.header.base_end;
        while offset < self.file.len {
            if self.file.len - offset < 40 {
                return Err(self.log_error(offset, "unverified short transaction header"));
            }
            let header = self.file.read(offset, 40)?;
            let mut crc_header = header.clone();
            crc_header[32..36].fill(0);
            let expected_crc = u32::from_le_bytes(header[32..36].try_into().expect("four bytes"));
            if &header[..8] != b"RSSHGTX4"
                || crc32c(&crc_header) != expected_crc
                || header[36..40] != [0; 4]
            {
                return Err(self.log_error(offset, "invalid transaction header"));
            }
            let sequence = read_u64(&header, 8)?;
            let payload_len = read_u64(&header, 16)?;
            let operation_count = read_u64(&header, 24)?;
            if sequence
                != self
                    .sequence
                    .checked_add(1)
                    .ok_or_else(|| self.log_error(offset, "sequence overflow"))?
                || payload_len > wire::MAX_TX_BYTES - 64
                || operation_count == 0
                || operation_count > payload_len / 48
            {
                return Err(self.log_error(offset, "invalid transaction sequence or size"));
            }
            let end = add(add(offset, 64)?, payload_len)?;
            if end > self.file.len {
                self.tail = Some(NativeBookTail { offset, end: self.file.len });
                break;
            }
            let payload_end = end - 24;
            let footer = self.file.read(payload_end, 24)?;
            let expected = u32::from_le_bytes(footer[16..20].try_into().expect("four bytes"));
            if &footer[..8] != b"RSSHGCMT"
                || read_u64(&footer, 8)? != sequence
                || footer[20..24] != [0; 4]
                || self.file.checksum(offset, payload_end)? != expected
            {
                return Err(self.log_error(offset, "invalid transaction footer or checksum"));
            }
            // frameを完全に検証してから索引へ一括適用する。
            let operations = self
                .parse_operations(offset + 40, payload_end, operation_count)
                .map_err(|_| self.log_error(offset, "invalid transaction operations"))?;
            for (key, value) in operations.positions {
                self.delta.insert(key, value);
            }
            if let Some(metadata) = operations.metadata {
                self.metadata_delta = Some(metadata);
            }
            self.sequence = sequence;
            self.valid_end = end;
            offset = end;
        }
        Ok(())
    }

    fn parse_operations(
        &self,
        mut offset: u64,
        end: u64,
        count: u64,
    ) -> Result<Operations, BookError> {
        let mut positions = Vec::new();
        let mut metadata = None;
        let mut previous = None;
        for index in 0..count {
            if add(offset, 48)? > end {
                return Err(invalid("short operation prefix"));
            }
            let prefix = self.file.read(offset, 48)?;
            if prefix[1..8] != [0; 7] {
                return Err(invalid("operation reserved bytes"));
            }
            let key: [u8; 32] = prefix[8..40].try_into().expect("key bytes");
            let len = read_u64(&prefix, 40)?;
            offset += 48;
            if add(offset, len)? > end {
                return Err(invalid("operation exceeds payload"));
            }
            if prefix[0] == 3 {
                if index != 0 || key != [0; 32] {
                    return Err(invalid("invalid metadata operation"));
                }
                self.metadata_at(offset, len)?;
                metadata = Some((offset, len));
            } else {
                if previous.is_some_and(|previous| previous >= key) {
                    return Err(invalid("operation keys not strictly sorted"));
                }
                previous = Some(key);
                let delta = match prefix[0] {
                    1 => {
                        self.snapshot_layout(offset, len)?;
                        self.snapshot_entry(key, offset, len)?;
                        Delta::Snapshot { offset, len }
                    }
                    2 if len == 0 => {
                        NativeBookEntry { key, candidates: vec![] }.position()?;
                        Delta::Deleted
                    }
                    _ => return Err(invalid("invalid operation opcode")),
                };
                positions.push((key, delta));
            }
            offset = add(offset, len)?;
        }
        if offset != end {
            return Err(invalid("extra operation payload bytes"));
        }
        Ok(Operations { positions, metadata })
    }

    /// 正常な開始ヘッダーの後ろにある未完了tail。
    pub fn tail(&self) -> Option<NativeBookTail> {
        self.tail
    }

    /// 最後に適用したtransactionの連番。永続IDではない。
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(super) fn accept_saved(
        &mut self,
        path: &Path,
        sequence: u64,
        end: u64,
        positions: Vec<([u8; 32], Delta)>,
        metadata: Option<(u64, u64)>,
    ) -> Result<(), BookError> {
        self.file = Arc::new(ReadFile::open(path)?);
        if self.file.len != end {
            return Err(invalid("file changed during native save"));
        }
        for (key, delta) in positions {
            self.delta.insert(key, delta);
        }
        if let Some(metadata) = metadata {
            self.metadata_delta = Some(metadata);
        }
        self.sequence = sequence;
        self.valid_end = end;
        self.tail = None;
        Ok(())
    }

    /// I/O要求の累計。
    pub fn io_stats(&self) -> NativeBookIoStats {
        NativeBookIoStats {
            read_calls: self.file.calls.load(Ordering::Relaxed),
            read_bytes: self.file.bytes.load(Ordering::Relaxed),
            seek_calls: self.file.seeks.load(Ordering::Relaxed),
            read_syscalls: self.file.reads.load(Ordering::Relaxed),
        }
    }

    /// 候補手だけを取得する。コメント本文は読まない。
    pub fn get(&self, position: &Position) -> Result<Option<NativeBookEntry>, BookError> {
        let key = *position
            .try_to_packed_sfen()
            .map_err(|err| BookError::InvalidData(err.to_string()))?
            .as_bytes();
        self.get_key(key)
    }

    /// 問い合わせた向きの候補手を返し、直接一致がなければ反転局面を照会する。
    ///
    /// 候補手が空でも直接一致を優先する。反転は盤面の 180 度回転と先後の交換を指す。
    /// 返すキーと指し手は問い合わせた局面の向きに揃え、手番視点の評価値、深さ、表示順位を保持する。
    /// 評価値の採用条件や候補手の選択方針は呼び出し側が決める。
    ///
    /// # Errors
    /// 局面を符号化できない場合、または照会に失敗した場合に返す。
    pub fn get_with_flipped(
        &self,
        position: &Position,
    ) -> Result<Option<NativeBookEntry>, BookError> {
        if let Some(entry) = self.get(position)? {
            return Ok(Some(entry));
        }
        let flipped_key = *position
            .try_to_packed_sfen_flipped()
            .map_err(|err| BookError::InvalidData(err.to_string()))?
            .as_bytes();
        let Some(mut entry) = self.get_key(flipped_key)? else {
            return Ok(None);
        };
        entry.key = *position
            .try_to_packed_sfen()
            .map_err(|err| BookError::InvalidData(err.to_string()))?
            .as_bytes();
        for candidate in &mut entry.candidates {
            candidate.mv = candidate.mv.flip();
        }
        entry.candidates.sort_unstable_by_key(|candidate| candidate.mv.raw());
        Ok(Some(entry))
    }

    /// 正規化PackedSfenキーで候補手だけを取得する。
    pub fn get_key(&self, key: [u8; 32]) -> Result<Option<NativeBookEntry>, BookError> {
        match self.delta.get(&key) {
            Some(Delta::Deleted) => Ok(None),
            Some(Delta::Snapshot { offset, len }) => {
                self.snapshot_entry(key, *offset, *len).map(Some)
            }
            None => self.find_base(key)?.map(|index| self.base_entry(index, key)).transpose(),
        }
    }

    fn with_block<T>(
        &self,
        block_id: usize,
        use_entries: impl FnOnce(&[NativeBookEntry]) -> Result<T, BookError>,
    ) -> Result<T, BookError> {
        let mut cache =
            self.block_cache.lock().map_err(|_| invalid("native block cache lock poisoned"))?;
        if cache.as_ref().is_none_or(|(id, _)| *id != block_id) {
            let row =
                self.blocks.get(block_id).ok_or_else(|| invalid("invalid native block index"))?;
            let compressed = self.file.read(
                add(self.header.blocks_offset, row.offset)?,
                u64::from(row.compressed_bytes),
            )?;
            let first = block_id as u64 * u64::from(self.header.block_positions);
            let count =
                (self.header.node_count - first).min(u64::from(self.header.block_positions));
            let entries = super::block::decode(row, &compressed, count)?;
            if self
                .blocks
                .get(block_id + 1)
                .is_some_and(|next| entries.last().is_some_and(|entry| entry.key >= next.key))
            {
                return Err(invalid("overlapping native block keys"));
            }
            *cache = Some((block_id, entries));
        }
        use_entries(&cache.as_ref().expect("decoded block cached").1)
    }

    pub(super) fn base_key(&self, index: u64) -> Result<[u8; 32], BookError> {
        let size = u64::from(self.header.block_positions);
        self.with_block((index / size) as usize, |entries| {
            entries
                .get((index % size) as usize)
                .map(|entry| entry.key)
                .ok_or_else(|| invalid("invalid native position index"))
        })
    }

    pub(super) fn find_base(&self, key: [u8; 32]) -> Result<Option<u64>, BookError> {
        let end = self.blocks.partition_point(|row| row.key <= key);
        if end == 0 {
            return Ok(None);
        }
        let block_id = end - 1;
        self.with_block(block_id, |entries| {
            Ok(entries.binary_search_by_key(&key, |entry| entry.key).ok().map(|index| {
                block_id as u64 * u64::from(self.header.block_positions) + index as u64
            }))
        })
    }

    fn base_entry(&self, index: u64, key: [u8; 32]) -> Result<NativeBookEntry, BookError> {
        self.base_entry_with_position(index, key).map(|(entry, _)| entry)
    }

    fn base_entry_with_position(
        &self,
        index: u64,
        key: [u8; 32],
    ) -> Result<(NativeBookEntry, Position), BookError> {
        let size = u64::from(self.header.block_positions);
        self.with_block((index / size) as usize, |entries| {
            let entry = entries
                .get((index % size) as usize)
                .filter(|entry| entry.key == key)
                .cloned()
                .ok_or_else(|| invalid("invalid native position index"))?;
            let position = entry.validate_with_position()?;
            Ok((entry, position))
        })
    }

    fn entry_from_bytes(
        &self,
        key: [u8; 32],
        bytes: &[u8],
    ) -> Result<(NativeBookEntry, Position), BookError> {
        let candidates =
            bytes.chunks_exact(16).map(decode_candidate).collect::<Result<Vec<_>, _>>()?;
        let entry = NativeBookEntry { key, candidates };
        let position = entry.validate_with_position()?;
        Ok((entry, position))
    }

    fn snapshot_layout(&self, offset: u64, len: u64) -> Result<(u64, u64, u64), BookError> {
        if len < 32 {
            return Err(invalid("short position snapshot"));
        }
        let header = self.file.read(offset, 16)?;
        let count = read_u64(&header, 0)?;
        let text_len = read_u64(&header, 8)?;
        if count > u16::MAX as u64 {
            return Err(invalid("too many candidates"));
        }
        let offsets_offset = add(offset + 16, mul(count, 16)?)?;
        let text_offset = add(offsets_offset, mul(count + 2, 8)?)?;
        if add(text_offset - offset, text_len)? != len {
            return Err(invalid("snapshot size mismatch"));
        }
        let offsets = self.file.read(offsets_offset, mul(count + 2, 8)?)?;
        let mut previous = 0;
        for index in 0..count + 2 {
            let value = read_u64(&offsets, index as usize * 8)?;
            if value < previous
                || value > text_len
                || value - previous > wire::MAX_TEXT_BYTES as u64
                || (index == 0 && value != 0)
            {
                return Err(invalid("invalid snapshot comment offsets"));
            }
            previous = value;
        }
        if previous != text_len {
            return Err(invalid("invalid snapshot text length"));
        }
        Ok((count, offsets_offset, text_offset))
    }

    fn snapshot_entry(
        &self,
        key: [u8; 32],
        offset: u64,
        len: u64,
    ) -> Result<NativeBookEntry, BookError> {
        self.snapshot_entry_with_position(key, offset, len).map(|(entry, _)| entry)
    }

    fn snapshot_entry_with_position(
        &self,
        key: [u8; 32],
        offset: u64,
        len: u64,
    ) -> Result<(NativeBookEntry, Position), BookError> {
        // 検索では候補手だけを読み、コメントoffset検査はopen時と完全検証で行う。
        let header = self.file.read(offset, 16)?;
        let count = read_u64(&header, 0)?;
        if count > u16::MAX as u64 || add(16, mul(count, 16)?)? > len {
            return Err(invalid("invalid snapshot candidate count"));
        }
        let entry = self.entry_from_bytes(key, &self.file.read(offset + 16, mul(count, 16)?)?)?;
        Ok(entry)
    }

    fn text(&self, offset: u64, len: u64) -> Result<Option<String>, BookError> {
        if len > wire::MAX_TEXT_BYTES as u64 {
            return Err(invalid("native text exceeds limit"));
        }
        if len == 0 {
            return Ok(None);
        }
        let text = String::from_utf8(self.file.read(offset, len)?)
            .map_err(|_| invalid("invalid native UTF-8"))?;
        if text.contains('\r') {
            return Err(invalid("native text is not LF normalized"));
        }
        Ok(Some(text))
    }

    fn comment_index(&self, index: u64) -> Result<CommentIndex, BookError> {
        CommentIndex::decode(
            &self.file.read(add(self.header.comment_index_offset, mul(index, 32)?)?, 32)?,
        )
    }

    fn base_comment(
        &self,
        position_id: u64,
        target_kind: u8,
        move_raw: u16,
    ) -> Result<Option<String>, BookError> {
        let target = (position_id, target_kind, move_raw);
        let mut low = 0;
        let mut high = self.header.comment_count;
        while low < high {
            let middle = low + (high - low) / 2;
            let row = self.comment_index(middle)?;
            match (row.position_id, row.target_kind, row.move_raw).cmp(&target) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return self.comment_text(&row),
            }
        }
        Ok(None)
    }

    fn comment_text(&self, row: &CommentIndex) -> Result<Option<String>, BookError> {
        if add(row.offset, row.len)? > self.header.comment_text_bytes {
            return Err(invalid("comment exceeds text area"));
        }
        self.text(add(self.header.comment_text_offset, row.offset)?, row.len)
    }

    fn snapshot_record(
        &self,
        key: [u8; 32],
        offset: u64,
        len: u64,
    ) -> Result<NativeBookPosition, BookError> {
        self.snapshot_record_with_position(key, offset, len).map(|(record, _)| record)
    }

    fn snapshot_record_with_position(
        &self,
        key: [u8; 32],
        offset: u64,
        len: u64,
    ) -> Result<(NativeBookPosition, Position), BookError> {
        let (count, offsets_offset, text_offset) = self.snapshot_layout(offset, len)?;
        let (entry, position) = self.snapshot_entry_with_position(key, offset, len)?;
        let offsets = self.file.read(offsets_offset, mul(count + 2, 8)?)?;
        let mut record = NativeBookPosition::new(entry);
        for index in 0..count + 1 {
            let start = read_u64(&offsets, index as usize * 8)?;
            let end = read_u64(&offsets, (index as usize + 1) * 8)?;
            if let Some(text) = self.text(add(text_offset, start)?, end - start)? {
                if index == 0 {
                    record.comment = Some(text);
                } else {
                    record
                        .move_comments
                        .insert(record.entry.candidates[index as usize - 1].mv.raw(), text);
                }
            }
        }
        Ok((record, position))
    }

    /// コメントを含む1局面を取得する。
    pub fn record(&self, position: &Position) -> Result<Option<NativeBookPosition>, BookError> {
        let key = *position
            .try_to_packed_sfen()
            .map_err(|err| BookError::InvalidData(err.to_string()))?
            .as_bytes();
        self.record_key(key)
    }

    pub(super) fn record_key(
        &self,
        key: [u8; 32],
    ) -> Result<Option<NativeBookPosition>, BookError> {
        match self.delta.get(&key) {
            Some(Delta::Deleted) => Ok(None),
            Some(Delta::Snapshot { offset, len }) => {
                self.snapshot_record(key, *offset, *len).map(Some)
            }
            None => {
                let Some(index) = self.find_base(key)? else {
                    return Ok(None);
                };
                let mut record = NativeBookPosition::new(self.base_entry(index, key)?);
                record.comment = self.base_comment(index, 0, 0)?;
                for candidate in &record.entry.candidates {
                    if let Some(text) = self.base_comment(index, 1, candidate.mv.raw())? {
                        record.move_comments.insert(candidate.mv.raw(), text);
                    }
                }
                Ok(Some(record))
            }
        }
    }

    /// 局面コメントを取得する。
    pub fn position_comment(&self, position: &Position) -> Result<Option<String>, BookError> {
        self.comment_for(position, None)
    }

    /// 候補手コメントを取得する。
    pub fn move_comment(
        &self,
        position: &Position,
        mv: crate::types::Move,
    ) -> Result<Option<String>, BookError> {
        self.comment_for(position, Some(mv))
    }

    fn comment_for(
        &self,
        position: &Position,
        mv: Option<crate::types::Move>,
    ) -> Result<Option<String>, BookError> {
        let key = *position
            .try_to_packed_sfen()
            .map_err(|error| BookError::InvalidData(error.to_string()))?
            .as_bytes();
        match self.delta.get(&key) {
            Some(Delta::Deleted) => Ok(None),
            Some(Delta::Snapshot { offset, len }) => {
                let (_, offsets_offset, text_offset) = self.snapshot_layout(*offset, *len)?;
                let index = match mv {
                    None => 0,
                    Some(mv) => {
                        let entry = self.snapshot_entry(key, *offset, *len)?;
                        let Some(index) =
                            entry.candidates.iter().position(|candidate| candidate.mv == mv)
                        else {
                            return Ok(None);
                        };
                        index as u64 + 1
                    }
                };
                let pair = self.file.read(add(offsets_offset, mul(index, 8)?)?, 16)?;
                let start = read_u64(&pair, 0)?;
                let end = read_u64(&pair, 8)?;
                self.text(add(text_offset, start)?, end - start)
            }
            None => {
                let Some(index) = self.find_base(key)? else { return Ok(None) };
                if let Some(mv) = mv {
                    let entry = self.base_entry(index, key)?;
                    if !entry.candidates.iter().any(|candidate| candidate.mv == mv) {
                        return Ok(None);
                    }
                }
                self.base_comment(index, u8::from(mv.is_some()), mv.map_or(0, |mv| mv.raw()))
            }
        }
    }

    fn metadata_at(&self, offset: u64, len: u64) -> Result<NativeBookMetadata, BookError> {
        if !(20..=20 + 2 * wire::MAX_TEXT_BYTES as u64).contains(&len) {
            return Err(invalid("invalid metadata length"));
        }
        decode_metadata(&self.file.read(offset, len)?)
    }

    /// 文書メタデータを必要時にだけ読む。
    pub fn metadata(&self) -> Result<NativeBookMetadata, BookError> {
        match self.metadata_delta {
            Some((offset, len)) => self.metadata_at(offset, len),
            None if self.header.metadata_bytes != 0 => {
                let metadata =
                    self.metadata_at(self.header.metadata_offset, self.header.metadata_bytes)?;
                if metadata.depth_kind != self.header.depth_kind {
                    return Err(invalid("native metadata depth kind mismatch"));
                }
                Ok(metadata)
            }
            None => {
                Ok(NativeBookMetadata { depth_kind: self.header.depth_kind, ..Default::default() })
            }
        }
    }

    /// 候補手をキー順に走査する。本文は読み込まない。
    pub fn entries(&self) -> NativeBookEntries<'_> {
        NativeBookEntries { locations: Locations::new(self) }
    }

    /// コメントを含めてキー順に走査する。
    pub fn records(&self) -> NativeBookRecords<'_> {
        NativeBookRecords { locations: Locations::new(self), comment_cursor: 0 }
    }

    /// snapshotの局面数。差分がある場合はその本体存在確認を行う。
    pub fn len(&self) -> Result<u64, BookError> {
        let mut len = self.header.node_count;
        for (key, delta) in &self.delta {
            let exists = self.find_base(*key)?.is_some();
            match (exists, delta) {
                (true, Delta::Deleted) => len -= 1,
                (false, Delta::Snapshot { .. }) => len += 1,
                _ => {}
            }
        }
        Ok(len)
    }

    /// snapshotが空かを調べる。
    pub fn is_empty(&self) -> Result<bool, BookError> {
        Ok(self.len()? == 0)
    }

    /// 全件のCRC、意味、索引を検証する。
    pub fn validate_full(&self) -> Result<(), BookError> {
        self.validate_full_with_control(|_| BookControl::Continue)
    }

    /// 進捗通知とキャンセル付きの全件検証。
    pub fn validate_full_with_control(
        &self,
        control: impl FnMut(NativeBookProgress) -> BookControl,
    ) -> Result<(), BookError> {
        self.validate_full_and_visit(|_, _| Ok(()), control)
    }

    /// 全件を完全検証し、現在の snapshot の各 entry と検証に使った局面を一度ずつ渡す。
    ///
    /// 呼出し順は保証しない。削除済みや上書き済みの base、過去の transaction、
    /// コメント、未完了 tail も検査する。全 entry を追加でメモリに保持しない。
    /// 局面は手数 1 であり、コールバック中だけ借用できる。
    /// コールバックはブロックキャッシュのロックを保持せずに呼び出す。
    ///
    /// コールバックの結果は、このメソッドが `Ok(())` を返すまで未完成として扱う。
    /// 後続の破損検出、キャンセル、コールバック失敗時に副作用を巻き戻さない。
    ///
    /// # Examples
    /// ```no_run
    /// use rsshogi::book::{BookControl, BookError, NativeBook};
    /// let book = NativeBook::open("input.book")?;
    /// let mut candidates = 0;
    /// book.validate_full_and_visit(
    ///     |entry, _position| {
    ///         candidates += entry.candidates.len();
    ///         Ok(())
    ///     },
    ///     |_| BookControl::Continue,
    /// )?;
    /// println!("validated candidates: {candidates}");
    /// # Ok::<_, BookError>(())
    /// ```
    ///
    /// # Errors
    /// データの不正や I/O 失敗、進捗通知からのキャンセル時に返す。
    /// コールバックのエラーは変換せずに返し、以後の処理を中断する。
    pub fn validate_full_and_visit(
        &self,
        mut visit: impl FnMut(NativeBookEntry, &Position) -> Result<(), BookError>,
        mut control: impl FnMut(NativeBookProgress) -> BookControl,
    ) -> Result<(), BookError> {
        let mut check = |phase, completed, total| {
            if control(NativeBookProgress { phase, completed, total }) == BookControl::Cancel {
                Err(BookError::Cancelled)
            } else {
                Ok(())
            }
        };
        check("checksum", 0, Some(self.header.base_end - 128))?;
        let mut crc = Crc32c::new();
        let mut offset = 128;
        while offset < self.header.base_end {
            let len = (self.header.base_end - offset).min(64 * 1024);
            crc.update(&self.file.read(offset, len)?);
            offset += len;
            check("checksum", offset - 128, Some(self.header.base_end - 128))?;
        }
        if crc.finish() != self.header.base_crc {
            return Err(invalid("native base checksum mismatch"));
        }
        let mut previous = None;
        let mut moves = 0u64;
        for index in 0..self.header.node_count {
            let key = self.base_key(index)?;
            if previous.is_some_and(|previous| previous >= key) {
                return Err(invalid("base keys not strictly sorted"));
            }
            let (entry, position) = self.base_entry_with_position(index, key)?;
            moves = add(moves, entry.candidates.len() as u64)?;
            if !self.delta.contains_key(&key) {
                visit(entry, &position)?;
            }
            previous = Some(key);
            check("positions", index + 1, Some(self.header.node_count))?;
        }
        if moves != self.header.move_count {
            return Err(invalid("native move count mismatch"));
        }
        let mut previous_comment = None;
        let mut text_end = 0;
        for index in 0..self.header.comment_count {
            let row = self.comment_index(index)?;
            let tuple = (row.position_id, row.target_kind, row.move_raw);
            if row.position_id >= self.header.node_count
                || previous_comment.is_some_and(|previous| previous >= tuple)
                || row.offset != text_end
            {
                return Err(invalid("invalid comment index ordering"));
            }
            if row.target_kind == 1 {
                let entry = self.base_entry(row.position_id, self.base_key(row.position_id)?)?;
                if !entry.candidates.iter().any(|candidate| candidate.mv.raw() == row.move_raw) {
                    return Err(invalid("comment references absent candidate"));
                }
            }
            self.comment_text(&row)?;
            text_end = add(row.offset, row.len)?;
            previous_comment = Some(tuple);
            check("comments", index + 1, Some(self.header.comment_count))?;
        }
        if text_end != self.header.comment_text_bytes {
            return Err(invalid("unreferenced comment text"));
        }
        if self.header.metadata_bytes != 0 {
            let metadata =
                self.metadata_at(self.header.metadata_offset, self.header.metadata_bytes)?;
            if metadata.depth_kind != self.header.depth_kind {
                return Err(invalid("native metadata depth kind mismatch"));
            }
            if metadata == NativeBookMetadata::default() {
                return Err(invalid("empty base metadata must be omitted"));
            }
        }
        let mut offset = self.header.base_end;
        let mut sequence = 0;
        while offset < self.valid_end {
            let header = self.file.read(offset, 40)?;
            let payload_end = add(offset + 40, read_u64(&header, 16)?)?;
            let count = read_u64(&header, 24)?;
            let mut cursor = offset + 40;
            for _ in 0..count {
                let current = (|| {
                    let prefix = self.file.read(cursor, 48)?;
                    let key = prefix[8..40].try_into().expect("key bytes");
                    let len = read_u64(&prefix, 40)?;
                    cursor += 48;
                    let current = match prefix[0] {
                        1 => {
                            let (record, position) =
                                self.snapshot_record_with_position(key, cursor, len)?;
                            if matches!(self.delta.get(&key), Some(Delta::Snapshot { offset, len: current_len }) if *offset == cursor && *current_len == len) {
                                Some((record.entry, position))
                            } else {
                                None
                            }
                        }
                        3 => {
                            self.metadata_at(cursor, len)?;
                            None
                        }
                        _ => None,
                    };
                    cursor = add(cursor, len)?;
                    Ok::<_, BookError>(current)
                })()
                .map_err(|_| BookError::NativeLog {
                    offset,
                    valid_end: offset,
                    sequence,
                    reason: "invalid historical transaction data",
                })?;
                if let Some((entry, position)) = current {
                    visit(entry, &position)?;
                }
            }
            sequence += 1;
            offset = add(payload_end, 24)?;
            check(
                "transactions",
                offset - self.header.base_end,
                Some(self.valid_end - self.header.base_end),
            )?;
        }
        if self.tail.is_some() {
            return Err(self.log_error(self.valid_end, "incomplete transaction tail"));
        }
        check("complete", self.file.len, Some(self.file.len))
    }
}

struct Operations {
    positions: Vec<([u8; 32], Delta)>,
    metadata: Option<(u64, u64)>,
}

#[derive(Clone, Copy)]
enum Location {
    Base(u64),
    Delta(u64, u64),
}

struct Locations<'a> {
    book: &'a NativeBook,
    index: u64,
    base_key: Option<[u8; 32]>,
    delta: btree_map::Iter<'a, [u8; 32], Delta>,
    next_delta: Option<(&'a [u8; 32], &'a Delta)>,
    failed: bool,
}

impl<'a> Locations<'a> {
    fn new(book: &'a NativeBook) -> Self {
        let mut delta = book.delta.iter();
        let next_delta = delta.next();
        Self { book, index: 0, base_key: None, delta, next_delta, failed: false }
    }

    fn next(&mut self) -> Option<Result<([u8; 32], Location), BookError>> {
        if self.failed {
            return None;
        }
        loop {
            if self.base_key.is_none() && self.index < self.book.header.node_count {
                match self.book.base_key(self.index) {
                    Ok(key) => self.base_key = Some(key),
                    Err(err) => {
                        self.failed = true;
                        return Some(Err(err));
                    }
                }
            }
            if let Some((key, delta)) = self.next_delta
                && self.base_key.is_none_or(|base| *key <= base)
            {
                let key = *key;
                let delta = *delta;
                self.next_delta = self.delta.next();
                if self.base_key == Some(key) {
                    self.index += 1;
                    self.base_key = None;
                }
                match delta {
                    Delta::Deleted => continue,
                    Delta::Snapshot { offset, len } => {
                        return Some(Ok((key, Location::Delta(offset, len))));
                    }
                }
            }
            if let Some(key) = self.base_key.take() {
                let index = self.index;
                self.index += 1;
                return Some(Ok((key, Location::Base(index))));
            }
            return None;
        }
    }
}

/// 本文を読まないキー順iterator。
pub struct NativeBookEntries<'a> {
    locations: Locations<'a>,
}

impl Iterator for NativeBookEntries<'_> {
    type Item = Result<NativeBookEntry, BookError>;
    fn next(&mut self) -> Option<Self::Item> {
        self.locations.next().map(|location| {
            let (key, location) = location?;
            match location {
                Location::Base(index) => self.locations.book.base_entry(index, key),
                Location::Delta(offset, len) => {
                    self.locations.book.snapshot_entry(key, offset, len)
                }
            }
        })
    }
}

/// コメント索引も順次走査する、所有snapshotのiterator。
pub struct NativeBookRecords<'a> {
    locations: Locations<'a>,
    comment_cursor: u64,
}

impl Iterator for NativeBookRecords<'_> {
    type Item = Result<NativeBookPosition, BookError>;
    fn next(&mut self) -> Option<Self::Item> {
        self.locations.next().map(|location| {
            let (key, location) = location?;
            let book = self.locations.book;
            match location {
                Location::Delta(offset, len) => book.snapshot_record(key, offset, len),
                Location::Base(index) => {
                    let mut record = NativeBookPosition::new(book.base_entry(index, key)?);
                    while self.comment_cursor < book.header.comment_count {
                        let row = book.comment_index(self.comment_cursor)?;
                        if row.position_id > index {
                            break;
                        }
                        self.comment_cursor += 1;
                        if row.position_id < index {
                            continue;
                        }
                        let text = book
                            .comment_text(&row)?
                            .ok_or_else(|| invalid("empty indexed comment"))?;
                        if row.target_kind == 0 {
                            record.comment = Some(text);
                        } else {
                            record.move_comments.insert(row.move_raw, text);
                        }
                    }
                    Ok(record)
                }
            }
        })
    }
}
