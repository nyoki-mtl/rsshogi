use super::*;

fn visit_entries(book: &NativeBook) -> Result<BTreeMap<[u8; 32], NativeBookEntry>, BookError> {
    let mut entries = BTreeMap::new();
    book.validate_full_and_visit(
        |entry, position| {
            assert_eq!(position.to_sfen(None), entry.position()?.to_sfen(None));
            assert_eq!(position.game_ply(), 1);
            // コールバックから同じ reader を呼んでもキャッシュのロックを競合しない。
            assert_eq!(book.get_key(entry.key)?, Some(entry.clone()));
            assert!(entries.insert(entry.key, entry).is_none());
            Ok(())
        },
        |_| BookControl::Continue,
    )?;
    Ok(entries)
}

#[test]
fn visit_matches_current_snapshot_after_add_delete_overwrite_and_readd() {
    let directory = Directory::new();
    let path = directory.path("visit.book");
    let mut positions = vec![position()];
    for usi in ["7g7f", "3c3d", "2g2f", "8c8d"] {
        let mut next = positions.last().unwrap().clone();
        let mv = Move::from_usi(usi).unwrap();
        next.apply_move32(next.move32_from_move(mv));
        positions.push(next);
    }
    let records: Vec<_> = positions
        .iter()
        .map(|pos| NativeBookPosition::new(NativeBookEntry::from_position(pos, vec![]).unwrap()))
        .collect();
    let mut builder = NativeBookBuilder::new(&path, NativeBookMetadata::default())
        .unwrap()
        .with_block_positions(1)
        .unwrap();
    for record in &records[..3] {
        builder.push(record.clone()).unwrap();
    }
    builder.finish().unwrap();
    let original = NativeBook::open(&path).unwrap();
    assert_eq!(visit_entries(&original).unwrap().len(), 3);
    let mut editor = NativeBookEditor::open(&path).unwrap();
    editor.remove_position(&positions[0]).unwrap();
    editor.set_position_comment(&positions[1], Some("first".into())).unwrap();
    editor.upsert_position(records[3].clone()).unwrap();
    editor.save().unwrap();
    editor.set_position_comment(&positions[1], Some("second".into())).unwrap();
    editor.remove_position(&positions[3]).unwrap();
    editor.upsert_position(records[4].clone()).unwrap();
    editor.save().unwrap();
    editor.upsert_position(records[3].clone()).unwrap();
    editor
        .set_metadata(NativeBookMetadata { title: Some("new".into()), ..Default::default() })
        .unwrap();
    editor.save().unwrap();
    let book = NativeBook::open(&path).unwrap();
    let expected: BTreeMap<_, _> = book
        .entries()
        .map(|entry| entry.map(|entry| (entry.key, entry)))
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(visit_entries(&book).unwrap(), expected);
    assert_eq!(expected.len(), 4);
    assert_eq!(visit_entries(&original).unwrap().len(), 3);

    // 有効なログ内のコールバックエラーを NativeLog に置き換えない。
    let error = book
        .validate_full_and_visit(
            |entry, _| {
                if entry.key == records[1].entry.key {
                    Err(BookError::InvalidData("consumer error".into()))
                } else {
                    Ok(())
                }
            },
            |_| BookControl::Continue,
        )
        .unwrap_err();
    assert!(matches!(error, BookError::InvalidData(reason) if reason == "consumer error"));
}

#[test]
fn visit_empty_and_cancel_before_and_after_delivery() {
    let directory = Directory::new();
    let path = directory.path("empty.book");
    NativeBookBuilder::new(&path, NativeBookMetadata::default()).unwrap().finish().unwrap();
    let book = NativeBook::open(&path).unwrap();
    assert!(visit_entries(&book).unwrap().is_empty());
    let path = directory.path("one.book");
    create(&path);
    let book = NativeBook::open(&path).unwrap();
    for phase in ["checksum", "positions", "complete"] {
        let mut delivered = 0;
        let result = book.validate_full_and_visit(
            |_, _| {
                delivered += 1;
                Ok(())
            },
            |progress| {
                if progress.phase == phase { BookControl::Cancel } else { BookControl::Continue }
            },
        );
        assert!(matches!(result, Err(BookError::Cancelled)));
        assert_eq!(delivered, usize::from(phase != "checksum"));
    }
}

