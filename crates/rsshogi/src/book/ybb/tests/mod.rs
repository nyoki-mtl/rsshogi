mod diagnostics;
mod writer_blocks;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::board::hirate_position;
use crate::book::{BookCandidate, BookPosition};
use std::io::{self, Cursor};

use super::*;

fn temp_path(suffix: &str) -> PathBuf {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock after epoch").as_nanos();
    std::env::temp_dir().join(format!("rsshogi-ybb-{}-{nonce}-{suffix}", std::process::id()))
}

fn write_single_entry(path: &Path, position: &Position) {
    let packed = position.try_to_packed_sfen().expect("packed position");
    let mv = Move::from_usi("7g7f").expect("move");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&1u64.to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes());
    bytes.extend_from_slice(packed.as_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes());
    bytes.extend_from_slice(&position.game_ply().to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&mv.raw().to_le_bytes());
    bytes.extend_from_slice(&(-25i16).to_le_bytes());
    fs::write(path, bytes).expect("write YBB fixture");
}

#[test]
fn test_ybb_lookup_position_rejects_unencodable_positions_and_keeps_valid_lookup() {
    let path = temp_path("invalid-lookup-position");
    let position = hirate_position();
    write_single_entry(&path, &position);
    let book = YbbBook::open_with_options(&path, YbbBookOpenOptions::new().with_flipped(true))
        .expect("open YBB fixture");

    assert!(matches!(book.lookup_position(&Position::empty()), Err(BookError::InvalidData(_))));
    let one_king = Position::from_sfen("4k4/9/9/9/9/9/9/9/9 b - 1").expect("one king");
    assert!(matches!(book.lookup_position(&one_king), Err(BookError::InvalidData(_))));

    let entry = book.lookup_position(&position).expect("valid lookup").expect("entry");
    assert_eq!(entry.packed_sfen(), position.to_packed_sfen());
    assert_eq!(entry.moves()[0].mv(), Move::from_usi("7g7f").unwrap());

    let _ = fs::remove_file(path);
}

fn candidate(usi: &str, eval: i16, depth: Option<u16>) -> YbbMove {
    YbbMove::new(Move::from_usi(usi).unwrap(), eval, depth).unwrap()
}

fn open_bytes(bytes: Vec<u8>, options: YbbBookOpenOptions) -> YbbBook {
    let path = temp_path("roundtrip");
    fs::write(&path, bytes).unwrap();
    let result = YbbBook::open_with_options(&path, options);
    fs::remove_file(path).unwrap();
    result.unwrap()
}

