use std::fs;
use std::path::PathBuf;

use super::*;
use crate::board;
use crate::book::{
    BookCandidate, BookEntryMetadata, BookMoveMetadata, BookPosition, NativeBook, SbkEntryMetadata,
    SbkEvalMetadata, SbkMoveMetadata, SbkWriteOptions, YaneuraOuAccessMode,
    YaneuraOuBookOpenOptions, YbbBookOpenOptions, YbbEntry, YbbMove, YbbStorageMode,
    YbbValidationMode, YbbWriteOptions, YbbWriter,
};
use crate::types::Move;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        board::init();
        let path = super::super::temporary_path(
            &std::env::temp_dir().join("rsshogi-native-import"),
            "test",
        );
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
            "source.db",
            "source.sbk",
            "source.ybb",
            "output.book",
            "strict.book",
            "skip.book",
            "bad.book",
        ] {
            let _ = fs::remove_file(self.path(name));
        }
        fs::remove_dir(&self.0).expect("conversion must clean temporary runs");
    }
}

fn candidate(usi: &str, score: Option<i32>, weight: Option<i32>) -> BookCandidate {
    BookCandidate::new(
        Move::from_usi(usi).unwrap(),
        None,
        score,
        Some(0),
        weight,
        None,
        BookMoveMetadata::new(),
    )
}

fn database(candidates: Vec<BookCandidate>, metadata: BookEntryMetadata) -> BookDatabase {
    BookDatabase::try_from_entries(vec![BookDatabaseEntry::new(
        BookPosition::from_position(&board::hirate_position(), None),
        candidates,
        metadata,
    )])
    .unwrap()
}

#[test]
fn database_preserves_optional_values_comments_and_display_order() {
    let directory = TestDirectory::new();
    let first = BookCandidate::new(
        Move::from_usi("7g7f").unwrap(),
        None,
        None,
        None,
        None,
        None,
        BookMoveMetadata::from_yaneuraou("first\r\ncomment".into()),
    );
    let second = candidate("2g2f", Some(0), None);
    let database = database(
        vec![first, second],
        BookEntryMetadata::from_yaneuraou(1, "position\rcomment".into()),
    );
    let path = directory.path("output.book");
    let report = database
        .write_native_book(&path, &NativeBookImportOptions::new().with_allow_lossy(true))
        .unwrap();
    assert_eq!(report.positions, 1);
    assert_eq!(report.candidates, 2);
    assert_eq!(report.dropped_fields.get("yaneuraou.min_ply"), Some(&1));
    let book = NativeBook::open(&path).unwrap();
    book.validate_full().unwrap();
    let record = book.record(&board::hirate_position()).unwrap().unwrap();
    assert_eq!(record.comment.as_deref(), Some("position\ncomment"));
    let first =
        record.entry.candidates.iter().find(|candidate| candidate.mv.to_usi() == "7g7f").unwrap();
    let second =
        record.entry.candidates.iter().find(|candidate| candidate.mv.to_usi() == "2g2f").unwrap();
    assert_eq!(first.score, NativeScore::Unknown);
    assert_eq!(first.depth, None);
    assert_eq!(first.display_order, 0);
    assert_eq!(second.score, NativeScore::Cp { value: 0, bound: NativeBound::Exact });
    assert_eq!(second.depth, Some(0));
    assert_eq!(second.display_order, 1);
    assert_eq!(
        record.move_comments.get(&first.mv.raw()).map(String::as_str),
        Some("first\ncomment")
    );
}

#[test]
fn strict_weight_loss_leaves_no_output_and_lossy_never_turns_weight_into_score() {
    let directory = TestDirectory::new();
    let database = database(vec![candidate("7g7f", None, Some(123))], BookEntryMetadata::new());
    let strict = directory.path("strict.book");
    let error =
        database.write_native_book(&strict, &NativeBookImportOptions::default()).unwrap_err();
    assert!(error.to_string().contains("candidate.weight"));
    assert!(!strict.exists());
    let path = directory.path("output.book");
    let report = database
        .write_native_book(&path, &NativeBookImportOptions::new().with_allow_lossy(true))
        .unwrap();
    assert_eq!(report.dropped_fields.get("candidate.weight"), Some(&1));
    let entry = NativeBook::open(&path).unwrap().get(&board::hirate_position()).unwrap().unwrap();
    assert_eq!(entry.candidates[0].score, NativeScore::Unknown);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn candidate_counts_require_lossy_permission_including_zero() {
    for count in [0, u64::MAX] {
        let directory = TestDirectory::new();
        let source = BookCandidate::new(
            Move::from_usi("7g7f").unwrap(),
            None,
            Some(25),
            Some(16),
            None,
            Some(count),
            BookMoveMetadata::new(),
        );
        let database = database(vec![source], BookEntryMetadata::new());
        let strict = directory.path("strict.book");
        let error =
            database.write_native_book(&strict, &NativeBookImportOptions::default()).unwrap_err();
        assert!(error.to_string().contains("candidate.count"));
        assert!(!strict.exists());

        let output = directory.path("output.book");
        let report = database
            .write_native_book(&output, &NativeBookImportOptions::new().with_allow_lossy(true))
            .unwrap();
        assert_eq!(report.dropped_fields.get("candidate.count"), Some(&1));
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].field, "candidate.count");
        assert_eq!(report.issues[0].input_index, 0);
        assert_eq!(
            report.issues[0].sfen.as_deref(),
            Some(board::hirate_position().to_sfen(None).as_str())
        );
        let book = NativeBook::open(&output).unwrap();
        book.validate_full().unwrap();
        let entry = book.get(&board::hirate_position()).unwrap().unwrap();
        assert_eq!(entry.candidates[0].depth, Some(16));
        assert_eq!(
            entry.candidates[0].score,
            NativeScore::Cp { value: 25, bound: NativeBound::Exact }
        );
    }
}

