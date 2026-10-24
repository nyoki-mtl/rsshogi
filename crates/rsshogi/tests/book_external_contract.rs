#![cfg(feature = "book")]

use std::fs;
use std::io::{BufWriter, Cursor};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rsshogi::board::hirate_position;
use rsshogi::book::{
    BookDatabase, BookError, DEPTH_KIND_RETROGRADE, NativeBook, NativeBookBuilder, NativeBookEntry,
    NativeBookImportIssue, NativeBookImportOptions, NativeBookImportReport, NativeBookMetadata,
    NativeBookPosition, YaneuraOuAccessMode, YaneuraOuBook, YaneuraOuBookOpenOptions,
    YaneuraOuDb2016WriteOptions, YbbBook, YbbBookOpenOptions, YbbEntry, YbbMove, YbbStorageMode,
    YbbValidationMode, YbbWriteOptions, YbbWriter,
};
use rsshogi::types::Move;

fn temp_path(suffix: &str) -> PathBuf {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock after epoch").as_nanos();
    std::env::temp_dir().join(format!("rsshogi-task0026-{}-{nonce}-{suffix}", std::process::id()))
}

#[test]
fn native_import_options_are_available_to_external_consumers() {
    let directory = temp_path("native-import");
    fs::create_dir(&directory).unwrap();
    let db_path = directory.join("source.db");
    let position = hirate_position();
    fs::write(
        &db_path,
        format!("#YANEURAOU-DB2016 1.00\nsfen {}\n7g7f none -123 0\n", position.to_sfen(None)),
    )
    .unwrap();
    let source = YaneuraOuBook::open(&db_path).unwrap();
    let options = NativeBookImportOptions::new()
        .with_metadata(NativeBookMetadata {
            title: Some("external import".into()),
            ..NativeBookMetadata::default()
        })
        .with_depth_kind(DEPTH_KIND_RETROGRADE)
        .with_allow_lossy(true)
        .with_skip_unencodable_positions(false)
        .with_memory_budget(1024 * 1024)
        .with_block_positions(16);
    let db_output = directory.join("db.book");
    let report = source.write_native_book(&db_output, &options).unwrap();
    assert_eq!(report.dropped_fields.get("position.original_ply"), Some(&1));
    assert!(!report.dropped_fields.contains_key("yaneuraou.min_ply"));

    let ybb_path = directory.join("source.ybb");
    let mut writer =
        YbbWriter::new(Cursor::new(Vec::new()), 1, YbbWriteOptions::new().with_depth(true))
            .unwrap();
    writer
        .write_entry(
            &YbbEntry::new(
                position.to_packed_sfen(),
                1,
                vec![YbbMove::new(Move::from_usi("7g7f").unwrap(), -123, Some(0)).unwrap()],
            )
            .unwrap(),
        )
        .unwrap();
    fs::write(&ybb_path, writer.finish().unwrap().into_inner()).unwrap();
    let ybb = YbbBook::open_with_options(
        &ybb_path,
        YbbBookOpenOptions::new()
            .with_storage_mode(YbbStorageMode::File)
            .with_validation_mode(YbbValidationMode::OnAccess),
    )
    .unwrap();
    let ybb_output = directory.join("ybb.book");
    assert_eq!(
        ybb.write_native_book(&ybb_output, &options).unwrap().dropped_fields.get("ybb.ply"),
        Some(&1)
    );
    let db_native = NativeBook::open(&db_output).unwrap();
    let ybb_native = NativeBook::open(&ybb_output).unwrap();
    db_native.validate_full().unwrap();
    ybb_native.validate_full().unwrap();
    assert_eq!(db_native.metadata().unwrap().depth_kind, DEPTH_KIND_RETROGRADE);
    assert_eq!(db_native.metadata().unwrap().title.as_deref(), Some("external import"));
    assert_eq!(db_native.block_positions(), 16);
    assert_eq!(db_native.get(&position).unwrap(), ybb_native.get(&position).unwrap());
    drop((db_native, ybb_native, ybb));
    for path in [db_path, ybb_path, db_output, ybb_output] {
        fs::remove_file(path).unwrap();
    }
    fs::remove_dir(directory).unwrap();
}