#[test]
fn visit_rejects_hidden_base_semantics_and_comments_with_valid_checksums() {
    use super::super::{block, crc::crc32c, wire::Header};
    for corrupt_move in [false, true] {
        for deleted in [false, true] {
            let directory = Directory::new();
            let path = directory.path("hidden.book");
            create(&path);
            let base = fs::read(&path).unwrap();
            let original = Header::decode(&base[..128]).unwrap();
            let mut editor = NativeBookEditor::open(&path).unwrap();
            if deleted {
                editor.remove_position(&position()).unwrap();
            } else {
                editor.set_position_comment(&position(), Some("replacement".into())).unwrap();
            }
            editor.save().unwrap();
            drop(editor);
            let edited = fs::read(&path).unwrap();
            let mut suffix = base[original.comment_index_offset as usize..].to_vec();
            let mut entry = record().entry;
            if corrupt_move {
                entry.candidates[0].mv = Move::from_usi("5a5b").unwrap();
                entry.candidates.sort_unstable_by_key(|candidate| candidate.mv.raw());
            } else {
                suffix[(original.comment_text_offset - original.comment_index_offset) as usize] =
                    b'\r';
            }
            let (row, compressed) = block::encode(&[entry], 0).unwrap();
            let mut header = Header::new(
                original.node_count,
                original.move_count,
                original.comment_count,
                original.comment_text_bytes,
                original.metadata_bytes,
                compressed.len() as u64,
                original.block_positions,
            )
            .unwrap();
            header.depth_kind = original.depth_kind;
            let mut bytes = vec![0; 128];
            bytes.extend_from_slice(&row.encode());
            bytes.extend_from_slice(&compressed);
            bytes.extend_from_slice(&suffix);
            header.base_crc = crc32c(&bytes[128..]);
            bytes[..128].copy_from_slice(&header.encode());
            bytes.extend_from_slice(&edited[original.base_end as usize..]);
            fs::write(&path, bytes).unwrap();
            let book = NativeBook::open(&path).unwrap();
            // 現在 snapshot の走査だけでは、隠れた base の不正を検出できない。
            assert!(book.entries().collect::<Result<Vec<_>, _>>().is_ok());
            assert!(book.validate_full().is_err());
            assert!(visit_entries(&book).is_err());
        }
    }
}

#[test]
fn visit_does_not_commit_results_before_incomplete_tail_is_rejected() {
    let directory = Directory::new();
    let path = directory.path("tail.book");
    create(&path);
    let base_end = fs::metadata(&path).unwrap().len() as usize;
    let mut editor = NativeBookEditor::open(&path).unwrap();
    editor.set_position_comment(&position(), Some("new".into())).unwrap();
    editor.save().unwrap();
    drop(editor);
    let bytes = fs::read(&path).unwrap();
    fs::write(&path, &bytes[..base_end + 40]).unwrap();
    let book = NativeBook::open(&path).unwrap();
    let mut delivered = 0;
    let error = book
        .validate_full_and_visit(
            |_, _| {
                delivered += 1;
                Ok(())
            },
            |_| BookControl::Continue,
        )
        .unwrap_err();
    assert_eq!(delivered, 1);
    assert!(matches!(error, BookError::NativeLog { reason: "incomplete transaction tail", .. }));
    assert!(book.validate_full().is_err());
}

#[test]
fn profiling_and_validated_writer_preserve_saved_bytes() {
    let directory = Directory::new();
    let plain = directory.path("plain.book");
    create(&plain);
    let profiled = directory.path("profiled.book");
    super::super::profile::start();
    create(&profiled);
    let phases = super::super::profile::finish();
    assert_eq!(phases["snapshot_decode_validate"].units, 1);
    assert!(!phases.contains_key("writer_normalize"));
    let direct = directory.path("direct.book");
    let book = NativeBook::open(&plain).unwrap();
    super::super::builder::write_sorted(&direct, book.metadata().unwrap(), [Ok(record())], false)
        .unwrap();
    assert_eq!(fs::read(&plain).unwrap(), fs::read(&profiled).unwrap());
    assert_eq!(fs::read(&plain).unwrap(), fs::read(&direct).unwrap());
    for enabled in [false, true] {
        if enabled {
            super::super::profile::start();
        }
        let path = directory.path(&format!("rejected-{enabled}.book"));
        let mut builder = NativeBookBuilder::new(&path, NativeBookMetadata::default()).unwrap();
        let mut bad = record();
        bad.entry.candidates[0].mv = Move::from_usi("5a5b").unwrap();
        assert!(builder.push(bad).is_err());
        assert!(builder.finish().is_err());
        assert!(!path.exists());
        if enabled {
            super::super::profile::finish();
        }
    }
}