fn encode(entries: &[YbbEntry], has_depth: bool) -> Vec<u8> {
    let mut writer = YbbWriter::new(
        Cursor::new(Vec::new()),
        entries.len() as u64,
        YbbWriteOptions::new().with_depth(has_depth),
    )
    .unwrap();
    for entry in entries {
        writer.write_entry(entry).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn test_full_validation_checks_all_rows_in_both_storage_modes() {
    let mut position = hirate_position();
    let entry = YbbEntry::new(position.to_packed_sfen(), 1, vec![candidate("7g7f", 0, Some(9999))])
        .unwrap();
    position.apply_move(Move::from_usi("7g7f").unwrap());
    let other = YbbEntry::new(position.to_packed_sfen(), 2, vec![]).unwrap();
    let mut entries = [entry, other];
    entries.sort_by(|a, b| a.packed_sfen.cmp_bytes(&b.packed_sfen));
    let valid = encode(&entries, true);
    let mut unsorted = valid.clone();
    let (first, rest) = unsorted[HEADER_BYTES..].split_at_mut(INDEX_BYTES);
    first.swap_with_slice(&mut rest[..INDEX_BYTES]);
    let mut invalid_move = valid.clone();
    let move_area = HEADER_BYTES + 2 * INDEX_BYTES;
    invalid_move[move_area..move_area + 2].copy_from_slice(&0u16.to_le_bytes());
    let mut invalid_offset = valid.clone();
    invalid_offset[HEADER_BYTES + 32..HEADER_BYTES + 40].copy_from_slice(&u64::MAX.to_le_bytes());
    let mut truncated = valid;
    truncated.pop();
    for bytes in [unsorted, invalid_move, invalid_offset, truncated] {
        let path = temp_path("full-validation");
        fs::write(&path, bytes).unwrap();
        for storage_mode in [YbbStorageMode::Memory, YbbStorageMode::File] {
            let options = YbbBookOpenOptions::new().with_storage_mode(storage_mode);
            assert!(YbbBook::open_with_options(&path, options).is_err());
            let deferred = options.with_validation_mode(YbbValidationMode::OnAccess);
            let book = YbbBook::open_with_options(&path, deferred).unwrap();
            assert!(book.validate_full().is_err());
            assert!(book.diagnose_full().is_err());
            assert_eq!(book.options(), deferred);
        }
        fs::remove_file(path).unwrap();
    }
}

#[test]
fn test_validation_progress_cancellation_and_reentrant_lookup() {
    let key = hirate_position().to_packed_sfen();
    let entries = [
        YbbEntry::new(key, 1, vec![candidate("7g7f", 10, None)]).unwrap(),
        YbbEntry::new(key, 50, vec![]).unwrap(),
    ];
    assert_eq!(YbbBookOpenOptions::new(), YbbBookOpenOptions::default());
    for storage_mode in [YbbStorageMode::Memory, YbbStorageMode::File] {
        let options = YbbBookOpenOptions::new()
            .with_storage_mode(storage_mode)
            .with_validation_mode(YbbValidationMode::OnAccess);
        let book = open_bytes(encode(&entries, false), options);
        let mut progress = Vec::new();
        book.validate_full_with_control(|point| {
            // コールバック中に同じ reader を参照しても、ファイルロックを再入しない。
            assert_eq!(book.lookup_packed_sfen(key, 1).unwrap(), Some(entries[0].clone()));
            progress.push((point.processed_row_count(), point.total_row_count()));
            BookControl::Continue
        })
        .unwrap();
        assert_eq!(progress, [(0, 2), (1, 2), (2, 2)]);
        for stop_after in [0, 1] {
            let mut progress = Vec::new();
            let result = book.validate_full_with_control(|point| {
                progress.push(point.processed_row_count());
                if point.processed_row_count() == stop_after {
                    BookControl::Cancel
                } else {
                    BookControl::Continue
                }
            });
            assert!(matches!(result, Err(BookError::Cancelled)));
            assert_eq!(progress, (0..=stop_after).collect::<Vec<_>>());
            assert_eq!(book.options(), options);
            assert_eq!(book.lookup_packed_sfen(key, 50).unwrap(), Some(entries[1].clone()));
            book.clone().validate_full().unwrap();
        }

        let empty = open_bytes(encode(&[], false), options);
        let mut progress = Vec::new();
        empty
            .validate_full_with_control(|point| {
                progress.push((point.processed_row_count(), point.total_row_count()));
                BookControl::Continue
            })
            .unwrap();
        assert_eq!(progress, [(0, 0)]);
        assert!(matches!(
            empty.validate_full_with_control(|_| BookControl::Cancel),
            Err(BookError::Cancelled)
        ));
    }
}

#[test]
fn test_file_backed_lookup_and_iteration_match_memory_with_duplicate_keys() {
    let position = hirate_position();
    let key = position.to_packed_sfen();
    let flipped = Position::from_sfen(&position.to_sfen_flipped(None)).unwrap();
    for has_depth in [false, true] {
        let entries = vec![
            YbbEntry::new(key, 50, vec![candidate("7g7f", -25, has_depth.then_some(9999))])
                .unwrap(),
            YbbEntry::new(key, 1, vec![]).unwrap(),
            YbbEntry::new(key, 50, vec![candidate("2g2f", 30, has_depth.then_some(0))]).unwrap(),
        ];
        for ignore_ply in [false, true] {
            for flip in [false, true] {
                let options =
                    YbbBookOpenOptions::new().with_ignore_ply(ignore_ply).with_flipped(flip);
                let bytes = encode(&entries, has_depth);
                let memory = open_bytes(bytes.clone(), options);
                let file = open_bytes(
                    bytes,
                    options
                        .with_storage_mode(YbbStorageMode::File)
                        .with_validation_mode(YbbValidationMode::OnAccess),
                );
                assert_eq!(file.options().storage_mode(), YbbStorageMode::File);
                assert_eq!(file.len(), memory.len());
                assert_eq!(file.has_depth(), memory.has_depth());
                assert_eq!(file.iter_entries().collect::<Result<Vec<_>, _>>().unwrap(), entries);
                for ply in [1, 50, 99] {
                    assert_eq!(
                        file.lookup_packed_sfen(key, ply).unwrap(),
                        memory.lookup_packed_sfen(key, ply).unwrap()
                    );
                }
                assert_eq!(
                    file.lookup_position(&flipped).unwrap(),
                    memory.lookup_position(&flipped).unwrap()
                );
            }
        }
    }
}

#[test]
fn test_file_backed_binary_search_multiple_keys_and_shared_cursor() {
    let mut position = hirate_position();
    let mut entries = Vec::new();
    for usi in ["7g7f", "3c3d", "2g2f", "8c8d"] {
        entries.push(
            YbbEntry::new(
                position.to_packed_sfen(),
                position.game_ply(),
                vec![candidate(usi, -3, Some(5))],
            )
            .unwrap(),
        );
        position.apply_move(Move::from_usi(usi).unwrap());
    }
    let missing = position.to_packed_sfen();
    entries.sort_by(|a, b| a.packed_sfen.cmp_bytes(&b.packed_sfen));
    let book = open_bytes(
        encode(&entries, true),
        YbbBookOpenOptions::new()
            .with_storage_mode(YbbStorageMode::File)
            .with_validation_mode(YbbValidationMode::OnAccess),
    );
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let clone = book.clone();
            let expected = &entries;
            scope.spawn(move || {
                for _ in 0..20 {
                    assert!(clone.lookup_packed_sfen(missing, 5).unwrap().is_none());
                    for entry in expected {
                        assert_eq!(
                            clone.lookup_packed_sfen(entry.packed_sfen, entry.ply).unwrap(),
                            Some(entry.clone())
                        );
                    }
                    assert_eq!(
                        clone.iter_entries().collect::<Result<Vec<_>, _>>().unwrap(),
                        *expected
                    );
                }
            });
        }
    });
}