#[test]
fn explicit_cp_is_preserved_for_white_without_sign_or_range_narrowing() {
    let directory = TestDirectory::new();
    let sfen = board::hirate_position().to_sfen(None).replace(" b ", " w ");
    let position = Position::from_sfen(&sfen).unwrap();
    let entry = BookDatabaseEntry::new(
        BookPosition::from_position(&position, None),
        vec![candidate("3c3d", Some(i32::MIN), None), candidate("8c8d", Some(i32::MAX), None)],
        BookEntryMetadata::new(),
    );
    let database = BookDatabase::try_from_entries(vec![entry]).unwrap();
    let path = directory.path("output.book");
    let report = database.write_native_book(&path, &NativeBookImportOptions::default()).unwrap();
    assert!(report.dropped_fields.is_empty());
    let record = NativeBook::open(&path).unwrap().get(&position).unwrap().unwrap();
    let values: Vec<_> = record.candidates.iter().map(|candidate| candidate.score).collect();
    assert!(values.contains(&NativeScore::Cp { value: i32::MIN, bound: NativeBound::Exact }));
    assert!(values.contains(&NativeScore::Cp { value: i32::MAX, bound: NativeBound::Exact }));
}

#[test]
fn representability_reports_sfen_index_and_bounded_examples() {
    let directory = TestDirectory::new();
    let path = directory.path("skip.book");
    let options = NativeBookImportOptions::new().with_skip_unencodable_positions(true);
    let sfen = "9/9/9/9/9/9/9/9/4K4 b - 1";
    let mut import = Import::new(&path, NativeBookMetadata::default(), &options).unwrap();
    for index in 0..40 {
        assert!(import.position(index, sfen, std::iter::empty()).unwrap().is_none());
    }
    let report = import.finish().unwrap();
    assert_eq!(report.skipped_positions, 40);
    assert_eq!(report.positions, 0);
    assert_eq!(report.dropped_fields.get("position"), Some(&40));
    assert_eq!(report.issues.len(), 32);
    assert_eq!(report.omitted_issues, 8);
    assert_eq!(report.issues[31].input_index, 31);
    assert_eq!(report.issues[0].sfen.as_deref(), Some(sfen));
    let strict = directory.path("strict.book");
    let options = NativeBookImportOptions::new();
    let mut import = Import::new(&strict, NativeBookMetadata::default(), &options).unwrap();
    let error = import.position(7, sfen, std::iter::empty()).err().unwrap().to_string();
    assert!(error.contains("input 7"));
    assert!(error.contains("9/9/9/9/9/9/9/9/4K4"));
    assert!(!strict.exists());
}

#[test]
fn yaneuraou_stream_conversion_and_source_io_errors() {
    let directory = TestDirectory::new();
    let source = directory.path("source.db");
    let sfen = board::hirate_position().to_sfen(None);
    fs::write(&source, format!("#YANEURAOU-DB2016 1.00\nsfen {sfen}\n7g7f none 0 0 9\n")).unwrap();
    let book = YaneuraOuBook::open(&source).unwrap();
    let path = directory.path("output.book");
    let report = book
        .write_native_book(&path, &NativeBookImportOptions::new().with_allow_lossy(true))
        .unwrap();
    assert_eq!(report.positions, 1);
    assert_eq!(report.dropped_fields.get("candidate.count"), Some(&1));
    assert_eq!(NativeBook::open(&path).unwrap().metadata().unwrap().depth_kind, 0);
    let entry = NativeBook::open(&path).unwrap().get(&board::hirate_position()).unwrap().unwrap();
    assert_eq!(entry.candidates[0].score, NativeScore::Cp { value: 0, bound: NativeBound::Exact });
    fs::remove_file(&source).unwrap();
    let bad = directory.path("bad.book");
    assert!(matches!(
        book.write_native_book(&bad, &NativeBookImportOptions::new().with_allow_lossy(true)),
        Err(BookError::Io(_))
    ));
    assert!(!bad.exists());
}