#[test]
fn native_import_report_fields_remain_constructible_and_destructurable() {
    let issue = NativeBookImportIssue {
        input_index: 0,
        sfen: None,
        field: "consumer.unknown_field".into(),
        reason: "diagnostic for display".into(),
    };
    let report = NativeBookImportReport {
        positions: 1,
        candidates: 2,
        skipped_positions: 0,
        dropped_fields: std::collections::BTreeMap::from([(issue.field.clone(), 1)]),
        issues: vec![issue],
        omitted_issues: 0,
    };
    let NativeBookImportReport {
        positions,
        candidates,
        skipped_positions,
        dropped_fields,
        issues,
        omitted_issues,
    } = report.clone();
    let NativeBookImportIssue { input_index, sfen, field, reason } = issues[0].clone();
    assert_eq!((positions, candidates, skipped_positions, omitted_issues), (1, 2, 0, 0));
    assert_eq!((input_index, sfen), (0, None));
    assert_eq!(dropped_fields.get(&field), Some(&1));
    assert!(!reason.is_empty());
    assert_eq!(report, report.clone());
}

#[test]
fn db2016_reader_and_writer_preserve_contract_and_sort_positions() {
    let path = temp_path("book.db");
    let startpos = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
    let kings = "4k4/9/9/9/9/9/9/9/4K4 b - 1";
    let input = format!(
        "#YANEURAOU-DB2016 1.00\n\
         sfen {startpos}\n\
         # start entry\n\
         7g7f none 10 3 2 # first move\n\
         sfen {kings}\n\
         # kings entry\n"
    );
    fs::write(&path, input).expect("write DB2016 fixture");

    let options = YaneuraOuBookOpenOptions::with_access_mode(YaneuraOuAccessMode::ScanOnly);
    let book = YaneuraOuBook::open_with_options(&path, options).expect("open DB2016 fixture");
    let mut entries: Vec<_> = book
        .iter_entries()
        .expect("entry iterator")
        .collect::<Result<_, _>>()
        .expect("parse entries");

    assert_eq!(entries[0].comment(), "start entry");
    assert_eq!(entries[0].moves().len(), 1);
    assert_eq!(entries[0].moves()[0].mv(), Move::from_usi("7g7f").unwrap());
    assert_eq!(entries[0].moves()[0].ponder(), Move::MOVE_NONE);
    assert_eq!(entries[0].moves()[0].score(), Some(10));
    assert_eq!(entries[0].moves()[0].depth(), Some(3));
    assert_eq!(entries[0].moves()[0].count(), Some(2));
    assert_eq!(entries[0].moves()[0].comment(), "first move");

    entries.reverse();
    let owned = entries
        .iter()
        .map(BookDatabase::entry_from_yaneuraou)
        .collect::<Result<Vec<_>, _>>()
        .expect("convert entries");
    let database = BookDatabase::try_from_entries(owned).expect("build database");
    let output = database
        .to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::new())
        .expect("write DB2016");

    assert!(output.starts_with("#YANEURAOU-DB2016 1.00\n"));
    let kings_offset = output.find(&format!("sfen {kings}")).expect("kings row");
    let start_offset = output.find(&format!("sfen {startpos}")).expect("start row");
    assert!(kings_offset < start_offset, "normalized SFEN rows must be sorted");
    assert!(output.contains("# start entry"));
    assert!(output.contains("7g7f none 10 3 2"));
    assert!(output.contains("# first move"));

    let _ = fs::remove_file(path);
}

#[test]
fn db2016_bad_entry_does_not_hide_later_entries() {
    let path = temp_path("recoverable-entry.db");
    let bad = "4k4/9/9/9/9/9/9/9/4K4 b - 1";
    let good = "4k4/9/9/9/9/9/9/4P4/4K4 b - 1";
    let input =
        format!("#YANEURAOU-DB2016 1.00\nsfen {bad}\nK*5e none 0 0\nsfen {good}\n5h5g none 1 2\n");
    fs::write(&path, input).expect("write DB2016 fixture");

    let options = YaneuraOuBookOpenOptions::with_access_mode(YaneuraOuAccessMode::ScanOnly);
    let book = YaneuraOuBook::open_with_options(&path, options).expect("open structural book");
    let mut entries = book.iter_entries().expect("entry iterator");
    assert!(entries.next().expect("bad entry result").is_err());
    assert_eq!(entries.next().expect("good entry result").expect("good entry").sfen(), good);
    assert!(book.lookup_sfen(bad).is_err());
    assert_eq!(book.lookup_sfen(good).expect("lookup later entry").expect("entry").sfen(), good);

    let _ = fs::remove_file(path);
}

