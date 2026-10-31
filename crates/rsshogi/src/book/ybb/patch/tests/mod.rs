use std::fs;
use std::io::{self, Cursor, SeekFrom};
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use crate::board::hirate_position;
use crate::book::{
    BookCandidate, BookDatabaseEntry, BookEntryMetadata, BookMoveMetadata, YbbBookOpenOptions,
    YbbMove, YbbStorageMode, YbbValidationMode,
};
use crate::types::Move;

fn path() -> std::path::PathBuf {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    std::env::temp_dir().join(format!("rsshogi-ybb-patch-{}-{nonce}", std::process::id()))
}

fn encode(entries: &[YbbEntry], depth: bool) -> Vec<u8> {
    let mut writer = YbbWriter::new(
        Cursor::new(Vec::new()),
        entries.len() as u64,
        YbbWriteOptions::new().with_depth(depth),
    )
    .unwrap();
    for entry in entries {
        writer.write_entry(entry).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn open(bytes: Vec<u8>, options: YbbBookOpenOptions) -> YbbBook {
    let path = path();
    fs::write(&path, bytes).unwrap();
    let book = YbbBook::open_with_options(&path, options).unwrap();
    fs::remove_file(path).unwrap();
    book
}

fn decode(bytes: Vec<u8>) -> Vec<YbbEntry> {
    open(bytes, YbbBookOpenOptions::new()).iter_entries().collect::<Result<_, _>>().unwrap()
}

fn positions() -> Vec<Position> {
    let mut position = hirate_position();
    let mut positions = vec![position.clone()];
    for usi in ["7g7f", "3c3d", "2g2f", "8c8d", "2f2e"] {
        position.apply_move(Move::from_usi(usi).unwrap());
        positions.push(position.clone());
    }
    positions.sort_by(|a, b| a.to_packed_sfen().cmp_bytes(&b.to_packed_sfen()));
    positions
}

fn upsert(
    position: &Position,
    ply: Option<u32>,
    candidates: Vec<BookCandidate>,
) -> BookDatabaseEntry {
    BookDatabaseEntry::new(
        BookPosition::from_position(position, ply),
        candidates,
        BookEntryMetadata::default(),
    )
}

fn candidate(score: Option<i32>, depth: Option<u32>) -> BookCandidate {
    BookCandidate::new(
        Move::from_usi("7g7f").unwrap(),
        None,
        score,
        depth,
        None,
        None,
        BookMoveMetadata::default(),
    )
}

#[derive(Default)]
struct Output {
    cursor: Cursor<Vec<u8>>,
    calls: usize,
    budget: Option<usize>,
    fail_seek: bool,
    fail_flush: bool,
    short_writes: bool,
    interrupt: bool,
}

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.calls += 1;
        if self.interrupt {
            self.interrupt = false;
            return Err(io::ErrorKind::Interrupted.into());
        }
        let limit =
            self.budget.unwrap_or(usize::MAX).min(if self.short_writes { 3 } else { usize::MAX });
        if limit == 0 {
            return Err(io::Error::other("injected write failure"));
        }
        let n = self.cursor.write(&bytes[..bytes.len().min(limit)])?;
        if let Some(budget) = &mut self.budget {
            *budget -= n;
        }
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.calls += 1;
        if self.fail_flush { Err(io::Error::other("injected flush failure")) } else { Ok(()) }
    }
}

impl Seek for Output {
    fn seek(&mut self, offset: SeekFrom) -> io::Result<u64> {
        self.calls += 1;
        if self.fail_seek {
            Err(io::Error::other("injected seek failure"))
        } else {
            self.cursor.seek(offset)
        }
    }
}