#[test]
fn test_buffered_scan_boundaries_noncontiguous_offsets_and_interleaved_lookup() {
    let key = hirate_position().to_packed_sfen();
    let mut entries: Vec<_> = (0..5000)
        .map(|ply| {
            YbbEntry::new(key, ply, vec![candidate("7g7f", ply as i16, Some(ply)); 4]).unwrap()
        })
        .collect();
    // 最大行は通常の buffer より大きく、次の小さい行でも buffer を再利用する。
    entries[2500] =
        YbbEntry::new(key, 2500, vec![candidate("2g2f", -10, Some(3)); usize::from(u16::MAX)])
            .unwrap();
    let mut bytes = encode(&entries, true);
    // 候補手 offset は索引順に連続する必要がなく、同じ領域の共有も受理する。
    let offset_a = HEADER_BYTES + INDEX_BYTES * 10 + 32;
    let offset_b = HEADER_BYTES + INDEX_BYTES * 4000 + 32;
    let offset = bytes[offset_b..offset_b + 8].to_vec();
    bytes[offset_a..offset_a + 8].copy_from_slice(&offset);
    let options = YbbBookOpenOptions::new().with_validation_mode(YbbValidationMode::OnAccess);
    let memory = open_bytes(bytes.clone(), options);
    let file = open_bytes(bytes, options.with_storage_mode(YbbStorageMode::File));
    let mut first = file.iter_entries();
    let mut second = file.iter_entries();
    for expected in memory.iter_entries() {
        let expected = expected.unwrap();
        assert_eq!(first.next().unwrap().unwrap(), expected);
        if expected.ply % 1000 == 0 {
            assert_eq!(file.lookup_packed_sfen(key, expected.ply).unwrap(), Some(expected.clone()));
        }
        assert_eq!(second.next().unwrap().unwrap(), expected);
    }
    assert!(first.next().is_none());
    assert!(second.next().is_none());
    file.validate_full().unwrap();
}