#[test]
fn sbk_state_analysis_and_move_annotations_do_not_become_cp() {
    let directory = TestDirectory::new();
    let mv = Move::from_usi("7g7f").unwrap();
    let source_candidate = BookCandidate::new(
        mv,
        None,
        Some(1),
        None,
        Some(77),
        None,
        BookMoveMetadata::from_sbk(SbkMoveMetadata::new(0x0100_7677, Some(1), Some(77), Some(-1))),
    );
    let metadata = BookEntryMetadata::from_sbk(SbkEntryMetadata::new(
        Some(9),
        0,
        Some(55),
        None,
        None,
        Some("state comment".into()),
        vec![SbkEvalMetadata::new(
            Some(900),
            Some(30),
            None,
            Some(10),
            Some("7g7f".into()),
            Some("engine".into()),
        )],
    ));
    let database = database(vec![source_candidate], metadata);
    let source = directory.path("source.sbk");
    fs::write(&source, database.to_sbk_bytes(&SbkWriteOptions::new()).unwrap()).unwrap();
    let book = SbkBook::open(&source).unwrap();
    let strict = directory.path("strict.book");
    assert!(book.write_native_book(&strict, &NativeBookImportOptions::default()).is_err());
    assert!(!strict.exists());
    let path = directory.path("output.book");
    let report = book
        .write_native_book(&path, &NativeBookImportOptions::new().with_allow_lossy(true))
        .unwrap();
    assert_eq!(report.dropped_fields.get("sbk.state_eval"), Some(&1));
    assert_eq!(report.dropped_fields.get("sbk.state_eval.pv"), Some(&1));
    assert_eq!(report.dropped_fields.get("sbk.move_evaluation"), Some(&1));
    let book = NativeBook::open(&path).unwrap();
    book.validate_full().unwrap();
    let record = book.record(&board::hirate_position()).unwrap().unwrap();
    assert_eq!(record.comment.as_deref(), Some("state comment"));
    assert_eq!(record.entry.candidates[0].score, NativeScore::Unknown);
    assert_eq!(record.entry.candidates[0].depth, None);
}

fn options_variants() -> [NativeBookImportOptions; 4] {
    std::array::from_fn(|bits| {
        NativeBookImportOptions::new()
            .with_allow_lossy(bits & 1 != 0)
            .with_skip_unencodable_positions(bits & 2 != 0)
    })
}

fn open_db(directory: &TestDirectory, input: &str) -> YaneuraOuBook {
    let path = directory.path("source.db");
    fs::write(&path, format!("#YANEURAOU-DB2016 1.00\n{input}")).unwrap();
    YaneuraOuBook::open_with_options(
        &path,
        YaneuraOuBookOpenOptions::with_access_mode(YaneuraOuAccessMode::AssumeSortedByCaller),
    )
    .unwrap()
}

#[test]
fn db2016_metadata_depth_and_loss_counts_match_owned_conversion() {
    for ply in [None, Some(0), Some(37)] {
        for depth_kind in [0, 1, 2, u32::MAX] {
            let directory = TestDirectory::new();
            let sfen = board::hirate_position().to_sfen(ply);
            let sfen = if ply.is_none() { sfen.rsplit_once(' ').unwrap().0 } else { &sfen };
            let book = open_db(
                &directory,
                &format!("sfen {sfen}\n# position\n2g2f\n# first\n7g7f none 0 0\n"),
            );
            let options = NativeBookImportOptions::new()
                .with_metadata(NativeBookMetadata {
                    title: Some("title\r\n".into()),
                    description: Some("description".into()),
                    depth_kind: 99,
                })
                .with_depth_kind(depth_kind)
                .with_allow_lossy(ply.is_some());
            let path = directory.path("output.book");
            let report = book.write_native_book(&path, &options).unwrap();
            assert_eq!(report.dropped_fields.get("position.original_ply").copied(), ply.map(|_| 1));
            assert!(!report.dropped_fields.contains_key("yaneuraou.min_ply"));
            let native = NativeBook::open(&path).unwrap();
            native.validate_full().unwrap();
            assert_eq!(
                native.metadata().unwrap(),
                NativeBookMetadata {
                    title: Some("title\n".into()),
                    description: Some("description".into()),
                    depth_kind,
                }
            );
            let expected = native.record(&board::hirate_position()).unwrap().unwrap();
            let first = expected.entry.candidates.iter().find(|c| c.mv.to_usi() == "2g2f").unwrap();
            let second =
                expected.entry.candidates.iter().find(|c| c.mv.to_usi() == "7g7f").unwrap();
            assert_eq!(
                (first.score, first.depth, first.display_order),
                (NativeScore::Unknown, None, 0)
            );
            assert_eq!((second.depth, second.display_order), (Some(0), 1));
            let database = BookDatabase::try_from_entries(
                book.iter_entries()
                    .unwrap()
                    .map(|e| BookDatabase::entry_from_yaneuraou(&e.unwrap()).unwrap())
                    .collect(),
            )
            .unwrap();
            let owned_path = directory.path("strict.book");
            assert_eq!(database.write_native_book(&owned_path, &options).unwrap(), report);
            assert_eq!(
                NativeBook::open(owned_path)
                    .unwrap()
                    .record(&board::hirate_position())
                    .unwrap()
                    .unwrap(),
                expected
            );
        }
    }
    assert_eq!(NativeBookImportOptions::new(), NativeBookImportOptions::default());
    assert_eq!(
        NativeBookImportOptions::new()
            .with_depth_kind(1)
            .with_metadata(NativeBookMetadata::default())
            .metadata
            .depth_kind,
        0
    );
}

