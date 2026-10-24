#![cfg(feature = "book")]

use std::fs;
use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rsshogi::book::{
    BookCandidate, BookDatabase, BookDatabaseEntry, BookEntryMetadata, BookError, BookMoveMetadata,
    BookPosition, YaneuraOuBook, YaneuraOuDb2016WriteOptions, YaneuraOuDb2016Writer,
};
use rsshogi::types::Move;

const HEADER: &str = "#YANEURAOU-DB2016 1.00\n";
const KINGS: &str = "4k4/9/9/9/9/9/9/9/4K4 b -";
const START: &str = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b -";
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn entry(sfen: &str, ply: Option<u32>, candidates: Vec<BookCandidate>) -> BookDatabaseEntry {
    BookDatabaseEntry::new(
        BookPosition::from_sfen(&format!("{sfen} 1"), ply).unwrap(),
        candidates,
        BookEntryMetadata::from_yaneuraou(99, "entry\nnote".into()),
    )
}

fn candidate(mv: Move, ponder: Option<Move>, score: Option<i32>, comment: &str) -> BookCandidate {
    BookCandidate::new(
        mv,
        ponder,
        score,
        Some(3),
        None,
        Some(4),
        BookMoveMetadata::from_yaneuraou(comment.into()),
    )
}

fn roundtrip(output: &[u8], expected: Option<u32>) {
    let path = std::env::temp_dir().join(format!(
        "rsshogi-db-writer-{}-{}.db",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, output).unwrap();
    let book = YaneuraOuBook::open(&path).unwrap();
    let source = book.iter_entries().unwrap().next().unwrap().unwrap();
    assert_eq!(source.original_ply(), expected);
    assert_eq!(source.min_ply(), expected.unwrap_or(0));
    assert_eq!(
        BookDatabaseEntry::from_yaneuraou(&source).unwrap().position().original_ply(),
        expected
    );
    fs::remove_file(path).unwrap();
}

#[test]
fn exact_ply_output_preserves_absent_zero_positive_and_overrides() {
    for ply in [None, Some(0), Some(17)] {
        for (options, expected) in [
            (YaneuraOuDb2016WriteOptions::new(), ply),
            (YaneuraOuDb2016WriteOptions::new().with_fixed_ply(0), Some(0)),
            (YaneuraOuDb2016WriteOptions::new().with_fixed_ply(8), Some(8)),
            (YaneuraOuDb2016WriteOptions::new().with_fixed_ply(8).with_omitted_ply(), None),
        ] {
            let entry = entry(KINGS, ply, vec![]);
            let suffix = expected.map_or_else(String::new, |value| format!(" {value}"));
            let exact = format!("{HEADER}sfen {KINGS}{suffix}\n# entry\n# note\n");
            let mut writer = YaneuraOuDb2016Writer::new(Vec::new(), options, Some(1)).unwrap();
            writer.write_entry(&entry).unwrap();
            let output = writer.finish().unwrap();
            assert_eq!(output, exact.as_bytes());
            roundtrip(&output, expected);
            let database = BookDatabase::try_from_entries(vec![entry]).unwrap();
            assert_eq!(database.to_yaneuraou_db2016_string(&options).unwrap(), exact);
            let mut direct = Vec::new();
            database.write_yaneuraou_db2016(&mut direct, &options).unwrap();
            assert_eq!(direct, output);
        }
    }
}

#[test]
fn ordering_duplicates_and_count_errors_leave_stream_recoverable() {
    let options = YaneuraOuDb2016WriteOptions::new();
    let low = entry(KINGS, None, vec![]);
    let high = entry(START, None, vec![]);
    let mut writer = YaneuraOuDb2016Writer::new(Vec::new(), options, Some(2)).unwrap();
    writer.write_entry(&low).unwrap();
    assert!(writer.write_entry(&low).is_err());
    writer.write_entry(&high).unwrap();
    assert!(writer.write_entry(&high).is_err());
    let output = writer.finish().unwrap();
    assert_eq!(
        output,
        format!("{HEADER}sfen {KINGS}\n# entry\n# note\nsfen {START}\n# entry\n# note\n")
            .as_bytes()
    );
    let mut writer = YaneuraOuDb2016Writer::new(Vec::new(), options, None).unwrap();
    writer.write_entry(&high).unwrap();
    assert!(writer.write_entry(&low).is_err());
    assert!(writer.finish().is_ok());
    assert!(
        YaneuraOuDb2016Writer::new(Vec::new(), options.with_keep_last_on_duplicate(), None)
            .is_err()
    );
    let database = BookDatabase::try_from_entries(vec![high, low]).unwrap();
    let mut direct = Vec::new();
    database.write_yaneuraou_db2016(&mut direct, &options.with_keep_last_on_duplicate()).unwrap();
    assert_eq!(direct, output);
}

#[test]
fn noe_requires_count_and_count_is_enforced_without_noe() {
    let options = YaneuraOuDb2016WriteOptions::new();
    assert!(YaneuraOuDb2016Writer::new(Vec::new(), options.with_noe(true), None).is_err());
    let empty = YaneuraOuDb2016Writer::new(Vec::new(), options.with_noe(true), Some(0)).unwrap();
    assert_eq!(empty.finish().unwrap(), format!("{HEADER}# NOE:0\n").as_bytes());
    let mut empty = YaneuraOuDb2016Writer::new(Vec::new(), options, Some(0)).unwrap();
    assert!(empty.write_entry(&entry(KINGS, None, vec![])).is_err());
    assert_eq!(empty.finish().unwrap(), HEADER.as_bytes());
    for emit in [false, true] {
        let short =
            YaneuraOuDb2016Writer::new(Vec::new(), options.with_noe(emit), Some(1)).unwrap();
        assert!(short.finish().is_err());
    }
    let mut writer =
        YaneuraOuDb2016Writer::new(Vec::new(), options.with_noe(true), Some(1)).unwrap();
    writer.write_entry(&entry(KINGS, None, vec![])).unwrap();
    assert_eq!(
        writer.finish().unwrap(),
        format!("{HEADER}# NOE:1\nsfen {KINGS}\n# entry\n# note\n").as_bytes()
    );
}

#[test]
fn varying_sfen_lengths_keep_order_checks_and_output_recoverable() {
    let mut entries = vec![
        entry(KINGS, None, vec![]),
        entry(START, Some(1), vec![]),
        entry("s3k4/9/9/9/9/9/9/9/4K4 b -", Some(2), vec![]),
        entry("s3k4/9/9/9/9/9/9/9/4K4 w -", Some(3), vec![]),
    ];
    entries.sort_by(|left, right| left.position().sfen().cmp(right.position().sfen()));
    assert!(entries[1].position().sfen().len() > entries[2].position().sfen().len());
    let options = YaneuraOuDb2016WriteOptions::new().with_preserved_move_order();
    let mut writer = YaneuraOuDb2016Writer::new(Vec::new(), options, Some(4)).unwrap();
    for (idx, entry) in entries[..3].iter().enumerate() {
        writer.write_entry(entry).unwrap();
        assert!(matches!(
            writer.write_entry(entry),
            Err(BookError::InvalidData(message)) if message == "DB2016 positions must be strictly increasing"
        ));
        if idx > 0 {
            assert!(matches!(
                writer.write_entry(&entries[idx - 1]),
                Err(BookError::InvalidData(message)) if message == "DB2016 positions must be strictly increasing"
            ));
        }
    }
    writer.write_entry(&entries[3]).unwrap();
    let output = writer.finish().unwrap();
    let database = BookDatabase::try_from_entries(entries).unwrap();
    assert_eq!(output, database.to_yaneuraou_db2016_string(&options).unwrap().as_bytes());
}

#[test]
fn candidate_order_comments_and_encoding_match_exact_output() {
    let moves = vec![
        candidate(Move::from_usi("7g7f").unwrap(), None, None, "low"),
        candidate(
            Move::from_usi("2g2f").unwrap(),
            Move::from_usi("3c3d"),
            Some(10),
            "high\nsecond",
        ),
        candidate(Move::from_usi("5g5f").unwrap(), None, Some(10), "tie"),
    ];
    let entry = entry(START, Some(0), moves);
    let low = "7g7f none none 3 4\n# low\n";
    let high = "2g2f 3c3d 10 3 4\n# high\n# second\n5g5f none 10 3 4\n# tie\n";
    for preserve in [false, true] {
        let options = if preserve {
            YaneuraOuDb2016WriteOptions::new().with_preserved_move_order()
        } else {
            YaneuraOuDb2016WriteOptions::new()
        };
        let mut writer = YaneuraOuDb2016Writer::new(Vec::new(), options, Some(1)).unwrap();
        writer.write_entry(&entry).unwrap();
        let expected = if preserve { format!("{low}{high}") } else { format!("{high}{low}") };
        assert_eq!(
            writer.finish().unwrap(),
            format!("{HEADER}sfen {START} 0\n# entry\n# note\n{expected}").as_bytes()
        );
    }
}

#[test]
fn invalid_entry_is_atomic_and_database_preflights_before_header() {
    for options in [
        YaneuraOuDb2016WriteOptions::new(),
        YaneuraOuDb2016WriteOptions::new().with_preserved_move_order(),
    ] {
        for (mv, ponder) in
            [(Move::MOVE_RESIGN, None), (Move::from_usi("7g7f").unwrap(), Some(Move::MOVE_NONE))]
        {
            let invalid = entry(
                START,
                None,
                vec![
                    candidate(Move::from_usi("2g2f").unwrap(), None, Some(3), "valid first"),
                    candidate(mv, ponder, Some(2), "bad"),
                ],
            );
            let mut bytes = Vec::new();
            let mut writer = YaneuraOuDb2016Writer::new(&mut bytes, options, Some(1)).unwrap();
            assert!(writer.write_entry(&invalid).is_err());
            writer.write_entry(&entry(START, None, vec![])).unwrap();
            writer.finish().unwrap();
            assert_eq!(bytes, format!("{HEADER}sfen {START}\n# entry\n# note\n").as_bytes());
            let database =
                BookDatabase::try_from_entries(vec![entry(KINGS, None, vec![]), invalid]).unwrap();
            let mut untouched = b"prefix".to_vec();
            assert!(database.write_yaneuraou_db2016(&mut untouched, &options).is_err());
            assert_eq!(untouched, b"prefix");
            assert!(database.to_yaneuraou_db2016_string(&options).is_err());
        }
    }
}

#[derive(Clone, Default)]
struct FailingOutput(Arc<Mutex<OutputState>>);
#[derive(Default)]
struct OutputState {
    bytes: Vec<u8>,
    fail_write: bool,
    fail_flush: bool,
    writes: usize,
    flushes: usize,
}
impl Write for FailingOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut state = self.0.lock().unwrap();
        state.writes += 1;
        if state.fail_write {
            state.bytes.push(bytes[0]);
            return Err(io::Error::other("injected write failure"));
        }
        state.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        let mut state = self.0.lock().unwrap();
        state.flushes += 1;
        if state.fail_flush { Err(io::Error::other("injected flush failure")) } else { Ok(()) }
    }
}