#[test]
fn test_file_backed_defers_bad_rows_and_moves_but_returns_errors() {
    let key = hirate_position().to_packed_sfen();
    let entry = YbbEntry::new(key, 1, vec![candidate("7g7f", 0, None)]).unwrap();
    let valid = encode(&[entry], false);
    let mut invalid_move = valid.clone();
    invalid_move[76..78].copy_from_slice(&0u16.to_le_bytes());
    let mut invalid_offset = valid.clone();
    invalid_offset[64..72].copy_from_slice(&u64::MAX.to_le_bytes());
    let mut truncated_moves = valid;
    truncated_moves.pop();
    for bytes in [invalid_move, invalid_offset, truncated_moves] {
        let path = temp_path("deferred-validation");
        fs::write(&path, &bytes).unwrap();
        assert!(YbbBook::open(&path).is_err());
        let book = YbbBook::open_with_options(
            &path,
            YbbBookOpenOptions::new()
                .with_storage_mode(YbbStorageMode::File)
                .with_validation_mode(YbbValidationMode::OnAccess),
        )
        .unwrap();
        assert!(book.lookup_packed_sfen(key, 1).is_err());
        let mut iter = book.iter_entries();
        assert!(iter.next().unwrap().is_err());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        drop(book);
        fs::remove_file(path).unwrap();
    }
}

