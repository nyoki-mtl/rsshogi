#![cfg(feature = "book")]

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rsshogi::board::hirate_position;
use rsshogi::book::{
    BookDatabase, YaneuraOuAccessMode, YaneuraOuBook, YaneuraOuBookOpenOptions,
    YaneuraOuDb2016WriteOptions, YbbBook,
};
use rsshogi::types::Move;

fn temp_path(suffix: &str) -> PathBuf {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock after epoch").as_nanos();
    std::env::temp_dir().join(format!("rsshogi-task0026-{}-{nonce}-{suffix}", std::process::id()))
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

    let book = YbbBook::open(&path).expect("open YBB fixture");
    assert_eq!(book.len(), 1);
    assert!(!book.has_depth());
    let entry = book.lookup_position(&position).expect("lookup").expect("entry");
    assert_eq!(entry.packed_sfen(), packed);
    assert_eq!(entry.ply(), position.game_ply());
    assert!(!entry.flipped());
    assert_eq!(entry.moves().len(), 1);
    assert_eq!(entry.moves()[0].mv(), mv);
    assert_eq!(entry.moves()[0].eval(), -25);
    assert_eq!(entry.moves()[0].depth(), None);

    let _ = fs::remove_file(path);
}