#[test]
fn db2016_resign_and_omitted_fields_keep_legacy_meaning() {
    let path = temp_path("omitted-fields.db");
    let startpos = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b -";
    let input =
        format!("#YANEURAOU-DB2016 1.00\nsfen {startpos}\nresign none 0 0\n7g7f none 10 3\n");
    fs::write(&path, input).expect("write DB2016 fixture");

    let options = YaneuraOuBookOpenOptions::with_access_mode(YaneuraOuAccessMode::ScanOnly);
    let book = YaneuraOuBook::open_with_options(&path, options).expect("open DB2016 fixture");
    let entry = book.iter_entries().expect("iterator").next().expect("entry").expect("valid entry");
    assert_eq!(entry.min_ply(), 0);
    assert_eq!(entry.moves()[0].mv(), Move::MOVE_RESIGN);
    assert_eq!(entry.moves()[1].count(), None);

    let database = BookDatabase::try_from_entries(vec![
        BookDatabase::entry_from_yaneuraou(&entry).expect("convert entry"),
    ])
    .expect("database");
    let output = database
        .to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::new())
        .expect_err("MOVE_NONE is not writable as a candidate");
    assert!(output.to_string().contains("invalid DB2016 move"));

    let path_without_resign = temp_path("omitted-fields-writable.db");
    let input = format!("#YANEURAOU-DB2016 1.00\nsfen {startpos}\n7g7f none 10 3\n");
    fs::write(&path_without_resign, input).expect("write writable fixture");
    let book = YaneuraOuBook::open_with_options(&path_without_resign, options).expect("open book");
    let entry = book.iter_entries().unwrap().next().unwrap().unwrap();
    let database =
        BookDatabase::try_from_entries(vec![BookDatabase::entry_from_yaneuraou(&entry).unwrap()])
            .unwrap();
    let output = database
        .to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::new())
        .expect("write DB2016");
    assert!(output.contains(&format!("sfen {startpos}\n")));
    assert!(!output.contains(&format!("sfen {startpos} 0\n")));
    assert!(output.contains("7g7f none 10 3\n"));
    assert!(!output.contains("7g7f none 10 3 none"));

    let _ = fs::remove_file(path);
    let _ = fs::remove_file(path_without_resign);
}

#[test]
fn db2016_binary_mode_searches_the_file_without_materializing_it() {
    let path = temp_path("binary-search.db");
    let earlier = "4k4/9/9/9/9/9/9/9/4K4 b - 1";
    let target = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
    let padding = "x".repeat(96 * 1024);
    let input = format!(
        "#YANEURAOU-DB2016 1.00\nsfen {earlier}\n# {padding}\nsfen {target}\n7g7f none 1 1\n"
    );
    fs::write(&path, input).expect("write DB2016 fixture");

    let options =
        YaneuraOuBookOpenOptions::with_access_mode(YaneuraOuAccessMode::AssumeSortedByCaller);
    let book = YaneuraOuBook::open_with_options(&path, options).expect("open DB2016 fixture");
    assert_eq!(book.lookup_sfen(earlier).expect("earlier lookup").expect("entry").sfen(), earlier);
    let entry = book.lookup_sfen(target).expect("target lookup").expect("entry");
    assert_eq!(entry.moves()[0].mv(), Move::from_usi("7g7f").unwrap());

    let _ = fs::remove_file(path);
}

#[test]
fn db2016_binary_mode_accepts_bom_without_header() {
    let path = temp_path("bom-headerless.db");
    let position = "4k4/9/9/9/9/9/9/9/4K4 b - 1";
    fs::write(&path, format!("\u{feff}sfen {position}\n5i5h none 1 1\n"))
        .expect("write DB2016 fixture");

    let options =
        YaneuraOuBookOpenOptions::with_access_mode(YaneuraOuAccessMode::AssumeSortedByCaller);
    let book = YaneuraOuBook::open_with_options(&path, options).expect("open DB2016 fixture");
    let entry = book.lookup_sfen(position).expect("lookup").expect("entry");
    assert_eq!(entry.sfen(), position);

    let _ = fs::remove_file(path);
}

