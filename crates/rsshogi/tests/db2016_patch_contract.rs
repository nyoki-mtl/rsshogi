#![cfg(feature = "book")]

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use rsshogi::book::{
    BookCandidate, BookControl, BookDatabaseEdit, BookDatabaseEntry, BookDatabasePatch,
    BookEntryMetadata, BookError, BookMoveMetadata, BookPatchWritePhase, BookPosition,
    YaneuraOuAccessMode, YaneuraOuBook, YaneuraOuBookOpenOptions,
};
use rsshogi::types::Move;

const HEADER: &str = "#YANEURAOU-DB2016 1.00\n";
const A: &str = "4k4/9/9/9/9/9/9/9/4K4 b -";
const B: &str = "4k4/9/9/9/9/9/9/9/4K4 w -";
const C: &str = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b -";
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Source(PathBuf);

impl Source {
    fn new(body: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "rsshogi-db-patch-{}-{}.db",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, format!("{HEADER}{body}")).unwrap();
        Self(path)
    }

    fn open(&self) -> YaneuraOuBook {
        YaneuraOuBook::open_with_options(
            &self.0,
            YaneuraOuBookOpenOptions::with_access_mode(YaneuraOuAccessMode::AssumeSortedByCaller),
        )
        .unwrap()
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        fs::remove_file(&self.0).unwrap();
    }
}

fn entry(sfen: &str, ply: Option<u32>, moves: Vec<BookCandidate>) -> BookDatabaseEntry {
    BookDatabaseEntry::new(
        BookPosition::from_sfen(&format!("{sfen} 1"), ply).unwrap(),
        moves,
        BookEntryMetadata::default(),
    )
}

fn candidate(mv: Move, weight: Option<i32>) -> BookCandidate {
    BookCandidate::new(mv, None, None, None, weight, None, BookMoveMetadata::default())
}

fn save(book: &YaneuraOuBook, patch: &BookDatabasePatch) -> Vec<u8> {
    let mut output = Vec::new();
    book.write_yaneuraou_db2016_with_patch(&mut output, patch).unwrap();
    output
}

#[test]
fn unchanged_preserves_order_optional_values_comments_and_preamble() {
    let source = Source::new(&format!(
        "# NOE:2\n// front\n# second\nsfen {A} 0\n# position\n5i4i none none 0 none // first\n# continued\n5i6i 5a4a 99 none 0 # second move\nsfen {C}\n7g7f none none none none\n"
    ));
    let expected = format!(
        "{HEADER}# front\n# second\nsfen {A} 0\n# position\n5i4i none none 0\n# first\n# continued\n5i6i 5a4a 99 none 0\n# second move\nsfen {C}\n7g7f none none none\n"
    );
    let book = source.open();
    let patch = BookDatabasePatch::new();
    assert_eq!(save(&book, &patch), expected.as_bytes());
    assert_eq!(save(&book, &patch), expected.as_bytes());
}

#[test]
fn patch_comments_normalize_crlf_and_cr_without_creating_data_rows() {
    let source = Source::new(&format!("sfen {C}\n"));
    let candidate = BookCandidate::new(
        Move::from_usi("7g7f").unwrap(),
        None,
        None,
        None,
        None,
        None,
        BookMoveMetadata::from_yaneuraou("move\r\nsfen injected\r# note\r".into()),
    );
    let mut patch = BookDatabasePatch::new();
    patch
        .upsert_entry(BookDatabaseEntry::new(
            BookPosition::from_sfen(&format!("{C} 1"), None).unwrap(),
            vec![candidate],
            BookEntryMetadata::from_yaneuraou(0, "position\r\nline2\rsfen injected\n".into()),
        ))
        .unwrap();
    let bytes = save(&source.open(), &patch);
    assert_eq!(bytes, format!(
        "{HEADER}sfen {C}\n# position\n# line2\n# sfen injected\n# \n7g7f none none none\n# move\n# sfen injected\n# # note\n# \n"
    ).as_bytes());
    let saved = Source::new(std::str::from_utf8(&bytes).unwrap().strip_prefix(HEADER).unwrap());
    let book = saved.open();
    assert_eq!(book.iter_entries().unwrap().count(), 1);
    assert_eq!(book.lookup_sfen(C).unwrap().unwrap().moves().len(), 1);
}

#[test]
fn inserts_replaces_and_deletes_in_normalized_order() {
    let source = Source::new(&format!("sfen {A}\nsfen {C} 18\n7g7f none 1 2 3\n"));
    let mut patch = BookDatabasePatch::new();
    patch.upsert_entry(entry(A, Some(0), vec![])).unwrap();
    patch.upsert_entry(entry(B, Some(8), vec![])).unwrap();
    patch.remove_entry_by_sfen(&format!("{C} 99")).unwrap();
    assert_eq!(
        save(&source.open(), &patch),
        format!("{HEADER}sfen {A} 0\nsfen {B} 8\n").as_bytes()
    );
    patch.remove_entry_by_sfen(&format!("{A} 3")).unwrap();
    patch.remove_entry_by_sfen(&format!("{B} 3")).unwrap();
    assert_eq!(save(&source.open(), &patch), HEADER.as_bytes());
}

