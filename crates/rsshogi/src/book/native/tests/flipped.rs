use super::*;

fn query_position() -> Position {
    Position::from_sfen("lnsgkgsnl/1r5b1/p1ppppppp/1p7/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL b - 3")
        .unwrap()
}

fn reversed_position() -> Position {
    Position::from_sfen("lnsgkgsnl/1r5b1/pppppp1pp/6p2/9/7P1/PPPPPPP1P/1B5R1/LNSGKGSNL w - 37")
        .unwrap()
}

fn reversed_entry() -> NativeBookEntry {
    let mut first = candidate("3d3e", 1);
    first.score = NativeScore::Cp { value: -73, bound: NativeBound::Lower };
    first.depth = Some(19);
    let mut second = candidate("8c8d", 0);
    second.score = NativeScore::Mate { plies: -7, bound: NativeBound::Upper };
    second.depth = Some(u32::MAX);
    NativeBookEntry::from_position(&reversed_position(), vec![first, second]).unwrap()
}

fn build(path: &Path, entries: &[NativeBookEntry]) -> NativeBook {
    let mut builder = NativeBookBuilder::new(path, NativeBookMetadata::default()).unwrap();
    for entry in entries {
        builder.push(NativeBookPosition::new(entry.clone())).unwrap();
    }
    builder.finish().unwrap();
    NativeBook::open(path).unwrap()
}

#[test]
fn flipped_lookup_restores_query_key_moves_and_canonical_order() {
    let directory = Directory::new();
    let stored = reversed_entry();
    let book = build(&directory.path("flipped.book"), std::slice::from_ref(&stored));
    let query = query_position();
    assert_eq!(book.get(&query).unwrap(), None);
    assert_eq!(book.get_key(*query.try_to_packed_sfen().unwrap().as_bytes()).unwrap(), None);
    let entry = book.get_with_flipped(&query).unwrap().unwrap();
    assert_eq!(entry.key, *query.try_to_packed_sfen().unwrap().as_bytes());
    assert!(entry.candidates.windows(2).all(|pair| pair[0].mv.raw() < pair[1].mv.raw()));
    assert_ne!(entry.candidates[0].display_order, stored.candidates[0].display_order);
    for (source, expected_move) in [("3d3e", "7f7e"), ("8c8d", "2g2f")] {
        let source = stored
            .candidates
            .iter()
            .find(|candidate| candidate.mv == Move::from_usi(source).unwrap())
            .unwrap();
        let mut expected = source.clone();
        expected.mv = Move::from_usi(expected_move).unwrap();
        assert!(query.is_legal_move(expected.mv));
        assert!(entry.candidates.contains(&expected));
    }
    entry.validate().unwrap();
    assert_eq!(book.get(&reversed_position()).unwrap(), Some(stored));
}

#[test]
fn flipped_lookup_direct_entries_take_precedence_including_empty_entries() {
    let directory = Directory::new();
    let direct =
        NativeBookEntry::from_position(&query_position(), vec![candidate("5g5f", 0)]).unwrap();
    let reversed = reversed_entry();
    let book = build(&directory.path("both.book"), &[direct.clone(), reversed.clone()]);
    assert_eq!(book.get_with_flipped(&query_position()).unwrap(), Some(direct));
    assert_eq!(book.get_with_flipped(&reversed_position()).unwrap(), Some(reversed.clone()));

    let empty = NativeBookEntry::from_position(&query_position(), vec![]).unwrap();
    let book = build(&directory.path("empty.book"), &[empty.clone(), reversed]);
    assert_eq!(book.get_with_flipped(&query_position()).unwrap(), Some(empty));
}

#[test]
fn flipped_lookup_missing_and_invalid_positions() {
    let directory = Directory::new();
    let book = build(&directory.path("miss.book"), &[reversed_entry()]);
    let mut query = query_position();
    let mv = Move::from_usi("7f7e").unwrap();
    query.apply_move32(query.move32_from_move(mv));
    assert_eq!(book.get_with_flipped(&query).unwrap(), None);
    assert!(book.get_with_flipped(&Position::empty()).is_err());
}

#[test]
fn depth_kind_constants_match_wire_values() {
    assert_eq!(DEPTH_KIND_UNSPECIFIED, 0);
    assert_eq!(DEPTH_KIND_SEARCH, 1);
    assert_eq!(DEPTH_KIND_RETROGRADE, 2);
    assert_eq!(NativeBookMetadata::default().depth_kind, DEPTH_KIND_UNSPECIFIED);
}