#[test]
fn db2016_and_database_check_candidates_before_skipping() {
    for sfen in ["9/9/9/9/9/9/9/9/4K4 b - 7", "4k4/9/9/9/9/9/9/9/9 b - 7"] {
        for options in options_variants() {
            let directory = TestDirectory::new();
            let source = open_db(&directory, &format!("sfen {sfen}\n7g7f\n"));
            let error = source.write_native_book(directory.path("bad.book"), &options).unwrap_err();
            assert!(error.to_string().contains("illegal candidate"));
            let database = BookDatabase::try_from_entries(vec![BookDatabaseEntry::new(
                BookPosition::from_sfen(sfen, Some(7)).unwrap(),
                vec![candidate("7g7f", None, None)],
                BookEntryMetadata::new(),
            )])
            .unwrap();
            assert!(
                database
                    .write_native_book(directory.path("bad.book"), &options)
                    .unwrap_err()
                    .to_string()
                    .contains("illegal candidate")
            );
            let source = open_db(&directory, &format!("sfen {sfen}\n"));
            let result = source.write_native_book(directory.path("skip.book"), &options);
            if options.skip_unencodable_positions {
                let report = result.unwrap();
                assert_eq!(report.skipped_positions, 1);
                assert_eq!(report.dropped_fields, BTreeMap::from([("position".into(), 1)]));
            } else {
                assert!(result.is_err());
            }
            assert!(!directory.path("bad.book").exists());
        }
    }
    for options in options_variants() {
        let directory = TestDirectory::new();
        let source = open_db(&directory, "sfen 9/9/9/9/9/9/9/9/4K4 b - 1\n5i5h\n5i5h\n");
        assert!(
            source
                .write_native_book(directory.path("bad.book"), &options)
                .unwrap_err()
                .to_string()
                .contains("duplicate candidate")
        );
        let source = open_db(
            &directory,
            &format!("sfen 9/9/9/9/9/9/9/9/4K4 b - 1\n{}", "5i5h\n".repeat(65536)),
        );
        assert!(
            source
                .write_native_book(directory.path("bad.book"), &options)
                .unwrap_err()
                .to_string()
                .contains("too many candidates")
        );
        let source = open_db(&directory, "sfen invalid sfen\n");
        let error = source.write_native_book(directory.path("bad.book"), &options).unwrap_err();
        assert!(matches!(error, BookError::InvalidData(_)));
        assert!(!directory.path("bad.book").exists());
    }
}

#[test]
fn skipped_row_does_not_count_source_fields() {
    let directory = TestDirectory::new();
    let book = open_db(&directory, "sfen 9/9/9/9/9/9/9/9/4K4 b - 7\n5i5h none 10 2 3\n");
    let options = NativeBookImportOptions::new().with_skip_unencodable_positions(true);
    let path = directory.path("output.book");
    let report = book.write_native_book(&path, &options).unwrap();
    assert_eq!(report.dropped_fields, BTreeMap::from([("position".into(), 1)]));
    let database = BookDatabase::try_from_entries(
        book.iter_entries()
            .unwrap()
            .map(|e| BookDatabase::entry_from_yaneuraou(&e.unwrap()).unwrap())
            .collect(),
    )
    .unwrap();
    assert_eq!(
        database.write_native_book(directory.path("strict.book"), &options).unwrap(),
        report
    );
}

