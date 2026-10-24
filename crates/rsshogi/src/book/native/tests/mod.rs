use super::*;
use crate::board::{self, Position};
use crate::book::{BookControl, BookError};
use crate::types::Move;
use std::fs;
use std::path::{Path, PathBuf};

mod flipped;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rsshogi_native_test_{}_{}",
            std::process::id(),
            TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let root = std::env::temp_dir().canonicalize().expect("temporary root");
        if let Ok(path) = self.0.canonicalize() {
            assert!(path.starts_with(root));
            assert!(
                path.file_name()
                    .expect("test directory name")
                    .to_string_lossy()
                    .starts_with("rsshogi_native_test_")
            );
            let _ = fs::remove_dir_all(path);
        }
    }
}

fn position() -> Position {
    board::init();
    board::hirate_position()
}
fn candidate(usi: &str, rank: u16) -> NativeCandidate {
    let mut candidate = NativeCandidate::new(Move::from_usi(usi).expect("USI move"));
    candidate.display_order = rank;
    candidate
}
fn record() -> NativeBookPosition {
    let mut first = candidate("7g7f", 0);
    first.score = NativeScore::Cp { value: 0, bound: NativeBound::Exact };
    first.depth = Some(u32::MAX);
    let mut record = NativeBookPosition::new(
        NativeBookEntry::from_position(&position(), vec![first, candidate("2g2f", 1)])
            .expect("entry"),
    );
    record.comment = Some("局面\r\nコメント".into());
    record
        .move_comments
        .insert(Move::from_usi("7g7f").expect("move").raw(), "候補手\r\n説明".into());
    record
}
fn create(path: &Path) {
    let mut builder = NativeBookBuilder::new(
        path,
        NativeBookMetadata {
            title: Some("題名".into()),
            description: Some("説明".into()),
            ..NativeBookMetadata::default()
        },
    )
    .expect("builder");
    builder.push(record()).expect("push");
    builder.finish().expect("finish");
}

#[test]
fn block_parameters_and_partial_corruption_detection() {
    use super::wire::Header;
    let directory = Directory::new();
    let path = directory.path("blocks.book");
    create(&path);
    let original = fs::read(&path).unwrap();
    let header = Header::decode(&original[..128]).unwrap();
    for block_positions in [1, 64, 256, 1024, 16_384] {
        let mut bytes = original.clone();
        let mut changed = header.clone();
        changed.block_positions = block_positions;
        bytes[..128].copy_from_slice(&changed.encode());
        fs::write(&path, bytes).unwrap();
        let book = NativeBook::open(&path).unwrap();
        book.validate_full().unwrap();
        assert_eq!(book.get(&position()).unwrap().unwrap(), record().entry);
    }
    for block_positions in [0, 16_385, u32::MAX] {
        let mut bytes = original.clone();
        let mut changed = header.clone();
        changed.block_positions = block_positions;
        bytes[..128].copy_from_slice(&changed.encode());
        fs::write(&path, bytes).unwrap();
        assert!(NativeBook::open(&path).is_err());
    }
    let mut bytes = original;
    bytes[header.blocks_offset as usize] ^= 1;
    fs::write(&path, bytes).unwrap();
    let book = NativeBook::open(&path).unwrap();
    assert!(book.get(&position()).is_err());
    assert!(book.get_with_flipped(&position()).is_err());
    assert!(book.validate_full().is_err());
}

#[test]
fn multiple_blocks_preserve_lookup_iteration_and_comments() {
    use crate::board::{LegalAll, MoveList, generate_moves};
    let directory = Directory::new();
    let path = directory.path("multiple.book");
    let start = position();
    let mut first_moves = MoveList::new();
    generate_moves::<LegalAll>(&start, &mut first_moves);
    let mut records = BTreeMap::new();
    'positions: for &first in first_moves.iter() {
        let mut child = start.clone();
        child.apply_move32(child.move32_from_move(first));
        let mut replies = MoveList::new();
        generate_moves::<LegalAll>(&child, &mut replies);
        for &reply in replies.iter() {
            let mut next = child.clone();
            next.apply_move32(next.move32_from_move(reply));
            let mut record =
                NativeBookPosition::new(NativeBookEntry::from_position(&next, vec![]).unwrap());
            record.comment = Some("コメント".into());
            records.insert(record.entry.key, record);
            if records.len() == 130 {
                break 'positions;
            }
        }
    }
    let mut builder = NativeBookBuilder::new(&path, NativeBookMetadata::default()).unwrap();
    for record in records.values().rev() {
        builder.push(record.clone()).unwrap();
    }
    builder.finish().unwrap();
    let book = NativeBook::open(&path).unwrap();
    assert_eq!(book.header.node_count, 130);
    book.validate_full().unwrap();
    for record in records.values() {
        assert_eq!(book.get_key(record.entry.key).unwrap(), Some(record.entry.clone()));
    }
    assert_eq!(
        book.records().collect::<Result<Vec<_>, _>>().unwrap(),
        records.into_values().collect::<Vec<_>>()
    );
}