#[test]
fn merge_replaces_inserts_deletes_in_byte_order_in_all_reader_modes() {
    let positions = positions();
    let source: Vec<_> = [1, 3, 4]
        .map(|i| YbbEntry::new(positions[i].to_packed_sfen(), i as u16, vec![]).unwrap())
        .into();
    for depth in [false, true] {
        for storage in [YbbStorageMode::Memory, YbbStorageMode::File] {
            for validation in [YbbValidationMode::Full, YbbValidationMode::OnAccess] {
                let options = YbbBookOpenOptions::new()
                    .with_storage_mode(storage)
                    .with_validation_mode(validation)
                    .with_ignore_ply(true)
                    .with_flipped(true);
                let book = open(encode(&source, depth), options);
                let mut patch = BookDatabasePatch::new();
                patch.upsert_entry(upsert(&positions[0], Some(0), vec![])).unwrap();
                patch.upsert_entry(upsert(&positions[3], Some(65535), vec![])).unwrap();
                patch.remove_entry_by_sfen(&positions[1].to_sfen(Some(999))).unwrap();
                patch.remove_entry_by_sfen(&positions[5].to_sfen(None)).unwrap();
                let saved_patch = patch.clone();
                let mut output = Cursor::new(Vec::new());
                book.write_ybb_with_patch(&mut output, &patch).unwrap();
                assert_eq!(
                    decode(output.into_inner()),
                    vec![
                        YbbEntry::new(positions[0].to_packed_sfen(), 0, vec![]).unwrap(),
                        YbbEntry::new(positions[3].to_packed_sfen(), 65535, vec![]).unwrap(),
                        source[2].clone()
                    ]
                );
                assert_eq!(patch, saved_patch);
                assert_eq!(book.options(), options);
                assert_eq!(book.iter_entries().collect::<Result<Vec<_>, _>>().unwrap(), source);
            }
        }
    }
}

#[test]
fn move_order_integer_boundaries_and_source_information_are_preserved() {
    let position = hirate_position();
    for depth in [false, true] {
        let moves = vec![
            YbbMove::new(Move::from_usi("7g7f").unwrap(), i16::MIN, depth.then_some(0)).unwrap(),
            YbbMove::new(Move::from_usi("2g2f").unwrap(), i16::MAX, depth.then_some(u16::MAX))
                .unwrap(),
        ];
        let entry = YbbEntry::new(position.to_packed_sfen(), 65535, moves).unwrap();
        let original = encode(std::slice::from_ref(&entry), depth);
        let book = open(original.clone(), YbbBookOpenOptions::new());
        let mut output = Cursor::new(Vec::new());
        book.write_ybb_with_patch(&mut output, &BookDatabasePatch::new()).unwrap();
        assert_eq!(output.into_inner(), original);
        for reverse in [false, true] {
            let mut candidates: Vec<_> = entry
                .moves
                .iter()
                .map(|m| {
                    BookCandidate::new(
                        m.mv(),
                        None,
                        Some(i32::from(m.eval())),
                        m.depth().map(u32::from),
                        None,
                        None,
                        BookMoveMetadata::default(),
                    )
                })
                .collect();
            if reverse {
                candidates.reverse();
            }
            let mut expected = entry.clone();
            if reverse {
                expected.moves.reverse();
            }
            expected.ply = 0;
            let mut patch = BookDatabasePatch::new();
            patch.upsert_entry(upsert(&position, Some(0), candidates)).unwrap();
            let mut output = Cursor::new(Vec::new());
            book.write_ybb_with_patch(&mut output, &patch).unwrap();
            assert_eq!(decode(output.into_inner()), [expected]);
        }
    }
}

#[test]
fn invalid_patch_is_rejected_before_reading_corrupt_source() {
    let positions = positions();
    let source = YbbEntry::new(positions[0].to_packed_sfen(), 1, vec![]).unwrap();
    let mut bytes = encode(&[source], false);
    bytes[HEADER_BYTES + 32..HEADER_BYTES + 40].copy_from_slice(&u64::MAX.to_le_bytes());
    for storage in [YbbStorageMode::Memory, YbbStorageMode::File] {
        let book = open(
            bytes.clone(),
            YbbBookOpenOptions::new()
                .with_storage_mode(storage)
                .with_validation_mode(YbbValidationMode::OnAccess),
        );
        let mut patch = BookDatabasePatch::new();
        patch.upsert_entry(upsert(positions.last().unwrap(), None, vec![])).unwrap();
        let mut output = Output::default();
        let error = book.write_ybb_with_patch(&mut output, &patch).unwrap_err();
        assert!(
            matches!(error, BookError::InvalidData(ref message) if message == "missing YBB ply")
        );
        assert_eq!(output.calls, 0);
    }
}

#[test]
fn corrupt_offsets_take_precedence_over_duplicate_or_reversed_source_keys() {
    let positions = positions();
    for second in [&positions[0], &positions[1]] {
        let first = YbbEntry::new(positions[1].to_packed_sfen(), 1, vec![]).unwrap();
        let second = YbbEntry::new(second.to_packed_sfen(), 1, vec![]).unwrap();
        let mut bytes = encode(&[first.clone(), first], false);
        let start = HEADER_BYTES + INDEX_BYTES;
        bytes[start..start + 32].copy_from_slice(second.packed_sfen.as_bytes());
        bytes[start + 32..start + 40].copy_from_slice(&u64::MAX.to_le_bytes());
        for storage in [YbbStorageMode::Memory, YbbStorageMode::File] {
            let book = open(
                bytes.clone(),
                YbbBookOpenOptions::new()
                    .with_storage_mode(storage)
                    .with_validation_mode(YbbValidationMode::OnAccess),
            );
            let mut output = Output::default();
            assert!(matches!(
                book.write_ybb_with_patch(&mut output, &BookDatabasePatch::new()),
                Err(BookError::InvalidFormat(_))
            ));
            assert_eq!(output.calls, 0);
        }
    }
}