#[test]
fn skipped_position_permission_does_not_allow_stored_field_loss() {
    let directory = TestDirectory::new();
    let sfen = board::hirate_position().to_sfen(None);
    let source = open_db(
        &directory,
        &format!("sfen 9/9/9/9/9/9/9/9/4K4 b - 7\n5i5h\nsfen {sfen}\n7g7f none 20 3 5\n"),
    );
    let options = NativeBookImportOptions::new().with_skip_unencodable_positions(true);
    let path = directory.path("output.book");
    let error = source.write_native_book(&path, &options).unwrap_err();
    assert!(error.to_string().contains("allow_lossy"));
    assert!(!path.exists());
    let report = source.write_native_book(&path, &options.with_allow_lossy(true)).unwrap();
    assert_eq!((report.positions, report.skipped_positions), (1, 1));
    assert_eq!(
        report.dropped_fields,
        BTreeMap::from([
            ("position".into(), 1),
            ("position.original_ply".into(), 1),
            ("candidate.count".into(), 1),
        ])
    );
}

#[test]
fn db2016_colliding_positions_are_errors_with_or_without_skipping() {
    for options in options_variants() {
        let directory = TestDirectory::new();
        let sfen = board::hirate_position().to_sfen(None);
        let sfen = sfen.rsplit_once(' ').unwrap().0;
        let source = open_db(&directory, &format!("sfen {sfen}\n7g7f\nsfen {sfen}\n7g7f\n"));
        let error = source.write_native_book(directory.path("bad.book"), &options).unwrap_err();
        assert!(matches!(error, BookError::InvalidData(_)));
        assert!(error.to_string().contains("duplicate native input position: key="));
        assert!(!directory.path("bad.book").exists());
    }
}

#[test]
fn strict_loss_stops_before_later_input_and_before_builder_push() {
    let directory = TestDirectory::new();
    let sfen = board::hirate_position().to_sfen(Some(12));
    let source = open_db(&directory, &format!("sfen {sfen}\n7g7f\nsfen invalid later input\n"));
    let error = source
        .write_native_book(directory.path("bad.book"), &NativeBookImportOptions::new())
        .unwrap_err();
    assert!(matches!(error, BookError::InvalidData(_)));
    let message = error.to_string();
    assert!(message.contains("input 0"));
    assert!(message.contains(sfen.rsplit_once(' ').unwrap().0));
    assert!(message.contains("position.original_ply"));
    assert!(!message.contains("later input"));
    assert_eq!(message.matches("invalid data:").count(), 1);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);

    let options = NativeBookImportOptions::new();
    let mut import =
        Import::new(&directory.path("bad.book"), NativeBookMetadata::default(), &options).unwrap();
    let entry = database(vec![candidate("7g7f", None, Some(1))], BookEntryMetadata::new());
    import
        .entry(0, &entry.entries()[0], *board::hirate_position().to_packed_sfen().as_bytes())
        .unwrap_err();
    assert_eq!(import.report.positions, 0);
    assert_eq!(import.report.candidates, 0);
}

#[test]
fn builder_input_failure_has_row_context_without_nested_error_prefixes() {
    let directory = TestDirectory::new();
    let options = NativeBookImportOptions::new().with_memory_budget(1024 * 1024);
    let path = directory.path("bad.book");
    let mut import = Import::new(&path, NativeBookMetadata::default(), &options).unwrap();
    let position = board::hirate_position();
    let record = NativeBookPosition {
        entry: NativeBookEntry::from_position(&position, vec![]).unwrap(),
        comment: Some("x".repeat(256 * 1024)),
        move_comments: BTreeMap::new(),
    };
    let sfen = position.to_sfen(None);
    let error = import.push(7, || sfen.clone(), record).unwrap_err();
    assert!(matches!(error, BookError::InvalidData(_)));
    let message = error.to_string();
    assert!(message.contains("input 7"));
    assert!(message.contains(&sfen));
    assert!(message.contains("memory budget"));
    assert_eq!(message.matches("invalid data:").count(), 1);
    drop(import);
    assert!(!path.exists());
}

#[test]
fn sbk_description_can_be_explicitly_removed() {
    let directory = TestDirectory::new();
    let source = sbk_source(&directory, &[vec![8, 0]]);
    let options =
        NativeBookImportOptions::new().with_allow_lossy(true).with_metadata(NativeBookMetadata {
            description: Some(String::new()),
            ..NativeBookMetadata::default()
        });
    let path = directory.path("output.book");
    source.write_native_book(&path, &options).unwrap();
    assert_eq!(NativeBook::open(path).unwrap().metadata().unwrap().description, None);
}