#[test]
fn configurable_block_sizes_cross_boundaries_and_find_missing_keys() {
    use crate::board::{LegalAll, MoveList, generate_moves};
    let directory = Directory::new();
    fn collect_positions(
        position: &Position,
        depth: u8,
        records: &mut BTreeMap<[u8; 32], NativeBookEntry>,
    ) {
        if depth == 0 || records.len() == 16_385 {
            return;
        }
        let mut moves = MoveList::new();
        generate_moves::<LegalAll>(position, &mut moves);
        for &mv in moves.iter() {
            let mut child = position.clone();
            child.apply_move32(child.move32_from_move(mv));
            let entry = NativeBookEntry::from_position(&child, vec![]).unwrap();
            records.insert(entry.key, entry);
            collect_positions(&child, depth - 1, records);
            if records.len() == 16_385 {
                break;
            }
        }
    }
    let mut records = BTreeMap::new();
    collect_positions(&position(), 4, &mut records);
    let all_entries: Vec<_> = records.into_values().collect();
    assert_eq!(all_entries.len(), 16_385);
    for size in [1, 64, 256, 1024, 16_384] {
        let count = if size == 16_384 { 16_385 } else { 1025 };
        let entries = &all_entries[..count];
        let path = directory.path(&format!("block-{size}.book"));
        let mut builder = NativeBookBuilder::new(
            &path,
            NativeBookMetadata { depth_kind: u32::MAX, ..Default::default() },
        )
        .unwrap()
        .with_memory_budget(1024 * 1024)
        .unwrap()
        .with_block_positions(size)
        .unwrap();
        for entry in entries.iter().rev() {
            let mut record = NativeBookPosition::new(entry.clone());
            record.comment = Some("x".repeat(100));
            builder.push(record).unwrap();
        }
        builder.finish().unwrap();
        let book = NativeBook::open(&path).unwrap();
        assert_eq!(book.block_positions(), size);
        book.validate_full().unwrap();
        for entry in entries {
            assert_eq!(book.get_key(entry.key).unwrap().as_ref(), Some(entry));
        }
        assert!(book.get_key([0; 32]).unwrap().is_none());
        assert!(book.get_key([255; 32]).unwrap().is_none());
        assert_eq!(book.entries().collect::<Result<Vec<_>, _>>().unwrap(), entries.to_vec());
        let mut editor = NativeBookEditor::open(&path).unwrap();
        editor
            .set_position_comment(&entries[0].position().unwrap(), Some("updated".into()))
            .unwrap();
        editor.save().unwrap();
        let copy = directory.path(&format!("copy-{size}.book"));
        editor.write_to(&copy).unwrap();
        assert_eq!(NativeBook::open(&copy).unwrap().block_positions(), size);
        editor.compact().unwrap();
        assert_eq!(editor.book().block_positions(), size);
        assert_eq!(editor.book().metadata().unwrap().depth_kind, u32::MAX);
        let expected = editor.book().records().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(
            NativeBook::open(&copy).unwrap().records().collect::<Result<Vec<_>, _>>().unwrap(),
            expected
        );
        drop(editor);
        let rescued = directory.path(&format!("rescued-{size}.book"));
        NativeBookEditor::salvage_prefix(&path, &rescued).unwrap();
        let rescued = NativeBook::open(rescued).unwrap();
        assert_eq!(rescued.block_positions(), size);
        assert_eq!(rescued.records().collect::<Result<Vec<_>, _>>().unwrap(), expected);
    }
}