#[test]
fn patch_last_operation_wins_and_failed_upsert_preserves_previous_edit() {
    let mut patch = BookDatabasePatch::default();
    assert_eq!(patch, BookDatabasePatch::new());
    patch.remove_entry_by_sfen(&format!("{A} 9")).unwrap();
    assert!(matches!(
        patch.edit_by_sfen(&format!("{A} 1")).unwrap(),
        Some(BookDatabaseEdit::Delete)
    ));
    patch.upsert_entry(entry(A, None, vec![])).unwrap();
    let previous = patch.clone();
    for moves in [
        vec![candidate(Move::MOVE_NONE, None)],
        vec![candidate(Move::from_raw(0xffff), None)],
        vec![candidate(Move::from_usi("5i4i").unwrap(), None); 2],
        vec![BookCandidate::new(
            Move::from_usi("5i4i").unwrap(),
            Some(Move::MOVE_NONE),
            None,
            None,
            None,
            None,
            BookMoveMetadata::default(),
        )],
    ] {
        assert!(matches!(
            patch.upsert_entry(entry(A, None, moves)),
            Err(BookError::InvalidData(_))
        ));
        assert_eq!(patch, previous);
    }
    assert!(patch.remove_entry_by_sfen("invalid").is_err());
    assert_eq!(patch, previous);
    assert!(matches!(
        patch.edit_by_sfen(&format!("{A} 87")).unwrap(),
        Some(BookDatabaseEdit::Upsert(_))
    ));
}

#[test]
fn validates_all_source_rows_even_when_deleted_or_replaced() {
    for body in [
        format!("sfen {A}\nsfen {A} 2\n"),
        format!("sfen {C}\nsfen {A}\n"),
        format!("sfen {A}\nbad-move none 0 1 2\n"),
        format!("sfen {A}\n5i4i none 0 1 2\n5i4i none 0 1 2\n"),
        format!("sfen {A}\nsfen invalid\n"),
        "stray data\n".to_owned(),
    ] {
        let source = Source::new(&body);
        let mut patch = BookDatabasePatch::new();
        patch.remove_entry_by_sfen(&format!("{A} 1")).unwrap();
        patch.remove_entry_by_sfen(&format!("{C} 1")).unwrap();
        let mut output = Vec::new();
        assert!(
            matches!(
                source.open().write_yaneuraou_db2016_with_patch(&mut output, &patch),
                Err(BookError::InvalidData(_))
            ),
            "{body}"
        );
    }
}

#[test]
fn rejects_unrepresentable_patch_before_output() {
    let source = Source::new(&format!("sfen {A}\n"));
    let mut patch = BookDatabasePatch::new();
    patch
        .upsert_entry(entry(A, None, vec![candidate(Move::from_usi("5i4i").unwrap(), Some(0))]))
        .unwrap();
    let mut output = Vec::new();
    assert!(matches!(
        source.open().write_yaneuraou_db2016_with_patch(&mut output, &patch),
        Err(BookError::Unsupported(_))
    ));
    assert!(output.is_empty());
}

struct FaultWriter {
    output: Vec<u8>,
    fail_after: Option<usize>,
    fail_flush: bool,
}

impl Write for FaultWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let remaining =
            self.fail_after.map_or(usize::MAX, |limit| limit.saturating_sub(self.output.len()));
        if remaining == 0 {
            return Err(io::Error::other("injected write failure"));
        }
        let length = bytes.len().min(3).min(remaining);
        self.output.extend_from_slice(&bytes[..length]);
        Ok(length)
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush { Err(io::Error::other("injected flush failure")) } else { Ok(()) }
    }
}

#[test]
fn short_writes_io_failures_and_cancel_allow_retry() {
    let source = Source::new(&format!("# preamble\nsfen {A}\nsfen {C}\n"));
    let book = source.open();
    let mut patch = BookDatabasePatch::new();
    patch.upsert_entry(entry(B, None, vec![])).unwrap();
    let expected = save(&book, &patch);
    let unchanged = patch.clone();
    for (fail_after, fail_flush) in [
        (None, false),
        (Some(5), false),
        (Some(HEADER.len() + 2), false),
        (Some(50), false),
        (None, true),
    ] {
        let mut writer = FaultWriter { output: Vec::new(), fail_after, fail_flush };
        let result = book.write_yaneuraou_db2016_with_patch(&mut writer, &patch);
        if fail_after.is_none() && !fail_flush {
            result.unwrap();
            assert_eq!(writer.output, expected);
        } else {
            assert!(matches!(result, Err(BookError::Io(_))));
        }
        assert_eq!(patch, unchanged);
        assert_eq!(save(&book, &patch), expected);
    }
    for cancel_at in 0..5 {
        let mut call = 0;
        let mut output = Vec::new();
        let result =
            book.write_yaneuraou_db2016_with_patch_with_control(&mut output, &patch, |progress| {
                assert_eq!(progress.phase(), BookPatchWritePhase::Writing);
                assert_eq!(progress.total_positions(), None);
                assert!(progress.processed_positions() <= 2);
                let cancel = call == cancel_at;
                call += 1;
                if cancel { BookControl::Cancel } else { BookControl::Continue }
            });
        assert!(matches!(result, Err(BookError::Cancelled)));
        assert_eq!(patch, unchanged);
        assert_eq!(save(&book, &patch), expected);
    }
}