#[test]
fn invalid_patch_never_touches_output_and_can_be_retried() {
    let position = hirate_position();
    for depth in [false, true] {
        let book = open(encode(&[], depth), YbbBookOpenOptions::new());
        let source = YbbEntry::new(position.to_packed_sfen(), 1, vec![]).unwrap();
        let mut corrupt_bytes = encode(&[source], depth);
        corrupt_bytes[HEADER_BYTES + 32..HEADER_BYTES + 40]
            .copy_from_slice(&u64::MAX.to_le_bytes());
        let corrupt_books: Vec<_> = [YbbStorageMode::Memory, YbbStorageMode::File]
            .into_iter()
            .map(|storage| {
                open(
                    corrupt_bytes.clone(),
                    YbbBookOpenOptions::new()
                        .with_storage_mode(storage)
                        .with_validation_mode(YbbValidationMode::OnAccess),
                )
            })
            .collect();
        let valid_depth = depth.then_some(0);
        let mut cases = vec![
            upsert(&position, None, vec![]),
            upsert(&position, Some(65536), vec![]),
            upsert(&position, Some(1), vec![candidate(None, valid_depth)]),
            upsert(&position, Some(1), vec![candidate(Some(-32769), valid_depth)]),
            upsert(&position, Some(1), vec![candidate(Some(32768), valid_depth)]),
            upsert(
                &position,
                Some(1),
                vec![candidate(Some(0), if depth { None } else { Some(0) })],
            ),
            upsert(&position, Some(1), vec![candidate(Some(0), Some(65536))]),
        ];
        let invalid_move = BookCandidate::new(
            Move::from_usi("3c3d").unwrap(),
            None,
            Some(0),
            valid_depth,
            None,
            None,
            BookMoveMetadata::default(),
        );
        cases.push(upsert(&position, Some(1), vec![invalid_move]));
        for field in 0..4 {
            let c = BookCandidate::new(
                Move::from_usi("7g7f").unwrap(),
                (field == 0).then(|| Move::from_usi("3c3d").unwrap()),
                Some(0),
                valid_depth,
                (field == 1).then_some(0),
                (field == 2).then_some(0),
                if field == 3 {
                    BookMoveMetadata::from_yaneuraou(String::new())
                } else {
                    BookMoveMetadata::default()
                },
            );
            cases.push(upsert(&position, Some(1), vec![c]));
        }
        cases.push(BookDatabaseEntry::new(
            BookPosition::from_position(&position, Some(1)),
            vec![],
            BookEntryMetadata::from_yaneuraou(0, String::new()),
        ));
        for entry in cases {
            let mut patch = BookDatabasePatch::new();
            patch.upsert_entry(entry).unwrap();
            let saved = patch.clone();
            let mut output = Output::default();
            let error = book.write_ybb_with_patch(&mut output, &patch).unwrap_err();
            assert!(matches!(error, BookError::InvalidData(_)));
            assert!(!error.to_string().contains("allow_lossy"));
            assert_eq!(output.calls, 0);
            for corrupt_book in &corrupt_books {
                let mut output = Output::default();
                let corrupt_error =
                    corrupt_book.write_ybb_with_patch(&mut output, &patch).unwrap_err();
                assert!(matches!(corrupt_error, BookError::InvalidData(_)));
                assert_eq!(corrupt_error.to_string(), error.to_string());
                assert_eq!(output.calls, 0);
            }
            assert_eq!(patch, saved);
            assert!(book.write_ybb_with_patch(&mut Output::default(), &patch).is_err());
        }
        let mut patch = BookDatabasePatch::new();
        patch.remove_entry_by_sfen("4k4/9/9/9/9/9/9/9/9 b - 1").unwrap();
        let mut output = Output::default();
        assert!(book.write_ybb_with_patch(&mut output, &patch).is_err());
        assert_eq!(output.calls, 0);
    }
}