#[test]
fn reader_roundtrip_comments_metadata_order_presence_and_io() {
    let directory = Directory::new();
    let path = directory.path("sample.book");
    create(&path);
    let book = NativeBook::open(&path).expect("open");
    book.validate_full().expect("validate");
    let before = book.io_stats();
    let entry = book.get(&position()).expect("lookup").expect("entry");
    let after = book.io_stats();
    let calls = after.read_calls - before.read_calls;
    assert!(calls <= 1);
    assert_eq!(after.seek_calls - before.seek_calls, calls);
    assert_eq!(after.read_syscalls - before.read_syscalls, calls);
    let bytes = fs::read(&path).unwrap();
    let compressed_bytes = u64::from_le_bytes(bytes[64..72].try_into().unwrap());
    assert_eq!(after.read_bytes - before.read_bytes, calls * compressed_bytes);
    assert_eq!(book.get(&position()).unwrap(), Some(entry.clone()));
    assert_eq!(book.io_stats(), after);
    assert_eq!(entry, record().entry);
    let normalized = record().normalized().expect("normalize");
    assert_eq!(book.record(&position()).expect("record"), Some(normalized.clone()));
    assert_eq!(book.records().collect::<Result<Vec<_>, _>>().expect("records"), vec![normalized]);
    assert_eq!(book.metadata().expect("metadata").title.as_deref(), Some("題名"));
    assert_eq!(book.len().expect("length"), 1);
    let original = fs::read(&path).unwrap();
    let mut editor = NativeBookEditor::open(&path).unwrap();
    editor.set_metadata(NativeBookMetadata::default()).unwrap();
    editor.save().unwrap();
    let saved = fs::read(&path).unwrap();
    assert!(saved.starts_with(&original));
    assert_eq!(saved.len() - original.len(), 132);
    assert_eq!(editor.book().metadata().unwrap(), NativeBookMetadata::default());
    assert_eq!(book.metadata().unwrap().title.as_deref(), Some("題名"));
}

#[test]
fn depth_kind_survives_edits_save_as_and_compaction() {
    for depth_kind in [0, 1, 2, u32::MAX] {
        let directory = Directory::new();
        let path = directory.path("depth.book");
        let metadata = NativeBookMetadata { depth_kind, ..NativeBookMetadata::default() };
        let mut builder = NativeBookBuilder::new(&path, metadata.clone()).unwrap();
        builder.push(record()).unwrap();
        builder.finish().unwrap();
        let mut editor = NativeBookEditor::open(&path).unwrap();
        assert_eq!(editor.metadata().unwrap(), metadata);
        editor.set_position_comment(&position(), Some("edited".into())).unwrap();
        editor.save().unwrap();
        assert_eq!(NativeBook::open(&path).unwrap().metadata().unwrap(), metadata);
        let copy = directory.path("copy.book");
        editor.write_to(&copy).unwrap();
        assert_eq!(NativeBook::open(&copy).unwrap().metadata().unwrap(), metadata);
        editor.compact().unwrap();
        let compacted = NativeBook::open(&path).unwrap();
        compacted.validate_full().unwrap();
        assert_eq!(compacted.metadata().unwrap(), metadata);
        assert_eq!(compacted.get(&position()).unwrap().unwrap(), record().entry);

        let changed = NativeBookMetadata {
            title: Some("changed metadata".into()),
            depth_kind,
            ..NativeBookMetadata::default()
        };
        editor.set_metadata(changed.clone()).unwrap();
        editor.save().unwrap();
        assert_eq!(NativeBook::open(&path).unwrap().metadata().unwrap(), changed);
        editor.compact().unwrap();
        assert_eq!(NativeBook::open(&path).unwrap().metadata().unwrap(), changed);
    }
}

