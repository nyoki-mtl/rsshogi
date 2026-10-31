use super::*;

#[test]
fn matching_iterator_validates_the_stopper_row_in_all_storage_modes() {
    let entries: Vec<_> = (0..7)
        .map(|n| {
            let mut key = [0; 32];
            key[0] = n;
            YbbEntry::new(PackedSfen::from_bytes(key), 1, vec![]).unwrap()
        })
        .collect();
    for storage in [YbbStorageMode::Memory, YbbStorageMode::File] {
        for offset in [1, u64::MAX] {
            let mut bytes = encode(&entries, false);
            // lower_bound が参照しない、一致行直後の行だけを破損させる。
            let start = HEADER_BYTES + 2 * INDEX_BYTES + 32;
            bytes[start..start + 8].copy_from_slice(&offset.to_le_bytes());
            let book = open_bytes(
                bytes,
                YbbBookOpenOptions::new()
                    .with_storage_mode(storage)
                    .with_validation_mode(YbbValidationMode::OnAccess),
            );
            let mut rows = book.iter_entries_by_packed_sfen(entries[1].packed_sfen).unwrap();
            assert_eq!(rows.next().unwrap().unwrap(), entries[1]);
            assert!(matches!(rows.next(), Some(Err(BookError::InvalidFormat(_)))));
            assert!(rows.next().is_none());
            assert!(rows.next().is_none());
        }
    }
}

#[test]
fn diagnostics_and_matching_rows_preserve_groups_in_all_modes() {
    let position = hirate_position();
    let mut other = position.clone();
    other.apply_move(Move::from_usi("7g7f").unwrap());
    let mut entries = vec![
        YbbEntry::new(position.to_packed_sfen(), 5, vec![candidate("7g7f", 10, Some(0))]).unwrap(),
        YbbEntry::new(position.to_packed_sfen(), 1, vec![]).unwrap(),
        YbbEntry::new(position.to_packed_sfen(), 5, vec![candidate("2g2f", -20, Some(u16::MAX))])
            .unwrap(),
        YbbEntry::new(other.to_packed_sfen(), 0, vec![]).unwrap(),
        YbbEntry::new(other.to_packed_sfen(), u16::MAX, vec![]).unwrap(),
        YbbEntry::new(other.to_packed_sfen(), 0, vec![]).unwrap(),
    ];
    entries.sort_by(|a, b| a.packed_sfen.cmp_bytes(&b.packed_sfen));
    for storage in [YbbStorageMode::Memory, YbbStorageMode::File] {
        for validation in [YbbValidationMode::Full, YbbValidationMode::OnAccess] {
            let options = YbbBookOpenOptions::new()
                .with_storage_mode(storage)
                .with_validation_mode(validation)
                .with_ignore_ply(true)
                .with_flipped(true);
            let book = open_bytes(encode(&entries, true), options);
            let mut progress = Vec::new();
            let report = book
                .diagnose_full_with_control(|point| {
                    assert!(
                        book.lookup_packed_sfen(position.to_packed_sfen(), 99).unwrap().is_some()
                    );
                    progress.push((point.processed_row_count(), point.total_row_count()));
                    BookControl::Continue
                })
                .unwrap();
            assert_eq!(progress, (0..=6).map(|n| (n, 6)).collect::<Vec<_>>());
            assert_eq!(
                (
                    report.row_count(),
                    report.position_count(),
                    report.duplicate_position_count(),
                    report.duplicate_row_count(),
                    report.duplicate_ply_row_count(),
                    report.varying_ply_position_count()
                ),
                (6, 2, 2, 4, 2, 2)
            );
            for key in [position.to_packed_sfen(), other.to_packed_sfen()] {
                let rows = book
                    .iter_entries_by_packed_sfen(key)
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                assert_eq!(
                    rows,
                    entries.iter().filter(|e| e.packed_sfen == key).cloned().collect::<Vec<_>>()
                );
            }
            let missing = PackedSfen::from_bytes([0; 32]);
            assert!(book.iter_entries_by_packed_sfen(missing).unwrap().next().is_none());
            for stop in [0, 3, 6] {
                assert!(matches!(
                    book.diagnose_full_with_control(|point| {
                        if point.processed_row_count() == stop {
                            BookControl::Cancel
                        } else {
                            BookControl::Continue
                        }
                    }),
                    Err(BookError::Cancelled)
                ));
                assert_eq!(book.diagnose_full().unwrap(), report);
            }
            assert_eq!(book.options(), options);
            let empty = open_bytes(encode(&[], false), options);
            assert_eq!(empty.diagnose_full().unwrap(), YbbDiagnostics::default());
            assert!(matches!(
                empty.diagnose_full_with_control(|_| BookControl::Cancel),
                Err(BookError::Cancelled)
            ));
        }
    }
}