#[test]
fn current_record_errors_keep_lazy_sfen_context_after_issue_limit() {
    let directory = TestDirectory::new();
    let options = NativeBookImportOptions::new().with_allow_lossy(true);
    let path = directory.path("bad.book");
    let mut import = Import::new(&path, NativeBookMetadata::default(), &options).unwrap();
    let position = board::hirate_position();
    let sfen = position.to_sfen(None);
    for index in 0..ISSUE_EXAMPLES {
        import.loss(index as u64, Some(&sfen), "ybb.ply", "source move number").unwrap();
    }
    assert_eq!(import.report.issues.len(), ISSUE_EXAMPLES);
    let mut record =
        NativeBookPosition::new(NativeBookEntry::from_position(&position, vec![]).unwrap());
    import.push(32, || panic!("successful push must not format SFEN"), record.clone()).unwrap();
    record.move_comments.insert(Move::from_usi("7g7f").unwrap().raw(), "absent move".into());
    let error = import.push(33, || sfen.clone(), record).unwrap_err();
    assert!(matches!(error, BookError::InvalidData(_)));
    let message = error.to_string();
    assert!(message.contains("input 33"));
    assert!(message.contains(&sfen));
    assert_eq!(message.matches("invalid data:").count(), 1);
    drop(import);
    assert!(!path.exists());
}

fn write_ybb(directory: &TestDirectory, entries: &[YbbEntry], depth: bool) -> std::path::PathBuf {
    let path = directory.path("source.ybb");
    let mut writer = YbbWriter::new(
        std::io::Cursor::new(Vec::new()),
        entries.len() as u64,
        YbbWriteOptions::new().with_depth(depth),
    )
    .unwrap();
    for entry in entries {
        writer.write_entry(entry).unwrap();
    }
    fs::write(&path, writer.finish().unwrap().into_inner()).unwrap();
    path
}

#[test]
fn ybb_conversion_preserves_values_across_storage_and_depth_modes() {
    for storage in [YbbStorageMode::Memory, YbbStorageMode::File] {
        for depth in [None, Some(0), Some(u16::MAX)] {
            let directory = TestDirectory::new();
            let entries = [YbbEntry::new(
                board::hirate_position().to_packed_sfen(),
                13,
                vec![
                    YbbMove::new(Move::from_usi("2g2f").unwrap(), i16::MIN, depth).unwrap(),
                    YbbMove::new(Move::from_usi("7g7f").unwrap(), i16::MAX, depth).unwrap(),
                ],
            )
            .unwrap()];
            let path = write_ybb(&directory, &entries, depth.is_some());
            let source = YbbBook::open_with_options(
                path,
                YbbBookOpenOptions::new()
                    .with_storage_mode(storage)
                    .with_validation_mode(YbbValidationMode::OnAccess),
            )
            .unwrap();
            assert!(
                source
                    .write_native_book(
                        directory.path("strict.book"),
                        &NativeBookImportOptions::new()
                    )
                    .is_err()
            );
            assert!(!directory.path("strict.book").exists());
            let options = NativeBookImportOptions::new().with_allow_lossy(true).with_depth_kind(2);
            let path = directory.path("output.book");
            let report = source.write_native_book(&path, &options).unwrap();
            assert_eq!((report.positions, report.candidates), (1, 2));
            assert_eq!(report.dropped_fields, BTreeMap::from([("ybb.ply".into(), 1)]));
            let native = NativeBook::open(path).unwrap();
            native.validate_full().unwrap();
            assert_eq!(native.metadata().unwrap().depth_kind, 2);
            let record = native.get(&board::hirate_position()).unwrap().unwrap();
            for (rank, original) in entries[0].moves().iter().enumerate() {
                let actual = record.candidates.iter().find(|c| c.mv == original.mv()).unwrap();
                assert_eq!(
                    actual.score,
                    NativeScore::Cp {
                        value: i32::from(original.eval()),
                        bound: NativeBound::Exact
                    }
                );
                assert_eq!(actual.depth, depth.map(u32::from));
                assert_eq!(actual.display_order, rank as u16);
            }
        }
    }
}

#[test]
fn ybb_import_keeps_reverse_display_order_in_canonical_storage() {
    for storage in [YbbStorageMode::Memory, YbbStorageMode::File] {
        let directory = TestDirectory::new();
        let mut moves = vec![
            YbbMove::new(Move::from_usi("7g7f").unwrap(), 70, None).unwrap(),
            YbbMove::new(Move::from_usi("2g2f").unwrap(), 20, None).unwrap(),
        ];
        moves.sort_by_key(|candidate| std::cmp::Reverse(candidate.mv().raw()));
        let entries = [YbbEntry::new(board::hirate_position().to_packed_sfen(), 1, moves).unwrap()];
        let path = write_ybb(&directory, &entries, false);
        let source = YbbBook::open_with_options(
            path,
            YbbBookOpenOptions::new()
                .with_storage_mode(storage)
                .with_validation_mode(YbbValidationMode::OnAccess),
        )
        .unwrap();
        let output = directory.path("output.book");
        source
            .write_native_book(&output, &NativeBookImportOptions::new().with_allow_lossy(true))
            .unwrap();
        let native = NativeBook::open(output).unwrap();
        native.validate_full().unwrap();
        let entry = native.get(&board::hirate_position()).unwrap().unwrap();
        assert!(entry.candidates[0].mv.raw() < entry.candidates[1].mv.raw());
        for (rank, original) in entries[0].moves().iter().enumerate() {
            let actual =
                entry.candidates.iter().find(|candidate| candidate.mv == original.mv()).unwrap();
            assert_eq!(actual.display_order, rank as u16);
            assert_eq!(
                actual.score,
                NativeScore::Cp { value: i32::from(original.eval()), bound: NativeBound::Exact }
            );
        }
    }
}