#[test]
fn editor_append_isolation_lock_order_delete_readd_compaction() {
    let directory = Directory::new();
    let path = directory.path("sample.book");
    create(&path);
    let original_bytes = fs::read(&path).expect("read");
    let original_reader = NativeBook::open(&path).expect("reader");
    let mut editor = NativeBookEditor::open(&path).expect("editor");
    assert!(matches!(NativeBookEditor::open(&path), Err(BookError::Busy)));
    let first_move = Move::from_usi("7g7f").expect("move");
    assert!(editor.reorder_candidate(&position(), first_move, 1).expect("reorder"));
    editor.set_metadata(NativeBookMetadata::default()).expect("metadata");
    assert!(editor.save().expect("save"));
    assert!(!editor.save().expect("no-op save"));
    assert!(fs::read(&path).expect("read").starts_with(&original_bytes));
    assert_eq!(
        original_reader.record(&position()).expect("old record").expect("exists").entry,
        record().entry
    );
    let saved = editor.record(&position()).expect("current record").expect("exists");
    assert_eq!(
        saved
            .entry
            .candidates
            .iter()
            .find(|candidate| candidate.mv == first_move)
            .expect("candidate")
            .display_order,
        1
    );
    assert!(editor.remove_candidate(&position(), first_move).expect("remove candidate"));
    editor.save().expect("save");
    assert!(editor.remove_position(&position()).expect("remove position"));
    editor.save().expect("save");
    assert!(editor.book().is_empty().expect("empty"));
    editor.set_position_comment(&position(), Some("new".into())).expect("readd");
    editor.save().expect("save");
    let before = editor.book().records().collect::<Result<Vec<_>, _>>().expect("records");
    assert!(before[0].entry.candidates.is_empty());
    assert!(before[0].move_comments.is_empty());
    let precompact = NativeBook::open(&path).expect("old snapshot");
    editor.compact().expect("compact with readers open");
    assert_eq!(editor.book().sequence(), 0);
    assert_eq!(
        editor.book().records().collect::<Result<Vec<_>, _>>().expect("new records"),
        before
    );
    assert_eq!(precompact.records().collect::<Result<Vec<_>, _>>().expect("old records"), before);
    assert_eq!(original_reader.metadata().expect("original meta").title.as_deref(), Some("題名"));
    editor.book().validate_full().expect("full validation");
}

#[test]
fn every_transaction_cut_is_classified_and_recoverable_without_partial_apply() {
    let directory = Directory::new();
    let source = directory.path("source.book");
    create(&source);
    let base = fs::read(&source).expect("base");
    {
        let mut editor = NativeBookEditor::open(&source).expect("editor");
        editor.set_position_comment(&position(), Some("changed".into())).expect("edit");
        editor.save().expect("save");
    }
    let bytes = fs::read(&source).expect("saved");
    let frame_len = bytes.len() - base.len();
    let cut_path = directory.path("cut.book");
    for cut in 1..frame_len {
        fs::write(&cut_path, &bytes[..base.len() + cut]).expect("write cut");
        if cut < 40 {
            assert!(matches!(NativeBook::open(&cut_path), Err(BookError::NativeLog { .. })));
            assert!(NativeBookEditor::recover_tail(&cut_path).is_err());
        } else {
            let book = NativeBook::open(&cut_path).expect("valid prefix");
            assert!(book.tail().is_some());
            assert_eq!(book.sequence(), 0);
            assert_eq!(
                book.position_comment(&position()).expect("comment"),
                record().normalized().expect("normalized").comment
            );
        }
    }
    fs::write(&cut_path, &bytes[..bytes.len() - 1]).expect("write tail");
    let backup = NativeBookEditor::recover_tail(&cut_path).expect("recover");
    assert_eq!(fs::read(&cut_path).expect("restored base"), base);
    let backup_bytes = fs::read(backup).expect("backup");
    assert_eq!(&backup_bytes[..8], b"RSSHGBAD");
    assert_eq!(&backup_bytes[24..], &bytes[base.len()..bytes.len() - 1]);
    assert!(NativeBookEditor::open(&cut_path).is_ok());
}