#[test]
fn diagnostic_bitmap_handles_the_entire_ply_range_and_resets_between_keys() {
    let key = hirate_position().to_packed_sfen();
    let entries: Vec<_> =
        (0..=u16::MAX).map(|ply| YbbEntry::new(key, ply, vec![]).unwrap()).collect();
    let book = open_bytes(
        encode(&entries, false),
        YbbBookOpenOptions::new().with_storage_mode(YbbStorageMode::File),
    );
    let report = book.diagnose_full().unwrap();
    assert_eq!(report.row_count(), 65536);
    assert_eq!(report.duplicate_row_count(), 65535);
    assert_eq!(report.duplicate_ply_row_count(), 0);
    assert_eq!(report.varying_ply_position_count(), 1);
}

#[test]
fn matching_iterator_fuses_on_decode_error_and_reports_search_errors() {
    let key = hirate_position().to_packed_sfen();
    let entry = YbbEntry::new(key, 1, vec![candidate("7g7f", 0, None)]).unwrap();
    for storage in [YbbStorageMode::Memory, YbbStorageMode::File] {
        let options = YbbBookOpenOptions::new()
            .with_storage_mode(storage)
            .with_validation_mode(YbbValidationMode::OnAccess);
        let mut bytes = encode(&[entry.clone(), entry.clone()], false);
        let start = HEADER_BYTES + 2 * INDEX_BYTES;
        bytes[start..start + 2].copy_from_slice(&0u16.to_le_bytes());
        let book = open_bytes(bytes, options);
        let mut rows = book.iter_entries_by_packed_sfen(key).unwrap();
        assert!(rows.next().unwrap().is_err());
        assert!(rows.next().is_none());
        assert!(rows.next().is_none());
        let mut bytes = encode(std::slice::from_ref(&entry), false);
        bytes[HEADER_BYTES + 32..HEADER_BYTES + 40].copy_from_slice(&u64::MAX.to_le_bytes());
        let book = open_bytes(bytes, options);
        assert!(book.iter_entries_by_packed_sfen(key).is_err());
    }
}

#[test]
fn matching_iterators_keep_independent_scan_positions_across_shared_lookups() {
    let key = hirate_position().to_packed_sfen();
    let entries: Vec<_> = (0..2000).map(|ply| YbbEntry::new(key, ply, vec![]).unwrap()).collect();
    let book = open_bytes(
        encode(&entries, false),
        YbbBookOpenOptions::new().with_storage_mode(YbbStorageMode::File),
    );
    let cloned = book.clone();
    let mut a = book.iter_entries_by_packed_sfen(key).unwrap();
    let mut b = cloned.iter_entries_by_packed_sfen(key).unwrap();
    for entry in entries {
        assert_eq!(a.next().unwrap().unwrap(), entry);
        assert!(book.lookup_packed_sfen(key, 0).unwrap().is_some());
        assert_eq!(b.next().unwrap().unwrap(), entry);
    }
    assert!(a.next().is_none());
    assert!(b.next().is_none());
}