#[test]
fn ybb_invalid_keys_moves_and_colliding_ply_rows_never_skip() {
    let position = board::hirate_position();
    let mv = YbbMove::new(Move::from_usi("7g7f").unwrap(), 0, None).unwrap();
    let kings = Position::from_sfen("4k4/9/9/9/9/9/9/9/4K4 b Pp 1").unwrap();
    let mut noncanonical = *kings.to_packed_sfen().as_bytes();
    // 盤上の 94 bit に続く先後の歩の持ち駒符号を入れ替え、同じ持ち駒を表す。
    noncanonical[12] ^= 0b0000_1001;
    let key = crate::board::PackedSfen::from_bytes(noncanonical);
    let mut decoded = Position::empty();
    decoded.set_packed_sfen(&key, false, 1).unwrap();
    assert_eq!(decoded.to_sfen(None), kings.to_sfen(None));
    assert_ne!(decoded.to_packed_sfen(), key);
    let cases = [
        vec![YbbEntry::new(crate::board::PackedSfen::from_bytes([0; 32]), 1, vec![]).unwrap()],
        vec![YbbEntry::new(key, 1, vec![]).unwrap()],
        vec![
            YbbEntry::new(
                position.to_packed_sfen(),
                1,
                vec![YbbMove::new(Move::from_usi("7g7e").unwrap(), 0, None).unwrap()],
            )
            .unwrap(),
        ],
        vec![YbbEntry::new(position.to_packed_sfen(), 1, vec![mv, mv]).unwrap()],
        vec![
            YbbEntry::new(position.to_packed_sfen(), 1, vec![mv]).unwrap(),
            YbbEntry::new(position.to_packed_sfen(), 2, vec![mv]).unwrap(),
        ],
    ];
    for entries in cases {
        for options in options_variants() {
            for storage in [YbbStorageMode::Memory, YbbStorageMode::File] {
                let directory = TestDirectory::new();
                let path = write_ybb(&directory, &entries, false);
                let source = YbbBook::open_with_options(
                    path,
                    YbbBookOpenOptions::new()
                        .with_storage_mode(storage)
                        .with_validation_mode(YbbValidationMode::OnAccess),
                )
                .unwrap();
                assert!(source.write_native_book(directory.path("bad.book"), &options).is_err());
                assert!(!directory.path("bad.book").exists());
            }
        }
    }
}

fn protobuf_varint(mut value: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    while value >= 128 {
        bytes.push(value as u8 | 128);
        value >>= 7;
    }
    bytes.push(value as u8);
    bytes
}

fn protobuf_message(field: u8, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![field << 3 | 2];
    bytes.extend(protobuf_varint(payload.len() as u64));
    bytes.extend(payload);
    bytes
}

fn sbk_source(directory: &TestDirectory, states: &[Vec<u8>]) -> SbkBook {
    let mut bytes = protobuf_message(2, b"source description");
    for state in states {
        bytes.extend(protobuf_message(3, state));
    }
    let path = directory.path("source.sbk");
    fs::write(&path, bytes).unwrap();
    SbkBook::open(path).unwrap()
}

#[test]
fn sbk_duplicate_diagnostics_are_always_errors() {
    for same_moves in [false, true] {
        for options in options_variants() {
            let directory = TestDirectory::new();
            let mut root = vec![8, 0];
            let mut candidate = vec![8];
            candidate.extend(protobuf_varint(0x0100_7677));
            root.extend(protobuf_message(9, &candidate));
            let mut duplicate = if same_moves { root.clone() } else { vec![8, 0] };
            duplicate[1] = 1;
            duplicate
                .extend(protobuf_message(7, board::hirate_position().to_sfen(None).as_bytes()));
            let source = sbk_source(&directory, &[root, duplicate]);
            assert_eq!(source.diagnostics().duplicate_positions(), 1);
            assert!(
                source
                    .write_native_book(directory.path("bad.book"), &options)
                    .unwrap_err()
                    .to_string()
                    .contains("duplicate SBK positions")
            );
            assert!(!directory.path("bad.book").exists());
        }
    }
}