#[test]
fn corrupt_length_footer_and_saved_frame_salvage_preserves_source() {
    let directory = Directory::new();
    let source = directory.path("source.book");
    create(&source);
    let base = fs::read(&source).expect("base");
    {
        let mut editor = NativeBookEditor::open(&source).expect("editor");
        editor.set_position_comment(&position(), Some("committed".into())).expect("edit");
        editor.save().expect("save");
    }
    let mut corrupt = fs::read(&source).expect("saved");
    corrupt[base.len() + 16] ^= 1;
    fs::write(&source, &corrupt).expect("corrupt header length");
    assert!(matches!(
        NativeBook::open(&source),
        Err(BookError::NativeLog { reason: "invalid transaction header", .. })
    ));
    assert!(NativeBookEditor::recover_tail(&source).is_err());
    let output = directory.path("salvaged.book");
    let report = NativeBookEditor::salvage_prefix(&source, &output).expect("salvage");
    assert_eq!(report.valid_end, base.len() as u64);
    assert_eq!(report.sequence, 0);
    assert_eq!(fs::read(&source).expect("unchanged source"), corrupt);
    assert_eq!(
        NativeBook::open(&output).expect("output").record(&position()).expect("record"),
        record().normalized().ok()
    );
    assert!(NativeBookEditor::salvage_prefix(&source, &output).is_err());
    let mut broken_base = base;
    broken_base[128] ^= 4;
    fs::write(&source, &broken_base).expect("corrupt base");
    assert!(NativeBookEditor::salvage_prefix(&source, directory.path("bad.book")).is_err());
}

#[test]
fn short_tail_salvage_empty_file_and_cancelled_validation() {
    let directory = Directory::new();
    let source = directory.path("source.book");
    fs::write(&source, []).expect("zero bytes");
    assert!(NativeBook::open(&source).is_err());
    fs::remove_file(&source).expect("remove empty");
    drop(NativeBookEditor::create(&source, NativeBookMetadata::default()).expect("empty book"));
    assert_eq!(fs::metadata(&source).expect("metadata").len(), 128);
    let book = NativeBook::open(&source).expect("open");
    assert!(matches!(
        book.validate_full_with_control(|_| BookControl::Cancel),
        Err(BookError::Cancelled)
    ));
    let mut file = fs::OpenOptions::new().append(true).open(&source).expect("append");
    use std::io::Write;
    file.write_all(b"RSS").expect("short tail");
    drop(file);
    let output = directory.path("rescued.book");
    let report = NativeBookEditor::salvage_prefix(&source, &output).expect("salvage short tail");
    assert_eq!(report.valid_end, 128);
    assert_eq!(fs::metadata(&source).expect("original retained").len(), 131);
    NativeBook::open(&output).expect("output").validate_full().expect("valid empty output");
}

#[test]
fn superseded_log_text_is_validated_and_salvage_stops_before_it() {
    let directory = Directory::new();
    let source = directory.path("history.book");
    create(&source);
    let base_end = fs::metadata(&source).expect("base metadata").len() as usize;
    {
        let mut editor = NativeBookEditor::open(&source).expect("editor");
        editor.set_position_comment(&position(), Some("bad".into())).expect("comment");
        editor.save().expect("first save");
        editor.set_position_comment(&position(), Some("current".into())).expect("comment");
        editor.save().expect("second save");
    }
    let mut bytes = fs::read(&source).expect("read");
    let payload_len =
        u64::from_le_bytes(bytes[base_end + 16..base_end + 24].try_into().expect("length"))
            as usize;
    let text_start = base_end + 40 + 48 + 16 + 16 * 2 + 8 * 4;
    bytes[text_start] = 0xff;
    let footer = base_end + 40 + payload_len;
    let crc = super::crc::crc32c(&bytes[base_end..footer]);
    bytes[footer + 16..footer + 20].copy_from_slice(&crc.to_le_bytes());
    fs::write(&source, &bytes).expect("write forged valid CRC");
    let book = NativeBook::open(&source).expect("structural open");
    assert_eq!(book.position_comment(&position()).expect("latest"), Some("current".into()));
    assert!(
        matches!(book.validate_full(), Err(BookError::NativeLog { valid_end, sequence: 0, .. }) if valid_end == base_end as u64)
    );
    let report =
        NativeBookEditor::salvage_prefix(&source, directory.path("rescued.book")).expect("salvage");
    assert_eq!(report.sequence, 0);
    assert_eq!(report.error_offset, base_end as u64);
    assert_eq!(report.valid_end, base_end as u64);
    assert_eq!(fs::read(&source).expect("original"), bytes);
}