#[test]
fn source_duplicates_reverse_order_and_corruption_cannot_be_hidden_by_patch() {
    let positions = positions();
    let first = YbbEntry::new(
        positions[0].to_packed_sfen(),
        1,
        vec![YbbMove::new(Move::from_usi("7g7f").unwrap(), 0, None).unwrap()],
    )
    .unwrap();
    let other = YbbEntry::new(positions[1].to_packed_sfen(), 2, vec![]).unwrap();
    let mut duplicate = first.clone();
    duplicate.ply = 99;
    let mut reverse = encode(&[first.clone(), other.clone()], false);
    let (a, b) = reverse[HEADER_BYTES..HEADER_BYTES + 2 * INDEX_BYTES].split_at_mut(INDEX_BYTES);
    a.swap_with_slice(b);
    let mut bad_move = encode(std::slice::from_ref(&first), false);
    bad_move[HEADER_BYTES + INDEX_BYTES..HEADER_BYTES + INDEX_BYTES + 2].fill(0);
    let mut bad_offset = encode(std::slice::from_ref(&first), false);
    bad_offset[HEADER_BYTES + 32..HEADER_BYTES + 40].copy_from_slice(&u64::MAX.to_le_bytes());
    for (bytes, strict) in [
        (encode(&[first.clone(), first.clone()], false), true),
        (encode(&[first.clone(), duplicate], false), true),
        (reverse, true),
        (bad_move, false),
        (bad_offset, false),
    ] {
        for storage in [YbbStorageMode::Memory, YbbStorageMode::File] {
            let book = open(
                bytes.clone(),
                YbbBookOpenOptions::new()
                    .with_storage_mode(storage)
                    .with_validation_mode(YbbValidationMode::OnAccess),
            );
            for action in 0..3 {
                let mut patch = BookDatabasePatch::new();
                if action == 1 {
                    patch.remove_entry_by_sfen(&positions[0].to_sfen(None)).unwrap();
                }
                if action == 2 {
                    patch.upsert_entry(upsert(&positions[0], Some(1), vec![])).unwrap();
                }
                let mut output = Output::default();
                let error = book.write_ybb_with_patch(&mut output, &patch).unwrap_err();
                if strict {
                    assert!(matches!(error, BookError::InvalidData(_)));
                    assert!(error.to_string().contains("row 1"));
                }
                assert_eq!(output.calls, 0);
            }
        }
    }
}

#[test]
fn empty_input_full_deletion_and_zero_candidate_upsert_remain_distinct() {
    let position = hirate_position();
    let entry = YbbEntry::new(position.to_packed_sfen(), 1, vec![]).unwrap();
    for depth in [false, true] {
        let book = open(encode(std::slice::from_ref(&entry), depth), YbbBookOpenOptions::new());
        let mut patch = BookDatabasePatch::new();
        patch.remove_entry_by_sfen(&position.to_sfen(None)).unwrap();
        let mut output = Cursor::new(Vec::new());
        book.write_ybb_with_patch(&mut output, &patch).unwrap();
        assert!(decode(output.into_inner()).is_empty());
        let empty = open(encode(&[], depth), YbbBookOpenOptions::new());
        patch.upsert_entry(upsert(&position, Some(0), vec![])).unwrap();
        let mut output = Cursor::new(Vec::new());
        empty.write_ybb_with_patch(&mut output, &patch).unwrap();
        assert_eq!(
            decode(output.into_inner()),
            [YbbEntry::new(position.to_packed_sfen(), 0, vec![]).unwrap()]
        );
    }
}