#[test]
fn test_file_backed_checks_header_index_extent_and_empty_iteration() {
    for bytes in [
        vec![],
        MAGIC.to_vec(),
        {
            let mut bytes = encode(&[], false);
            bytes[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
            bytes
        },
        {
            let mut bytes = encode(&[], false);
            bytes[16..24].copy_from_slice(&1u64.to_le_bytes());
            bytes
        },
        {
            let mut bytes = encode(&[], false);
            bytes[24..32].copy_from_slice(&2u64.to_le_bytes());
            bytes
        },
    ] {
        let path = temp_path("bad-header");
        fs::write(&path, bytes).unwrap();
        assert!(
            YbbBook::open_with_options(
                &path,
                YbbBookOpenOptions::new()
                    .with_storage_mode(YbbStorageMode::File)
                    .with_validation_mode(YbbValidationMode::OnAccess)
            )
            .is_err()
        );
        fs::remove_file(path).unwrap();
    }
    let empty = open_bytes(
        encode(&[], true),
        YbbBookOpenOptions::new()
            .with_storage_mode(YbbStorageMode::File)
            .with_validation_mode(YbbValidationMode::OnAccess),
    );
    assert!(empty.is_empty());
    let mut iter = empty.iter_entries();
    assert_eq!(iter.len(), 0);
    assert!(iter.next().is_none());
    assert!(empty.lookup_position(&hirate_position()).unwrap().is_none());
}

#[test]
fn test_file_backed_iteration_detects_unsorted_keys_and_truncated_file() {
    let mut position = hirate_position();
    let key = position.to_packed_sfen();
    position.apply_move(Move::from_usi("7g7f").unwrap());
    let mut entries = vec![
        YbbEntry::new(key, 1, vec![]).unwrap(),
        YbbEntry::new(position.to_packed_sfen(), 2, vec![]).unwrap(),
    ];
    entries.sort_by(|a, b| a.packed_sfen.cmp_bytes(&b.packed_sfen));
    let mut bytes = encode(&entries, false);
    let (first, second) = bytes[HEADER_BYTES..].split_at_mut(INDEX_BYTES);
    first.swap_with_slice(second);
    let book = open_bytes(
        bytes,
        YbbBookOpenOptions::new()
            .with_storage_mode(YbbStorageMode::File)
            .with_validation_mode(YbbValidationMode::OnAccess),
    );
    let mut iter = book.iter_entries();
    assert!(iter.next().unwrap().is_ok());
    assert!(iter.next().unwrap().is_err());

    let path = temp_path("truncated-after-open");
    fs::write(&path, encode(&entries, false)).unwrap();
    let book = YbbBook::open_with_options(
        &path,
        YbbBookOpenOptions::new()
            .with_storage_mode(YbbStorageMode::File)
            .with_validation_mode(YbbValidationMode::OnAccess),
    )
    .unwrap();
    fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(HEADER_BYTES as u64).unwrap();
    assert!(matches!(book.lookup_packed_sfen(key, 1), Err(BookError::Io(_))));
    assert!(matches!(book.validate_full(), Err(BookError::Io(_))));
    assert!(matches!(
        book.validate_full_with_control(|_| BookControl::Cancel),
        Err(BookError::Cancelled)
    ));
    drop(book);
    fs::remove_file(path).unwrap();
}

#[test]
fn test_iterator_preserves_rows_moves_and_ignores_lookup_options() {
    let key = hirate_position().to_packed_sfen();
    let entries = vec![
        YbbEntry::new(
            key,
            50,
            vec![candidate("7g7f", -25, Some(0)), candidate("2g2f", 30, Some(65535))],
        )
        .unwrap(),
        YbbEntry::new(key, 1, vec![]).unwrap(),
        YbbEntry::new(key, 50, vec![candidate("7g7f", 4, Some(9)), candidate("7g7f", 5, Some(8))])
            .unwrap(),
    ];
    let book = open_bytes(
        encode(&entries, true),
        YbbBookOpenOptions::new().with_ignore_ply(true).with_flipped(true),
    );
    let mut iter = book.iter_entries();
    assert_eq!(iter.len(), 3);
    assert_eq!(iter.next().unwrap().unwrap(), entries[0]);
    assert_eq!(iter.size_hint(), (2, Some(2)));
    assert_eq!(iter.collect::<Result<Vec<_>, _>>().unwrap(), entries[1..]);
    assert_eq!(book.lookup_packed_sfen(key, 1).unwrap().unwrap().ply(), 50);
    let mut iter = book.iter_entries();
    for _ in 0..3 {
        iter.next().unwrap().unwrap();
    }
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert_eq!(iter.len(), 0);
}

#[test]
fn test_writer_matches_independent_binary_layout_with_and_without_depth() {
    let position = hirate_position();
    for has_depth in [false, true] {
        let entry = YbbEntry::new(
            position.to_packed_sfen(),
            65535,
            vec![
                candidate("7g7f", i16::MIN, has_depth.then_some(65535)),
                candidate("2g2f", i16::MAX, has_depth.then_some(0)),
            ],
        )
        .unwrap();
        let bytes = encode(std::slice::from_ref(&entry), has_depth);
        let mut expected = b"YANE-BINBOOK-V1\0".to_vec();
        expected.extend_from_slice(&1u64.to_le_bytes());
        expected.extend_from_slice(&u64::from(has_depth).to_le_bytes());
        expected.extend_from_slice(position.to_packed_sfen().as_bytes());
        expected.extend_from_slice(&0u64.to_le_bytes());
        expected.extend_from_slice(&65535u16.to_le_bytes());
        expected.extend_from_slice(&2u16.to_le_bytes());
        for (usi, eval, depth) in [("7g7f", i16::MIN, 65535u16), ("2g2f", i16::MAX, 0)] {
            expected.extend_from_slice(&Move::from_usi(usi).unwrap().raw().to_le_bytes());
            expected.extend_from_slice(&eval.to_le_bytes());
            if has_depth {
                expected.extend_from_slice(&depth.to_le_bytes());
            }
        }
        assert_eq!(bytes, expected);
        assert_eq!(bytes.len(), 76 + if has_depth { 12 } else { 8 });
        let book = open_bytes(bytes, YbbBookOpenOptions::new());
        assert_eq!(book.iter_entries().next().unwrap().unwrap(), entry);
    }
}

#[test]
fn test_writer_empty_books_zero_move_rows_and_maximum_move_count() {
    for has_depth in [false, true] {
        let bytes = encode(&[], has_depth);
        assert_eq!(bytes.len(), 32);
        let book = open_bytes(bytes, YbbBookOpenOptions::new());
        assert!(book.is_empty());
        assert_eq!(book.has_depth(), has_depth);
    }
    let key = hirate_position().to_packed_sfen();
    let empty = YbbEntry::new(key, 0, vec![]).unwrap();
    let bytes = encode(std::slice::from_ref(&empty), false);
    assert_eq!(bytes.len(), 76);
    assert_eq!(
        open_bytes(bytes, YbbBookOpenOptions::new()).iter_entries().next().unwrap().unwrap(),
        empty
    );
    let mv = candidate("7g7f", 0, None);
    let maximum = YbbEntry::new(key, 1, vec![mv; 65535]).unwrap();
    let bytes = encode(&[maximum], false);
    assert_eq!(&bytes[74..76], &65535u16.to_le_bytes());
    assert!(YbbEntry::new(key, 1, vec![mv; 65536]).is_err());
    assert!(YbbMove::new(Move::MOVE_NONE, 0, None).is_err());
    assert!(YbbMove::new(Move::from_raw(0xffff), 0, None).is_err());
}

#[test]
fn test_writer_offsets_and_rejects_unsorted_or_flipped_rows() {
    let position = hirate_position();
    let flipped = Position::from_sfen(&position.to_sfen_flipped(Some(1))).unwrap();
    let mut entries = vec![
        YbbEntry::new(position.to_packed_sfen(), 1, vec![candidate("7g7f", 1, None)]).unwrap(),
        YbbEntry::new(
            flipped.to_packed_sfen(),
            2,
            vec![candidate("3c3d", 2, None), candidate("8c8d", 3, None)],
        )
        .unwrap(),
    ];
    entries.sort_by(|left, right| left.packed_sfen.cmp_bytes(&right.packed_sfen));
    assert_ne!(entries[0].packed_sfen, entries[1].packed_sfen);
    let bytes = encode(&entries, false);
    assert_eq!(
        u64::from_le_bytes(bytes[108..116].try_into().unwrap()),
        entries[0].moves.len() as u64 * 4
    );
    assert_eq!(
        open_bytes(bytes, YbbBookOpenOptions::new())
            .iter_entries()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        entries
    );
    let mut writer = YbbWriter::new(Cursor::new(Vec::new()), 2, YbbWriteOptions::new()).unwrap();
    writer.write_entry(&entries[1]).unwrap();
    assert!(writer.write_entry(&entries[0]).is_err());
    assert!(writer.finish().is_err());
    let mut entry = entries[0].clone();
    entry.flipped = true;
    let mut writer = YbbWriter::new(Cursor::new(Vec::new()), 1, YbbWriteOptions::new()).unwrap();
    assert!(writer.write_entry(&entry).is_err());
}

#[test]
fn test_new_entry_uses_move_coordinates_from_flipped_candidates() {
    let position = hirate_position();
    let entry =
        YbbEntry::new(position.to_packed_sfen(), 1, vec![candidate("7g7f", 42, None)]).unwrap();
    let book = open_bytes(encode(&[entry], false), YbbBookOpenOptions::new().with_flipped(true));
    let flipped = Position::from_sfen(&position.to_sfen_flipped(Some(1))).unwrap();
    let result = book.lookup_position(&flipped).unwrap().unwrap();
    assert!(result.flipped());
    assert_ne!(result.moves()[0].raw_move(), result.moves()[0].mv().raw());
    let mut writer = YbbWriter::new(Cursor::new(Vec::new()), 1, YbbWriteOptions::new()).unwrap();
    assert!(writer.write_entry(&result).is_err());
    let new_entry = YbbEntry::new(flipped.to_packed_sfen(), 1, result.moves().to_vec()).unwrap();
    assert_eq!(new_entry.moves()[0].raw_move(), result.moves()[0].mv().raw());
    let copied = open_bytes(encode(&[new_entry], false), YbbBookOpenOptions::new());
    assert_eq!(
        copied.lookup_position(&flipped).unwrap().unwrap().moves()[0].mv(),
        result.moves()[0].mv()
    );
}

#[test]
fn test_writer_rejects_count_depth_output_and_size_mismatches() {
    let key = hirate_position().to_packed_sfen();
    let entry = YbbEntry::new(key, 1, vec![candidate("7g7f", 0, None)]).unwrap();
    let writer = YbbWriter::new(Cursor::new(Vec::new()), 1, YbbWriteOptions::new()).unwrap();
    assert!(writer.finish().is_err());
    let mut writer = YbbWriter::new(Cursor::new(Vec::new()), 0, YbbWriteOptions::new()).unwrap();
    assert!(writer.write_entry(&entry).is_err());
    assert!(writer.finish().is_err());
    for has_depth in [false, true] {
        let entry =
            YbbEntry::new(key, 1, vec![candidate("7g7f", 0, (!has_depth).then_some(0))]).unwrap();
        let mut writer = YbbWriter::new(
            Cursor::new(Vec::new()),
            1,
            YbbWriteOptions::new().with_depth(has_depth).with_allow_lossy(true),
        )
        .unwrap();
        assert!(writer.write_entry(&entry).is_err());
        assert!(writer.finish().is_err());
    }
    assert!(YbbWriter::new(Cursor::new(vec![1]), 0, YbbWriteOptions::new()).is_err());
    assert!(YbbWriter::new(Cursor::new(Vec::new()), u64::MAX, YbbWriteOptions::new()).is_err());
    let mut writer = YbbWriter::new(Cursor::new(Vec::new()), 1, YbbWriteOptions::new()).unwrap();
    writer.move_bytes = u64::MAX;
    assert!(writer.write_entry(&entry).is_err());
}

#[derive(Default)]
struct FaultOutput {
    cursor: Cursor<Vec<u8>>,
    write_budget: Option<usize>,
    fail_seek: bool,
    fail_flush: bool,
}

impl Write for FaultOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len().min(self.write_budget.unwrap_or(usize::MAX));
        if count == 0 {
            return Err(io::Error::other("injected write failure"));
        }
        let written = self.cursor.write(&bytes[..count])?;
        if let Some(budget) = &mut self.write_budget {
            *budget -= written;
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush {
            return Err(io::Error::other("injected flush failure"));
        }
        Ok(())
    }
}

