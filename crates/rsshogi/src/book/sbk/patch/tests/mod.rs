use std::sync::atomic::{AtomicUsize, Ordering};
use std::{fs, io};

use super::*;
use crate::book::{BookEntryMetadata, BookMoveMetadata, BookPosition};

static SERIAL: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    path: std::path::PathBuf,
    book: SbkBook,
}

impl Fixture {
    fn new(bytes: &[u8]) -> Self {
        crate::board::init();
        let path = std::env::temp_dir().join(format!(
            "sbk-patch-{}-{}.sbk",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, bytes).unwrap();
        let book = SbkBook::open(&path).unwrap();
        Self { path, book }
    }
    fn output(&self, patch: &BookDatabasePatch) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.book.write_sbk_with_patch(&mut bytes, patch).unwrap();
        bytes
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn state(id: i32, position: Option<&str>, moves: &[(&Position, &str, Option<i32>)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_field_i32(&mut bytes, FIELD_STATE_ID, id);
    if let Some(sfen) = position {
        write_field_string(&mut bytes, FIELD_STATE_POSITION, sfen).unwrap();
    }
    for (pos, usi, next) in moves {
        let mut payload = Vec::new();
        write_field_i32(
            &mut payload,
            FIELD_MOVE_MOVE,
            sbk_move_word_from_position_move(pos, Move::from_usi(usi).unwrap()).unwrap(),
        );
        if let Some(id) = next {
            write_field_i32(&mut payload, FIELD_MOVE_NEXT_STATE_ID, *id);
        }
        write_field_string(&mut payload, 90, "move-unknown").unwrap();
        write_field_message(&mut bytes, FIELD_STATE_MOVES, &payload).unwrap();
    }
    write_field_string(&mut bytes, 90, "state-unknown").unwrap();
    bytes
}

fn archive(states: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_field_string(&mut bytes, FIELD_SBOOK_AUTHOR, "author").unwrap();
    for state in states {
        write_field_message(&mut bytes, FIELD_SBOOK_STATE, state).unwrap();
    }
    write_field_string(&mut bytes, FIELD_SBOOK_DESCRIPTION, "description").unwrap();
    write_field_string(&mut bytes, 90, "top-unknown").unwrap();
    bytes
}

fn positions() -> Vec<Position> {
    crate::board::init();
    let mut position = Position::from_sfen(InitialPosition::Standard.to_sfen()).unwrap();
    let mut result = vec![position.clone()];
    for usi in ["7g7f", "3c3d", "2g2f"] {
        let mv = position.move32_from_move(Move::from_usi(usi).unwrap());
        position.apply_move32(mv);
        result.push(position.clone());
    }
    result
}

fn chain() -> Fixture {
    let pos = positions();
    Fixture::new(&archive(&[
        state(10, Some(&pos[0].to_sfen(None)), &[(&pos[0], "7g7f", Some(40))]),
        state(40, None, &[(&pos[1], "3c3d", Some(70))]),
        state(70, None, &[(&pos[2], "2g2f", None)]),
    ]))
}

fn decode(bytes: &[u8]) -> Vec<RawSbkState> {
    fields(bytes)
        .unwrap()
        .iter()
        .filter(|f| f.number == FIELD_SBOOK_STATE)
        .map(|f| decode_state_payload(f.payload).unwrap())
        .collect()
}

fn commented(book: &SbkBook, pos: &Position) -> BookDatabaseEntry {
    let entry = BookDatabaseEntry::from_sbk(&book.lookup_position(pos).unwrap().unwrap()).unwrap();
    BookDatabaseEntry::new(
        entry.position().clone(),
        entry.candidates().to_vec(),
        BookEntryMetadata::from_sbk(SbkEntryMetadata::new(
            None,
            0,
            None,
            None,
            None,
            Some("edited".into()),
            vec![],
        )),
    )
}

#[test]
fn unchanged_preserves_unknown_fields_optional_values_and_omitted_positions() {
    let fixture = chain();
    let bytes = fixture.output(&BookDatabasePatch::new());
    let raw = decode(&bytes);
    assert_eq!(raw.iter().map(|s| s.id).collect::<Vec<_>>(), vec![Some(0), Some(1), Some(2)]);
    assert!(raw[0].position.is_some());
    assert!(raw[1].position.is_none() && raw[2].position.is_none());
    assert_eq!(raw[0].moves[0].next_state_id, Some(1));
    assert_eq!(raw[2].moves[0].next_state_id, None);
    assert_eq!(raw[0].moves[0].evaluation, None);
    for word in ["state-unknown", "move-unknown", "top-unknown", "author", "description"] {
        assert!(bytes.windows(word.len()).any(|window| window == word.as_bytes()));
    }
    let reopened = Fixture::new(&bytes);
    assert_eq!(reopened.book.len(), 3);
    assert_eq!(bytes, fixture.output(&BookDatabasePatch::new()));
}

#[test]
fn deleted_parent_keeps_orphan_and_terminates_incoming_edge() {
    let fixture = chain();
    let pos = positions();
    let mut patch = BookDatabasePatch::new();
    patch.remove_entry_by_sfen(&pos[1].to_sfen(None)).unwrap();
    let bytes = fixture.output(&patch);
    let raw = decode(&bytes);
    assert_eq!(raw.len(), 2);
    assert_eq!(raw[0].moves[0].next_state_id, Some(-1));
    assert!(raw[1].position.is_some());
    let reopened = Fixture::new(&bytes);
    assert!(reopened.book.lookup_position(&pos[2]).unwrap().is_some());
}

#[test]
fn comment_edit_does_not_reconnect_existing_terminal() {
    let fixture = chain();
    let pos = positions();
    let mut patch = BookDatabasePatch::new();
    patch.upsert_entry(commented(&fixture.book, &pos[2])).unwrap();
    patch
        .upsert_entry(BookDatabaseEntry::new(
            BookPosition::from_position(&pos[3], None),
            vec![],
            BookEntryMetadata::new(),
        ))
        .unwrap();
    let bytes = fixture.output(&patch);
    let raw = decode(&bytes);
    assert_eq!(raw[2].comment.as_deref(), Some("edited"));
    assert_eq!(raw[2].moves[0].next_state_id, None);
    assert!(raw[3].position.is_some());
    assert_eq!(Fixture::new(&bytes).book.len(), 4);
}

#[test]
fn explicit_ply_on_existing_and_new_linked_children_is_preserved() {
    let fixture = chain();
    let pos = positions();
    let entry =
        BookDatabaseEntry::from_sbk(&fixture.book.lookup_position(&pos[1]).unwrap().unwrap())
            .unwrap();
    let mut patch = BookDatabasePatch::new();
    patch
        .upsert_entry(BookDatabaseEntry::new(
            BookPosition::from_position(&pos[1], Some(100)),
            entry.candidates().to_vec(),
            entry.metadata().clone(),
        ))
        .unwrap();
    let bytes = fixture.output(&patch);
    assert!(decode(&bytes)[1].position.as_deref().unwrap().ends_with(" 100"));
    assert!(
        Fixture::new(&bytes)
            .book
            .lookup_position(&pos[1])
            .unwrap()
            .unwrap()
            .sfen()
            .ends_with(" 100")
    );
    let candidate = BookCandidate::new(
        Move::from_usi("2g2f").unwrap(),
        None,
        None,
        None,
        None,
        None,
        BookMoveMetadata::new(),
    );
    let fixture = Fixture::new(&archive(&[state(1, Some(&pos[2].to_sfen(None)), &[])]));
    let mut patch = BookDatabasePatch::new();
    patch
        .upsert_entry(BookDatabaseEntry::new(
            BookPosition::from_position(&pos[2], None),
            vec![candidate],
            BookEntryMetadata::new(),
        ))
        .unwrap();
    patch
        .upsert_entry(BookDatabaseEntry::new(
            BookPosition::from_position(&pos[3], Some(200)),
            vec![],
            BookEntryMetadata::new(),
        ))
        .unwrap();
    let bytes = fixture.output(&patch);
    assert!(decode(&bytes)[1].position.as_deref().unwrap().ends_with(" 200"));
    patch
        .upsert_entry(BookDatabaseEntry::new(
            BookPosition::from_position(&pos[3], Some(65536)),
            vec![],
            BookEntryMetadata::new(),
        ))
        .unwrap();
    let mut bytes = Vec::new();
    assert!(matches!(
        fixture.book.write_sbk_with_patch(&mut bytes, &patch),
        Err(BookError::InvalidData(_))
    ));
    assert!(bytes.is_empty());
}

#[test]
fn maximum_ply_edge_to_visited_state_can_be_saved() {
    let pos = positions();
    let fixture = Fixture::new(&archive(&[
        state(0, Some(&pos[1].to_sfen(None)), &[]),
        state(1, Some(&pos[0].to_sfen(Some(65535))), &[(&pos[0], "7g7f", Some(0))]),
    ]));
    let bytes = fixture.output(&BookDatabasePatch::new());
    let reopened = Fixture::new(&bytes);
    assert_eq!(reopened.book.len(), 2);
    assert!(reopened.book.lookup_position(&pos[0]).unwrap().unwrap().sfen().ends_with(" 65535"));
    assert_eq!(decode(&bytes)[1].moves[0].next_state_id, Some(0));
}

#[test]
fn explicit_ply_must_leave_room_for_reader_graph_traversal() {
    let fixture = chain();
    let pos = positions();
    let original =
        BookDatabaseEntry::from_sbk(&fixture.book.lookup_position(&pos[1]).unwrap().unwrap())
            .unwrap();
    let mut patch = BookDatabasePatch::new();
    patch
        .upsert_entry(BookDatabaseEntry::new(
            BookPosition::from_position(&pos[1], Some(65535)),
            original.candidates().to_vec(),
            original.metadata().clone(),
        ))
        .unwrap();
    let mut output = Vec::new();
    assert!(matches!(
        fixture.book.write_sbk_with_patch(&mut output, &patch),
        Err(BookError::InvalidData(_))
    ));
    assert!(output.is_empty());
    let original =
        BookDatabaseEntry::from_sbk(&fixture.book.lookup_position(&pos[2]).unwrap().unwrap())
            .unwrap();
    let mut patch = BookDatabasePatch::new();
    patch
        .upsert_entry(BookDatabaseEntry::new(
            BookPosition::from_position(&pos[2], Some(65535)),
            original.candidates().to_vec(),
            original.metadata().clone(),
        ))
        .unwrap();
    assert!(
        Fixture::new(&fixture.output(&patch))
            .book
            .lookup_position(&pos[2])
            .unwrap()
            .unwrap()
            .sfen()
            .ends_with(" 65535")
    );
}

#[test]
fn new_candidate_connects_to_new_child_without_full_export() {
    let pos = positions();
    let fixture = Fixture::new(&archive(&[state(99, Some(&pos[2].to_sfen(None)), &[])]));
    let candidate = BookCandidate::new(
        Move::from_usi("2g2f").unwrap(),
        None,
        None,
        None,
        None,
        None,
        BookMoveMetadata::new(),
    );
    let mut patch = BookDatabasePatch::new();
    patch
        .upsert_entry(BookDatabaseEntry::new(
            BookPosition::from_position(&pos[2], None),
            vec![candidate],
            BookEntryMetadata::new(),
        ))
        .unwrap();
    patch
        .upsert_entry(BookDatabaseEntry::new(
            BookPosition::from_position(&pos[3], None),
            vec![],
            BookEntryMetadata::new(),
        ))
        .unwrap();
    let raw = decode(&fixture.output(&patch));
    assert_eq!(raw[0].moves[0].next_state_id, Some(1));
    assert_eq!(raw[0].moves[0].evaluation, Some(0));
    assert!(raw[1].position.is_none());
}

#[test]
fn deletion_of_first_state_anchors_new_first_and_all_deletion_fails() {
    let fixture = chain();
    let pos = positions();
    let mut patch = BookDatabasePatch::new();
    patch.remove_entry_by_sfen(&pos[0].to_sfen(None)).unwrap();
    let bytes = fixture.output(&patch);
    let reopened = Fixture::new(&bytes);
    assert!(reopened.book.lookup_position(&pos[1]).unwrap().is_some());
    assert_eq!(decode(&bytes)[0].id, Some(0));
    for position in &pos[1..3] {
        patch.remove_entry_by_sfen(&position.to_sfen(None)).unwrap();
    }
    let mut output = Vec::new();
    assert!(matches!(
        fixture.book.write_sbk_with_patch(&mut output, &patch),
        Err(BookError::InvalidData(_))
    ));
    assert!(output.is_empty());
}

#[test]
fn unknown_eval_is_retained_but_cannot_be_silently_replaced() {
    let pos = positions();
    let mut source = state(10, Some(&pos[0].to_sfen(None)), &[]);
    let mut eval = Vec::new();
    write_field_i32(&mut eval, 1, 42);
    write_field_string(&mut eval, 99, "eval-unknown").unwrap();
    write_field_message(&mut source, FIELD_STATE_EVALS, &eval).unwrap();
    let fixture = Fixture::new(&archive(&[source]));
    let mut patch = BookDatabasePatch::new();
    let original =
        BookDatabaseEntry::from_sbk(&fixture.book.lookup_position(&pos[0]).unwrap().unwrap())
            .unwrap();
    patch.upsert_entry(original).unwrap();
    assert!(fixture.output(&patch).windows(12).any(|w| w == b"eval-unknown"));
    patch.upsert_entry(commented(&fixture.book, &pos[0])).unwrap();
    let mut bytes = Vec::new();
    assert!(matches!(
        fixture.book.write_sbk_with_patch(&mut bytes, &patch),
        Err(BookError::Unsupported(_))
    ));
    assert!(bytes.is_empty());
}

#[test]
fn strict_save_rejects_dangling_duplicate_and_inconsistent_edges() {
    let pos = positions();
    for source in [
        archive(&[state(1, Some(&pos[0].to_sfen(None)), &[(&pos[0], "7g7f", Some(99))])]),
        {
            let mut s = state(1, Some(&pos[0].to_sfen(None)), &[]);
            write_field_i32(&mut s, FIELD_STATE_ID, 1);
            archive(&[s])
        },
        archive(&[
            state(1, Some(&pos[0].to_sfen(None)), &[(&pos[0], "7g7f", Some(2))]),
            state(2, Some(&pos[2].to_sfen(None)), &[]),
        ]),
    ] {
        let fixture = Fixture::new(&source);
        let mut bytes = Vec::new();
        assert!(matches!(
            fixture.book.write_sbk_with_patch(&mut bytes, &BookDatabasePatch::new()),
            Err(BookError::InvalidData(_))
        ));
        assert!(bytes.is_empty());
    }
}

struct Broken {
    remaining: usize,
    flush: bool,
}
impl Write for Broken {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::other("injected write failure"));
        }
        let count = bytes.len().min(self.remaining).min(3);
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.flush { Err(io::Error::other("injected flush failure")) } else { Ok(()) }
    }
}