#[test]
fn native_lock_child() {
    let Some(path) = std::env::var_os("RSSHOGI_NATIVE_LOCK_CHILD") else {
        return;
    };
    let busy = std::env::var_os("RSSHOGI_NATIVE_EXPECT_BUSY").is_some();
    let result = NativeBookEditor::open(PathBuf::from(path));
    if busy {
        assert!(matches!(result, Err(BookError::Busy)));
    } else {
        assert!(result.is_ok());
        if std::env::var_os("RSSHOGI_NATIVE_EXIT_WITH_LOCK").is_some() {
            // Dropを実行せず終了し、OSによるロック回収を親で確認する。
            std::process::exit(77);
        }
    }
}

#[test]
fn writer_lock_is_enforced_across_processes_and_released_on_exit() {
    let directory = Directory::new();
    let path = directory.path("locked.book");
    let editor = NativeBookEditor::create(&path, NativeBookMetadata::default()).expect("create");
    let run_child = |busy: bool| {
        let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
        command
            .args(["--exact", "book::native::tests::native_lock_child", "--nocapture"])
            .env("RSSHOGI_NATIVE_LOCK_CHILD", &path);
        if busy {
            command.env("RSSHOGI_NATIVE_EXPECT_BUSY", "1");
        }
        assert!(command.output().expect("child").status.success());
    };
    run_child(true);
    drop(editor);
    run_child(false);
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "book::native::tests::native_lock_child"])
        .env("RSSHOGI_NATIVE_LOCK_CHILD", &path)
        .env("RSSHOGI_NATIVE_EXIT_WITH_LOCK", "1")
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(77));
    assert!(NativeBookEditor::open(&path).is_ok());
}

#[test]
fn append_and_sync_failures_poison_writer_and_keep_dirty() {
    use std::io::Write;
    for cut in [0, 1, 39, 40, 80, usize::MAX] {
        let directory = Directory::new();
        let path = directory.path("fault.book");
        create(&path);
        let mut editor = NativeBookEditor::open(&path).expect("editor");
        editor.set_position_comment(&position(), Some("changed".into())).expect("edit");
        let result = editor.save_with(|path, _, frame| {
            let mut file = fs::OpenOptions::new().append(true).open(path)?;
            let written = cut.min(frame.len());
            file.write_all(&frame[..written])?;
            Err(BookError::Io(std::io::Error::other(if cut == usize::MAX {
                "injected sync failure"
            } else {
                "injected write failure"
            })))
        });
        assert!(matches!(result, Err(BookError::SaveUncertain(_))));
        assert!(editor.is_dirty());
        let length = fs::metadata(&path).expect("metadata").len();
        assert!(matches!(editor.save(), Err(BookError::SaveUncertain(_))));
        assert_eq!(fs::metadata(&path).expect("metadata").len(), length);
        drop(editor);
        if cut == usize::MAX {
            let reopened = NativeBook::open(&path).expect("complete frame may survive failed sync");
            assert_eq!(reopened.sequence(), 1);
            assert_eq!(
                reopened.position_comment(&position()).expect("comment"),
                Some("changed".into())
            );
        }
    }
}

#[test]
fn save_as_includes_unsaved_changes_without_mutating_source() {
    let directory = Directory::new();
    let path = directory.path("source.book");
    let output = directory.path("output.book");
    create(&path);
    let original = fs::read(&path).expect("original");
    let mut editor = NativeBookEditor::open(&path).expect("editor");
    editor.set_position_comment(&position(), Some("dirty".into())).expect("edit");
    editor
        .set_metadata(NativeBookMetadata {
            title: Some("new title".into()),
            description: None,
            ..NativeBookMetadata::default()
        })
        .expect("metadata");
    editor.write_to(&output).expect("write copy");
    assert!(editor.is_dirty());
    assert_eq!(fs::read(&path).expect("original retained"), original);
    let copy = NativeBook::open(&output).expect("copy");
    assert_eq!(copy.position_comment(&position()).expect("comment"), Some("dirty".into()));
    assert_eq!(copy.metadata().expect("metadata").title.as_deref(), Some("new title"));
    assert!(editor.write_to(&output).is_err());
    editor.write_to_with_replace(&output, true).expect("explicit replacement");
    copy.validate_full().expect("old reader remains valid");
}