impl Seek for FaultOutput {
    fn seek(&mut self, offset: SeekFrom) -> io::Result<u64> {
        if self.fail_seek {
            return Err(io::Error::other("injected seek failure"));
        }
        self.cursor.seek(offset)
    }
}

#[test]
fn test_writer_io_failures_are_propagated_and_poison_the_writer() {
    let entry =
        YbbEntry::new(hirate_position().to_packed_sfen(), 1, vec![candidate("7g7f", 0, None)])
            .unwrap();
    let mut writer = YbbWriter::new(FaultOutput::default(), 1, YbbWriteOptions::new()).unwrap();
    writer.output.write_budget = Some(1);
    let large =
        YbbEntry::new(entry.packed_sfen, 1, vec![candidate("7g7f", 0, None); u16::MAX as usize])
            .unwrap();
    assert!(matches!(writer.write_entry(&large), Err(BookError::Io(_))));
    writer.output.write_budget = None;
    assert!(writer.write_entry(&entry).is_err());
    assert!(writer.finish().is_err());
    for failure in 0..3 {
        let mut writer = YbbWriter::new(FaultOutput::default(), 1, YbbWriteOptions::new()).unwrap();
        writer.write_entry(&entry).unwrap();
        match failure {
            0 => writer.output.write_budget = Some(1),
            1 => writer.output.fail_seek = true,
            _ => writer.output.fail_flush = true,
        }
        assert!(matches!(writer.finish(), Err(BookError::Io(_))));
    }
}