#[test]
fn io_failures_poison_stream_and_finish_flushes() {
    let failed_header = FailingOutput::default();
    failed_header.0.lock().unwrap().fail_write = true;
    assert!(matches!(
        YaneuraOuDb2016Writer::new(failed_header, YaneuraOuDb2016WriteOptions::new(), None,),
        Err(BookError::Io(_))
    ));
    let output = FailingOutput::default();
    let mut writer =
        YaneuraOuDb2016Writer::new(output.clone(), YaneuraOuDb2016WriteOptions::new(), None)
            .unwrap();
    output.0.lock().unwrap().fail_write = true;
    assert!(matches!(writer.write_entry(&entry(KINGS, None, vec![])), Err(BookError::Io(_))));
    let writes = output.0.lock().unwrap().writes;
    output.0.lock().unwrap().fail_write = false;
    assert!(writer.write_entry(&entry(KINGS, None, vec![])).is_err());
    assert!(writer.finish().is_err());
    assert_eq!(output.0.lock().unwrap().writes, writes);
    assert_eq!(output.0.lock().unwrap().flushes, 0);
    let output = FailingOutput::default();
    let writer =
        YaneuraOuDb2016Writer::new(output.clone(), YaneuraOuDb2016WriteOptions::new(), Some(0))
            .unwrap();
    output.0.lock().unwrap().fail_flush = true;
    assert!(matches!(writer.finish(), Err(BookError::Io(_))));
    assert_eq!(output.0.lock().unwrap().flushes, 1);
    output.0.lock().unwrap().fail_flush = false;
    let writer =
        YaneuraOuDb2016Writer::new(output.clone(), YaneuraOuDb2016WriteOptions::new(), Some(0))
            .unwrap();
    assert!(writer.finish().is_ok());
    assert_eq!(output.0.lock().unwrap().flushes, 2);
}