#[test]
fn sbk_unresolved_states_and_description_override() {
    for options in options_variants() {
        let directory = TestDirectory::new();
        let source = sbk_source(&directory, &[vec![8, 0], vec![8, 1]]);
        assert_eq!(source.diagnostics().unresolved_next_states(), 1);
        let result = source.write_native_book(directory.path("output.book"), &options);
        if options.allow_lossy {
            let report = result.unwrap();
            assert_eq!(report.skipped_positions, 1);
            assert_eq!(report.dropped_fields.get("sbk.unresolved_state"), Some(&1));
            let issue = report.issues.iter().find(|i| i.field == "sbk.unresolved_state").unwrap();
            assert_eq!((issue.input_index, &issue.sfen), (0, &None));
            assert_eq!(
                NativeBook::open(directory.path("output.book"))
                    .unwrap()
                    .metadata()
                    .unwrap()
                    .description
                    .as_deref(),
                Some("source description")
            );
            let options = options.with_metadata(NativeBookMetadata {
                title: Some("title".into()),
                description: Some("override".into()),
                depth_kind: 2,
            });
            source.write_native_book(directory.path("strict.book"), &options).unwrap();
            assert_eq!(
                NativeBook::open(directory.path("strict.book")).unwrap().metadata().unwrap(),
                options.metadata
            );
        } else {
            assert!(result.is_err());
        }
    }
}

#[test]
fn builder_options_validate_before_input_and_existing_destination_is_preserved() {
    let directory = TestDirectory::new();
    let book = open_db(&directory, "sfen invalid input\n");
    for options in [
        NativeBookImportOptions::new().with_memory_budget(0),
        NativeBookImportOptions::new().with_block_positions(0),
        NativeBookImportOptions::new().with_block_positions(16385),
    ] {
        let error = book.write_native_book(directory.path("bad.book"), &options).unwrap_err();
        assert!(matches!(error, BookError::InvalidData(_)));
        assert!(!error.to_string().contains("SFEN"));
        assert!(!error.to_string().contains("invalid book format"));
        assert_eq!(error.to_string().matches("invalid data:").count(), 1);
    }
    let database = database(vec![candidate("7g7f", None, None)], BookEntryMetadata::new());
    let path = directory.path("output.book");
    let options =
        NativeBookImportOptions::new().with_memory_budget(1024 * 1024).with_block_positions(16);
    database.write_native_book(&path, &options).unwrap();
    let expected = fs::read(&path).unwrap();
    assert_eq!(NativeBook::open(&path).unwrap().block_positions(), 16);
    let error = database.write_native_book(&path, &options).unwrap_err();
    assert!(
        matches!(error, BookError::Io(ref error) if error.kind() == std::io::ErrorKind::AlreadyExists)
    );
    assert_eq!(fs::read(&path).unwrap(), expected);
}

#[test]
fn import_preserves_publication_errors_and_complete_output() {
    for phase in ["written", "flushed", "synced", "validated", "published", "conflict"] {
        let directory = TestDirectory::new();
        let path = directory.path("output.book");
        let options = NativeBookImportOptions::new();
        let mut import = Import::new(&path, NativeBookMetadata::default(), &options).unwrap();
        let entry = database(vec![], BookEntryMetadata::new());
        import
            .entry(0, &entry.entries()[0], *board::hirate_position().to_packed_sfen().as_bytes())
            .unwrap();
        import.builder.checkpoint = match phase {
            "written" => |p| fail_checkpoint(p, "written"),
            "flushed" => |p| fail_checkpoint(p, "flushed"),
            "synced" => |p| fail_checkpoint(p, "synced"),
            "validated" => |p| fail_checkpoint(p, "validated"),
            "published" => |p| fail_checkpoint(p, "published"),
            _ => |_| Ok(()),
        };
        if phase == "conflict" {
            fs::write(&path, b"other writer").unwrap();
        }
        let error = import.finish().unwrap_err();
        if phase == "published" {
            assert!(matches!(error, BookError::SaveUncertain(_)));
            NativeBook::open(&path).unwrap().validate_full().unwrap();
        } else if phase == "conflict" {
            assert!(
                matches!(error, BookError::Io(ref error) if error.kind() == std::io::ErrorKind::AlreadyExists)
            );
            assert_eq!(fs::read(&path).unwrap(), b"other writer");
        } else {
            assert!(matches!(error, BookError::Io(_)));
            assert!(!path.exists());
        }
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), usize::from(path.exists()));
    }
}

fn fail_checkpoint(phase: &str, fail_at: &str) -> Result<(), BookError> {
    if phase == fail_at {
        Err(std::io::Error::other("injected import publication failure").into())
    } else {
        Ok(())
    }
}