fn database_entry(
    position: &Position,
    ply: Option<u32>,
    score: Option<i32>,
    depth: Option<u32>,
) -> BookDatabaseEntry {
    BookDatabaseEntry::new(
        BookPosition::from_position(position, ply),
        vec![BookCandidate::new(
            Move::from_usi("7g7f").unwrap(),
            None,
            score,
            depth,
            None,
            None,
            BookMoveMetadata::default(),
        )],
        BookEntryMetadata::default(),
    )
}

fn export_entry(entry: BookDatabaseEntry, options: YbbWriteOptions) -> Result<Vec<u8>, BookError> {
    let database = BookDatabase::try_from_entries(vec![entry])?;
    let mut output = Cursor::new(Vec::new());
    database.write_ybb(&mut output, &options)?;
    Ok(output.into_inner())
}

#[test]
fn test_database_export_sorting_move_order_and_integer_boundaries() {
    let position = hirate_position();
    let mut other = position.clone();
    let mv = other.move32_from_move(Move::from_usi("2g2f").unwrap());
    other.apply_move32(mv);
    let first = database_entry(&position, Some(0), Some(i16::MIN.into()), Some(65535));
    let mut candidates = first.candidates().to_vec();
    candidates.push(BookCandidate::new(
        Move::from_usi("2g2f").unwrap(),
        None,
        Some(i16::MAX.into()),
        Some(0),
        None,
        None,
        BookMoveMetadata::default(),
    ));
    let first =
        BookDatabaseEntry::new(first.position().clone(), candidates, first.metadata().clone());
    let second = BookDatabaseEntry::new(
        BookPosition::from_position(&other, Some(65535)),
        vec![],
        BookEntryMetadata::default(),
    );
    let mut entries = vec![first, second];
    entries.sort_by(|left, right| {
        right.position().packed_sfen().unwrap().cmp_bytes(&left.position().packed_sfen().unwrap())
    });
    let database = BookDatabase::try_from_entries(entries).unwrap();
    let mut output = Cursor::new(Vec::new());
    database.write_ybb(&mut output, &YbbWriteOptions::new().with_depth(true)).unwrap();
    let book = open_bytes(output.into_inner(), YbbBookOpenOptions::new());
    let rows = book.iter_entries().collect::<Result<Vec<_>, _>>().unwrap();
    assert!(rows[0].packed_sfen.cmp_bytes(&rows[1].packed_sfen).is_lt());
    let entry = book.lookup_packed_sfen(position.to_packed_sfen(), 0).unwrap().unwrap();
    assert_eq!(
        entry.moves,
        vec![candidate("7g7f", i16::MIN, Some(65535)), candidate("2g2f", i16::MAX, Some(0))]
    );
    assert!(book.lookup_packed_sfen(other.to_packed_sfen(), 65535).unwrap().is_some());
    let mut empty = Cursor::new(Vec::new());
    BookDatabase::new().write_ybb(&mut empty, &YbbWriteOptions::new()).unwrap();
    assert_eq!(empty.into_inner().len(), 32);
}