#[test]
fn duplicate_candidates_are_rejected_before_output_and_writer_can_retry() {
    for options in [
        YaneuraOuDb2016WriteOptions::new(),
        YaneuraOuDb2016WriteOptions::new().with_preserved_move_order(),
    ] {
        let mv = Move::from_usi("7g7f").unwrap();
        let invalid = entry(
            START,
            None,
            vec![candidate(mv, None, Some(1), "first"), candidate(mv, None, Some(2), "duplicate")],
        );
        let low = entry(KINGS, None, vec![]);
        let high = entry(START, None, vec![candidate(mv, None, Some(1), "valid")]);
        let mut bytes = Vec::new();
        {
            let mut writer = YaneuraOuDb2016Writer::new(&mut bytes, options, Some(2)).unwrap();
            writer.write_entry(&low).unwrap();
            assert!(
                matches!(writer.write_entry(&invalid), Err(BookError::InvalidData(message)) if message.starts_with("duplicate book database candidate:"))
            );
            writer.write_entry(&high).unwrap();
            writer.finish().unwrap();
        }
        let expected = BookDatabase::try_from_entries(vec![low, high])
            .unwrap()
            .to_yaneuraou_db2016_string(&options)
            .unwrap();
        assert_eq!(bytes, expected.as_bytes());
    }
}