#[test]
fn write_flush_cancel_and_retry_keep_patch_unchanged() {
    let fixture = chain();
    let mut patch = BookDatabasePatch::new();
    patch.upsert_entry(commented(&fixture.book, &positions()[0])).unwrap();
    let original = patch.clone();
    for mut broken in
        [Broken { remaining: 20, flush: false }, Broken { remaining: usize::MAX, flush: true }]
    {
        assert!(matches!(
            fixture.book.write_sbk_with_patch(&mut broken, &patch),
            Err(BookError::Io(_))
        ));
    }
    for phase in [BookPatchWritePhase::Validating, BookPatchWritePhase::Writing] {
        assert!(matches!(
            fixture.book.write_sbk_with_patch_with_control(&mut Vec::new(), &patch, |p| {
                if p.phase() == phase { BookControl::Cancel } else { BookControl::Continue }
            }),
            Err(BookError::Cancelled)
        ));
    }
    assert_eq!(original, patch);
    assert_eq!(Fixture::new(&fixture.output(&patch)).book.len(), 3);
}

#[test]
fn directed_cycle_and_child_first_order_remain_resolvable() {
    crate::board::init();
    let start = "4k4/9/9/9/9/9/9/9/4K4 b - 1";
    let mut pos = Position::from_sfen(start).unwrap();
    let mut states = Vec::new();
    for (i, usi) in ["5i4i", "5a4a", "4i5i", "4a5a"].iter().enumerate() {
        states.push(state(
            (i + 10) as i32,
            if i == 0 { Some(start) } else { None },
            &[(&pos, usi, Some(((i + 1) % 4 + 10) as i32))],
        ));
        pos.apply_move32(pos.move32_from_move(Move::from_usi(usi).unwrap()));
    }
    let fixture = Fixture::new(&archive(&states));
    assert_eq!(Fixture::new(&fixture.output(&BookDatabasePatch::new())).book.len(), 4);
    states.rotate_left(1);
    // 物理先頭の平手仮定を避け、入力の child-first state を明示局面にする。
    let mut child = Position::from_sfen(start).unwrap();
    child.apply_move32(child.move32_from_move(Move::from_usi("5i4i").unwrap()));
    write_field_string(&mut states[0], FIELD_STATE_POSITION, &child.to_sfen(None)).unwrap();
    let fixture = Fixture::new(&archive(&states));
    assert_eq!(Fixture::new(&fixture.output(&BookDatabasePatch::new())).book.len(), 4);
}