#[test]
fn test_database_rejects_missing_values_and_overflow_even_when_lossy() {
    let position = hirate_position();
    for (ply, score, depth) in [
        (None, Some(0), Some(0)),
        (Some(65536), Some(0), Some(0)),
        (Some(1), None, Some(0)),
        (Some(1), Some(32768), Some(0)),
        (Some(1), Some(-32769), Some(0)),
        (Some(1), Some(0), None),
        (Some(1), Some(0), Some(65536)),
    ] {
        for allow_lossy in [false, true] {
            assert!(
                export_entry(
                    database_entry(&position, ply, score, depth),
                    YbbWriteOptions::new().with_depth(true).with_allow_lossy(allow_lossy)
                )
                .is_err()
            );
        }
    }
}

#[test]
fn test_database_requires_explicit_metadata_and_depth_loss() {
    let position = hirate_position();
    let entry = database_entry(&position, Some(1), Some(0), None);
    for field in 0..6 {
        let old = &entry.candidates()[0];
        let candidate = BookCandidate::new(
            old.mv(),
            (field == 0).then_some(Move::from_usi("3c3d").unwrap()),
            old.score(),
            (field == 1).then_some(0),
            (field == 2).then_some(0),
            (field == 3).then_some(0),
            if field == 4 {
                BookMoveMetadata::from_yaneuraou(String::new())
            } else {
                BookMoveMetadata::default()
            },
        );
        let metadata = if field == 5 {
            BookEntryMetadata::from_yaneuraou(1, String::new())
        } else {
            BookEntryMetadata::default()
        };
        let entry = BookDatabaseEntry::new(entry.position().clone(), vec![candidate], metadata);
        assert!(export_entry(entry.clone(), YbbWriteOptions::new()).is_err(), "field {field}");
        let bytes = export_entry(entry, YbbWriteOptions::new().with_allow_lossy(true)).unwrap();
        let book = open_bytes(bytes, YbbBookOpenOptions::new());
        assert_eq!(book.iter_entries().next().unwrap().unwrap().moves[0].depth, None);
    }
}

#[test]
fn test_database_preflight_rejects_invalid_input_before_output() {
    let position = hirate_position();
    let valid = database_entry(&position, Some(1), Some(0), None);
    let mut other = position.clone();
    other.apply_move32(other.move32_from_move(Move::from_usi("2g2f").unwrap()));
    let invalid = database_entry(&other, Some(2), Some(0), None);
    let database = BookDatabase::try_from_entries(vec![valid.clone(), invalid]).unwrap();
    let mut output = Cursor::new(Vec::new());
    assert!(database.write_ybb(&mut output, &YbbWriteOptions::new()).is_err());
    assert!(output.get_ref().is_empty());
    assert_eq!(output.position(), 0);
    let unencodable = database_entry(&Position::empty(), Some(1), Some(0), None);
    assert!(export_entry(unencodable, YbbWriteOptions::new().with_allow_lossy(true)).is_err());
    let invalid = BookDatabaseEntry::new(
        valid.position().clone(),
        vec![BookCandidate::new(
            Move::MOVE_NONE,
            None,
            Some(0),
            None,
            None,
            None,
            BookMoveMetadata::default(),
        )],
        BookEntryMetadata::default(),
    );
    assert!(export_entry(invalid, YbbWriteOptions::new()).is_err());
    let duplicate = BookDatabaseEntry::new(
        valid.position().clone(),
        vec![valid.candidates()[0].clone(); 2],
        BookEntryMetadata::default(),
    );
    assert!(export_entry(duplicate, YbbWriteOptions::new()).is_err());
}