#[test]
fn ybb_exact_single_record_layout_is_readable() {
    let path = temp_path("single.ybb");
    let position = hirate_position();
    let packed = position.to_packed_sfen();
    let mv = Move::from_usi("7g7f").expect("move");

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"YANE-BINBOOK-V1\0");
    bytes.extend_from_slice(&1u64.to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes());
    bytes.extend_from_slice(packed.as_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes());
    bytes.extend_from_slice(&position.game_ply().to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&mv.raw().to_le_bytes());
    bytes.extend_from_slice(&(-25i16).to_le_bytes());
    fs::write(&path, bytes).expect("write YBB fixture");

    for storage_mode in [YbbStorageMode::Memory, YbbStorageMode::File] {
        for validation_mode in [YbbValidationMode::Full, YbbValidationMode::OnAccess] {
            let options = YbbBookOpenOptions::new()
                .with_storage_mode(storage_mode)
                .with_validation_mode(validation_mode);
            let book = YbbBook::open_with_options(&path, options).expect("open YBB fixture");
            assert_eq!(book.options(), options);
            assert_eq!(book.len(), 1);
            assert!(!book.has_depth());
            book.validate_full().expect("validate YBB fixture");
            let entry = book.lookup_position(&position).expect("lookup").expect("entry");
            assert_eq!(entry.packed_sfen(), packed);
            assert_eq!(entry.ply(), position.game_ply());
            assert!(!entry.flipped());
            assert_eq!(entry.moves().len(), 1);
            assert_eq!(entry.moves()[0].mv(), mv);
            assert_eq!(entry.moves()[0].eval(), -25);
            assert_eq!(entry.moves()[0].depth(), None);
        }
    }

    let _ = fs::remove_file(path);
}

#[test]
fn db2016_to_ybb_and_buffered_file_copy_preserve_candidate_fields() {
    let source = temp_path("export.db");
    let destination = temp_path("copy.ybb");
    let position = hirate_position();
    fs::write(
        &source,
        format!(
            "#YANEURAOU-DB2016 1.00\nsfen {}\n7g7f none -25 12 0\n2g2f none 30 0 2\n",
            position.to_sfen(Some(1)),
        ),
    )
    .unwrap();
    let book = YaneuraOuBook::open(&source).unwrap();
    let entry = book.lookup_position(&position).unwrap().unwrap();
    let database =
        BookDatabase::try_from_entries(vec![BookDatabase::entry_from_yaneuraou(&entry).unwrap()])
            .unwrap();
    let mut output = Cursor::new(Vec::new());
    let options = YbbWriteOptions::new().with_depth(true);
    assert!(database.write_ybb(&mut output, &options).is_err());
    assert!(output.get_ref().is_empty());
    database.write_ybb(&mut output, &options.with_allow_lossy(true)).unwrap();
    let original = output.into_inner();
    fs::write(&destination, &original).unwrap();
    let ybb = YbbBook::open(&destination).unwrap();
    let copy = temp_path("buffered-copy.ybb");
    let file = fs::OpenOptions::new().write(true).create_new(true).open(&copy).unwrap();
    let mut writer = YbbWriter::new(BufWriter::new(file), ybb.len(), options).unwrap();
    for entry in ybb.iter_entries() {
        writer.write_entry(&entry.unwrap()).unwrap();
    }
    drop(writer.finish().unwrap());
    assert_eq!(fs::read(&copy).unwrap(), original);
    let copied = YbbBook::open(&copy).unwrap().lookup_position(&position).unwrap().unwrap();
    assert_eq!(copied.moves().len(), 2);
    for (actual, source) in copied.moves().iter().zip(entry.moves()) {
        assert_eq!(actual.mv(), source.mv());
        assert_eq!(i32::from(actual.eval()), source.score().unwrap());
        assert_eq!(actual.depth().map(u32::from), source.depth());
    }
    for path in [source, destination, copy] {
        fs::remove_file(path).unwrap();
    }
}