#[test]
fn progress_cancellation_and_io_failures_preserve_patch_and_reader() {
    let positions = positions();
    let source = YbbEntry::new(positions[2].to_packed_sfen(), 1, vec![]).unwrap();
    let book = open(
        encode(std::slice::from_ref(&source), false),
        YbbBookOpenOptions::new().with_storage_mode(YbbStorageMode::File),
    );
    let mut patch = BookDatabasePatch::new();
    patch.upsert_entry(upsert(&positions[0], Some(0), vec![])).unwrap();
    patch.upsert_entry(upsert(&positions[4], Some(4), vec![])).unwrap();
    let saved = patch.clone();
    let mut reference = Cursor::new(Vec::new());
    let mut points = Vec::new();
    book.write_ybb_with_patch_with_control(&mut reference, &patch, |point| {
        assert_eq!(point.total_positions(), Some(1));
        assert!(book.lookup_packed_sfen(source.packed_sfen, 1).unwrap().is_some());
        points.push((point.phase(), point.processed_positions()));
        BookControl::Continue
    })
    .unwrap();
    assert!(points.contains(&(BookPatchWritePhase::Writing, 0)));
    assert_eq!(points.last(), Some(&(BookPatchWritePhase::Writing, 1)));
    let writing_start =
        points.iter().position(|point| point.0 == BookPatchWritePhase::Writing).unwrap();
    for stop in 0..points.len() {
        let mut calls = 0;
        let mut output = Output::default();
        assert!(matches!(
            book.write_ybb_with_patch_with_control(&mut output, &patch, |_| {
                let cancel = calls == stop;
                calls += 1;
                if cancel { BookControl::Cancel } else { BookControl::Continue }
            }),
            Err(BookError::Cancelled)
        ));
        if stop <= writing_start {
            assert_eq!(output.calls, 0);
        }
        let mut retry = Cursor::new(Vec::new());
        book.write_ybb_with_patch(&mut retry, &patch).unwrap();
        assert_eq!(retry.into_inner(), *reference.get_ref());
        assert_eq!(patch, saved);
    }
    for fault in 0..3 {
        let mut output = Output::default();
        match fault {
            0 => output.budget = Some(1),
            1 => output.fail_seek = true,
            _ => output.fail_flush = true,
        }
        assert!(matches!(book.write_ybb_with_patch(&mut output, &patch), Err(BookError::Io(_))));
        assert_eq!(patch, saved);
    }
    let mut output = Output { short_writes: true, interrupt: true, ..Output::default() };
    book.write_ybb_with_patch(&mut output, &patch).unwrap();
    assert_eq!(output.cursor.into_inner(), reference.into_inner());
}

#[test]
fn unmodified_rows_keep_opaque_keys_and_candidates_without_semantic_validation() {
    let key = PackedSfen::from_bytes([0; 32]);
    let entry = YbbEntry::new(
        key,
        0,
        vec![YbbMove::new(Move::from_usi("7g7f").unwrap(), -1, None).unwrap(); 2],
    )
    .unwrap();
    let bytes = encode(std::slice::from_ref(&entry), false);
    let book = open(bytes.clone(), YbbBookOpenOptions::new());
    let mut output = Cursor::new(Vec::new());
    book.write_ybb_with_patch(&mut output, &BookDatabasePatch::new()).unwrap();
    assert_eq!(output.into_inner(), bytes);
}

#[test]
fn cancellation_after_block_output_leaves_partial_bytes_and_allows_fresh_retry() {
    let entries: Vec<_> = (0u64..1600)
        .map(|index| {
            let mut bytes = [0; 32];
            bytes[..8].copy_from_slice(&index.to_be_bytes());
            YbbEntry::new(PackedSfen::from_bytes(bytes), 1, vec![]).unwrap()
        })
        .collect();
    let original = encode(&entries, false);
    let book =
        open(original.clone(), YbbBookOpenOptions::new().with_storage_mode(YbbStorageMode::File));
    let patch = BookDatabasePatch::new();
    let mut output = Output::default();
    assert!(matches!(
        book.write_ybb_with_patch_with_control(&mut output, &patch, |point| {
            if point.phase() == BookPatchWritePhase::Writing && point.processed_positions() == 1500
            {
                BookControl::Cancel
            } else {
                BookControl::Continue
            }
        }),
        Err(BookError::Cancelled)
    ));
    assert!(!output.cursor.get_ref().is_empty());
    let mut retry = Cursor::new(Vec::new());
    book.write_ybb_with_patch(&mut retry, &patch).unwrap();
    assert_eq!(retry.into_inner(), original);
}

#[test]
fn source_read_failure_is_detected_before_output() {
    let source_path = path();
    let entry = YbbEntry::new(hirate_position().to_packed_sfen(), 1, vec![]).unwrap();
    fs::write(&source_path, encode(&[entry], false)).unwrap();
    let book = YbbBook::open_with_options(
        &source_path,
        YbbBookOpenOptions::new()
            .with_storage_mode(YbbStorageMode::File)
            .with_validation_mode(YbbValidationMode::OnAccess),
    )
    .unwrap();
    // 元ファイル不変の前提違反を故意に作り、read 失敗の出力側への伝播を確認する。
    fs::OpenOptions::new().write(true).open(&source_path).unwrap().set_len(32).unwrap();
    let mut output = Output::default();
    assert!(matches!(
        book.write_ybb_with_patch(&mut output, &BookDatabasePatch::new()),
        Err(BookError::Io(_))
    ));
    assert_eq!(output.calls, 0);
    drop(book);
    fs::remove_file(source_path).unwrap();
}