#[test]
fn comment_index_search_crosses_255_and_256_in_numeric_tuple_order() {
    use crate::board::{LegalAll, MoveList, generate_moves};
    let directory = Directory::new();
    let path = directory.path("comments.book");
    let start = position();
    let mut first = MoveList::new();
    generate_moves::<LegalAll>(&start, &mut first);
    let mut records = Vec::new();
    for &mv in first.iter().take(18) {
        let mut child = start.clone();
        child.apply_move32(child.move32_from_move(mv));
        let mut replies = MoveList::new();
        generate_moves::<LegalAll>(&child, &mut replies);
        for &reply in replies.iter().take(18) {
            let mut target = child.clone();
            target.apply_move32(target.move32_from_move(reply));
            records.push(NativeBookPosition::new(
                NativeBookEntry::from_position(&target, vec![]).unwrap(),
            ));
        }
    }
    records.sort_by_key(|record| record.entry.key);
    assert_eq!(records.len(), 324);
    let mut builder = NativeBookBuilder::new(&path, NativeBookMetadata::default()).unwrap();
    for (index, record) in records.iter_mut().enumerate() {
        record.comment = Some(format!("position {index}"));
        builder.push(record.clone()).unwrap();
    }
    builder.finish().unwrap();
    let book = NativeBook::open(&path).unwrap();
    for index in [0, 1, 254, 255, 256, 257, 323] {
        assert_eq!(
            book.position_comment(&records[index].entry.position().unwrap()).unwrap(),
            records[index].comment
        );
    }
    let packed = crate::board::PackedSfen::from_bytes(records[0].entry.key);
    assert!(Position::sfen_unpack(&packed).unwrap().ends_with(" 0"));
    assert!(records[0].entry.position().unwrap().to_sfen(None).ends_with(" 1"));
}

#[test]
fn truncated_multi_position_and_metadata_transaction_has_no_partial_effect() {
    let directory = Directory::new();
    let path = directory.path("atomic.book");
    create(&path);
    let base = fs::read(&path).unwrap();
    let mut child = position();
    let mv = Move::from_usi("7g7f").unwrap();
    child.apply_move32(child.move32_from_move(mv));
    {
        let mut editor = NativeBookEditor::open(&path).unwrap();
        editor.set_position_comment(&position(), Some("first edit".into())).unwrap();
        editor.set_position_comment(&position(), Some("last edit".into())).unwrap();
        editor.set_position_comment(&child, Some("second position".into())).unwrap();
        editor
            .set_metadata(NativeBookMetadata {
                title: None,
                description: Some("new".into()),
                ..NativeBookMetadata::default()
            })
            .unwrap();
        editor.save().unwrap();
    }
    let saved = fs::read(&path).unwrap();
    let committed = NativeBook::open(&path).unwrap();
    assert_eq!(committed.position_comment(&position()).unwrap().as_deref(), Some("last edit"));
    assert_eq!(committed.position_comment(&child).unwrap().as_deref(), Some("second position"));
    assert_eq!(committed.metadata().unwrap().title, None);
    for cut in 40..saved.len() - base.len() {
        let truncated = directory.path("cut.book");
        fs::write(&truncated, &saved[..base.len() + cut]).unwrap();
        let book = NativeBook::open(&truncated).unwrap();
        assert_eq!(book.sequence(), 0);
        assert!(book.get(&child).unwrap().is_none());
        assert_eq!(book.metadata().unwrap().title.as_deref(), Some("題名"));
        assert!(NativeBookEditor::open(&truncated).is_err());
    }
}