#[test]
fn native_import_error_variants_and_strict_stopping_are_public_contracts() {
    let directory = temp_path("native-import-errors");
    fs::create_dir(&directory).unwrap();
    let input = directory.join("source.db");
    let output = directory.join("output.book");
    let sfen = hirate_position().to_sfen(None);
    let identity = sfen.rsplit_once(' ').unwrap().0;
    let open = || {
        YaneuraOuBook::open_with_options(
            &input,
            YaneuraOuBookOpenOptions::with_access_mode(YaneuraOuAccessMode::AssumeSortedByCaller),
        )
        .unwrap()
    };
    let options = NativeBookImportOptions::new();
    fs::write(&input, format!("sfen {identity}\n7g7f\n")).unwrap();
    let source = open();
    for invalid in [options.clone().with_memory_budget(0), options.clone().with_block_positions(0)]
    {
        let error = source.write_native_book(&output, &invalid).unwrap_err();
        assert!(matches!(error, BookError::InvalidData(_)));
        assert_eq!(error.to_string().matches("invalid data:").count(), 1);
        assert!(!error.to_string().contains("invalid book format"));
    }
    source.write_native_book(&output, &options).unwrap();
    let expected = fs::read(&output).unwrap();
    let error = source.write_native_book(&output, &options).unwrap_err();
    assert!(
        matches!(error, BookError::Io(error) if error.kind() == std::io::ErrorKind::AlreadyExists)
    );
    assert_eq!(fs::read(&output).unwrap(), expected);
    drop(source);
    fs::remove_file(&output).unwrap();

    for (first_ply, second_ply) in [("", ""), (" 1", " 99")] {
        fs::write(
            &input,
            format!("sfen {identity}{first_ply}\n7g7f\nsfen {identity}{second_ply}\n7g7f\n"),
        )
        .unwrap();
        let source = open();
        let error =
            source.write_native_book(&output, &options.clone().with_allow_lossy(true)).unwrap_err();
        assert!(matches!(error, BookError::InvalidData(_)));
        assert!(error.to_string().contains("duplicate native input position: key="));
        assert!(!output.exists());
    }
    fs::write(&input, format!("sfen {identity} 12\n7g7f\nsfen invalid later input\n")).unwrap();
    let source = open();
    let error = source.write_native_book(&output, &options).unwrap_err();
    assert!(matches!(error, BookError::InvalidData(_)));
    let message = error.to_string();
    assert!(message.contains("input 0"));
    assert!(message.contains("position.original_ply"));
    assert!(!message.contains("later input"));
    assert!(!output.exists());
    drop(source);
    fs::remove_file(input).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[test]
fn native_builder_error_variants_are_public_contracts() {
    let directory = temp_path("native-builder-errors");
    fs::create_dir(&directory).unwrap();
    let path = directory.join("output.book");
    let builder = || NativeBookBuilder::new(&path, NativeBookMetadata::default()).unwrap();
    let error = builder().with_memory_budget(0).err().unwrap();
    assert!(matches!(error, BookError::InvalidData(_)));
    for count in [0, 16385] {
        let error = builder().with_block_positions(count).err().unwrap();
        assert!(matches!(error, BookError::InvalidData(_)));
    }
    let position = hirate_position();
    let record =
        NativeBookPosition::new(NativeBookEntry::from_position(&position, vec![]).unwrap());
    let mut after_input = builder();
    after_input.push(record.clone()).unwrap();
    let error = after_input.with_memory_budget(1024 * 1024).err().unwrap();
    assert!(matches!(error, BookError::InvalidData(_)));
    let mut after_input = builder();
    after_input.push(record.clone()).unwrap();
    let error = after_input.with_block_positions(64).err().unwrap();
    assert!(matches!(error, BookError::InvalidData(_)));
    let mut duplicate = builder();
    duplicate.push(record.clone()).unwrap();
    duplicate.push(record.clone()).unwrap();
    let error = duplicate.finish().unwrap_err();
    assert!(matches!(error, BookError::InvalidData(_)));
    assert!(error.to_string().contains(&position.to_sfen(None)));
    assert!(!path.exists());
    let mut limited = builder().with_memory_budget(1024 * 1024).unwrap();
    let mut large = record;
    large.comment = Some("x".repeat(256 * 1024));
    let error = limited.push(large).unwrap_err();
    assert!(matches!(error, BookError::InvalidData(_)));
    drop(limited);
    assert!(!path.exists());
    builder().finish().unwrap();
    let before = fs::read(&path).unwrap();
    let error = NativeBookBuilder::new(&path, NativeBookMetadata::default()).err().unwrap();
    assert!(
        matches!(error, BookError::Io(error) if error.kind() == std::io::ErrorKind::AlreadyExists)
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::remove_file(path).unwrap();
    fs::remove_dir(directory).unwrap();
}