#[test]
fn complete_corrupt_frames_are_never_treated_as_recoverable_tails() {
    let directory = Directory::new();
    let path = directory.path("corrupt.book");
    create(&path);
    let base_end = fs::metadata(&path).unwrap().len() as usize;
    {
        let mut editor = NativeBookEditor::open(&path).unwrap();
        editor.set_position_comment(&position(), Some("saved".into())).unwrap();
        editor.save().unwrap();
    }
    let original = fs::read(&path).unwrap();
    for (offset, value, repair_crc) in [
        (base_end + 8, 2, true),              // sequence
        (base_end + 40, 9, true),             // opcode
        (base_end + 41, 1, true),             // operation reserved
        (base_end + 88 + 16 + 2, 255, true),  // candidate kind
        (base_end + 88 + 16 + 2, 255, false), // payload CRC
        (original.len() - 24, 0, false),      // footer magic
        (original.len() - 16, 2, false),      // footer sequence
        (original.len() - 1, 1, false),       // footer reserved
    ] {
        let mut bytes = original.clone();
        bytes[offset] = value;
        if repair_crc {
            bytes[base_end + 32..base_end + 36].fill(0);
            let crc = super::crc::crc32c(&bytes[base_end..base_end + 40]);
            bytes[base_end + 32..base_end + 36].copy_from_slice(&crc.to_le_bytes());
            let end = bytes.len();
            let crc = super::crc::crc32c(&bytes[base_end..end - 24]);
            bytes[end - 8..end - 4].copy_from_slice(&crc.to_le_bytes());
        }
        fs::write(&path, &bytes).unwrap();
        assert!(matches!(NativeBook::open(&path), Err(BookError::NativeLog { sequence: 0, .. })));
        assert!(NativeBookEditor::recover_tail(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn failed_backup_sync_never_truncates_and_failed_compact_reopen_poisons_writer() {
    let directory = Directory::new();
    let path = directory.path("failure.book");
    create(&path);
    let mut editor = NativeBookEditor::open(&path).unwrap();
    editor.set_position_comment(&position(), Some("committed".into())).unwrap();
    editor.save().unwrap();
    let result =
        editor.compact_with(|_| Err(std::io::Error::other("injected reopen failure").into()));
    assert!(matches!(result, Err(BookError::SaveUncertain(_))));
    let compacted = fs::read(&path).unwrap();
    assert!(matches!(editor.save(), Err(BookError::SaveUncertain(_))));
    assert!(editor.set_position_comment(&position(), None).is_err());
    assert_eq!(fs::read(&path).unwrap(), compacted);
    drop(editor);
    let mut editor = NativeBookEditor::open(&path).unwrap();
    assert_eq!(editor.record(&position()).unwrap().unwrap().comment.as_deref(), Some("committed"));
    editor.set_position_comment(&position(), Some("tail".into())).unwrap();
    editor.save().unwrap();
    drop(editor);
    let mut interrupted = fs::read(&path).unwrap();
    interrupted.pop();
    fs::write(&path, &interrupted).unwrap();
    let result = NativeBookEditor::recover_tail_with(&path, |_| {
        Err(std::io::Error::other("injected backup sync failure").into())
    });
    assert!(result.is_err());
    assert_eq!(fs::read(&path).unwrap(), interrupted);
    assert!(NativeBookEditor::open(&path).is_err());
    NativeBookEditor::recover_tail(&path).unwrap();
    assert!(NativeBookEditor::open(&path).is_ok());
}

#[test]
fn individual_comment_reads_do_not_fetch_other_comment_bodies() {
    let directory = Directory::new();
    let path = directory.path("selective.book");
    let mut value = record();
    let long_move = Move::from_usi("7g7f").unwrap();
    let short_move = Move::from_usi("2g2f").unwrap();
    value.comment = Some("root".into());
    value.move_comments.insert(long_move.raw(), "x".repeat(64 * 1024));
    value.move_comments.insert(short_move.raw(), "short".into());
    let mut builder = NativeBookBuilder::new(&path, NativeBookMetadata::default()).unwrap();
    builder.push(value.clone()).unwrap();
    builder.finish().unwrap();
    for logged in [false, true] {
        if logged {
            let mut editor = NativeBookEditor::open(&path).unwrap();
            value.comment = Some("updated".into());
            editor.upsert_position(value.clone()).unwrap();
            editor.save().unwrap();
        }
        let book = NativeBook::open(&path).unwrap();
        let before = book.io_stats();
        assert_eq!(book.position_comment(&position()).unwrap(), value.comment);
        assert_eq!(book.move_comment(&position(), short_move).unwrap().as_deref(), Some("short"));
        assert!(book.io_stats().read_bytes - before.read_bytes < 1024);
        assert_eq!(book.move_comment(&position(), long_move).unwrap().unwrap().len(), 64 * 1024);
        assert!(book.move_comment(&position(), Move::from_usi("6g6f").unwrap()).unwrap().is_none());
    }
}
