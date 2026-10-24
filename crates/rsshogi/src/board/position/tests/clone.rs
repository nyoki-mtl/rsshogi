use super::*;
use crate::board::{
    LegalAll, Move32List, generate_moves_move32, hirate_position, position_from_sfen,
};
use crate::types::EnteringKingRule;
use proptest::prelude::*;

// 比較用は最適化する Clone を呼ばず、従来の全 chunk 複製を使う。
fn reference_clone(source: &Position) -> Position {
    let mut result = Position::empty();
    result.board = source.board;
    result.bitboards = source.bitboards;
    result.hands = source.hands;
    result.side_to_move = source.side_to_move;
    result.entering_king_rule = source.entering_king_rule;
    result.entering_king_point = source.entering_king_point;
    result.ply = source.ply;
    result.st_index = source.st_index;
    result.king_square = source.king_square;
    result.state_stack = source.state_stack.reference_clone();
    result
}

fn assert_same(actual: &Position, expected: &Position) {
    assert_eq!(actual.to_sfen(None), expected.to_sfen(None));
    assert_eq!(actual.entering_king_rule, expected.entering_king_rule);
    assert_eq!(actual.entering_king_point, expected.entering_king_point);
    assert_eq!(actual.king_square, expected.king_square);
    assert_eq!(format!("{:?}", actual.bitboards), format!("{:?}", expected.bitboards));
    assert_eq!(actual.st_index, expected.st_index);
    assert_eq!(actual.state_stack.current_index(), expected.state_stack.current_index());
    for index in 0..=actual.st_index {
        // hot と cold の全フィールドを比較し、padding byte は比較しない。
        assert_eq!(
            format!("{:?}", actual.state_stack[index]),
            format!("{:?}", expected.state_stack[index])
        );
    }
    assert_eq!(actual.repetition_state(), expected.repetition_state());
    for ply in [0, 4, 16, 64, usize::MAX] {
        assert_eq!(
            actual.repetition_state_with_distance(ply),
            expected.repetition_state_with_distance(ply)
        );
    }
}

fn copy_and_compare(source: &Position, target: &mut Position) {
    let expected = reference_clone(source);
    target.clone_from(source);
    assert_same(target, &expected);
    assert_same(&source.clone(), &expected);
    assert_same(&source.clone_compact(), &expected);
    assert!(target.state_stack.chunk_count() >= source.state_stack.chunk_count());
    assert_ne!(
        std::ptr::from_ref(target.current_state()),
        std::ptr::from_ref(source.current_state())
    );
}

#[test]
fn clone_compact_keeps_history_settings_and_minimum_chunks() {
    for depth in [0, 1, 62, 63, 64, 65, 127, 128, 150, 300] {
        for indexed in [false, true] {
            for reserved in [0, 64, 256] {
                let mut source = kings(depth);
                if indexed {
                    source = source.clone_for_search();
                }
                source.set_entering_king_rule(EnteringKingRule::Point27);
                source.prepare_search_state_capacity(reserved).unwrap();
                let expected = reference_clone(&source);
                let source_chunks = source.state_stack.chunk_count();
                let source_ptr = std::ptr::from_ref(source.current_state());
                let view = source.state_history();
                let mut compact = source.clone_compact();
                assert_same(&compact, &expected);
                assert_eq!(compact.state_stack.chunk_count(), (depth + 1).div_ceil(64));
                assert_eq!(compact.has_prepared_search_state(), depth % 64 != 63);
                assert_eq!(source.state_stack.chunk_count(), source_chunks);
                assert_eq!(std::ptr::from_ref(source.current_state()), source_ptr);
                assert_eq!(view.len(), depth + 1);
                assert_ne!(std::ptr::from_ref(compact.current_state()), source_ptr);
                let mut undo_expected = reference_clone(&expected);
                while compact.state_stack_depth() > 0 {
                    let mv = compact.last_move();
                    compact.undo_move32(mv).unwrap();
                    undo_expected.undo_move32(mv).unwrap();
                    assert_eq!(compact.to_sfen(None), undo_expected.to_sfen(None));
                    assert_eq!(compact.key(), undo_expected.key());
                    assert_eq!(compact.repetition_state(), undo_expected.repetition_state());
                    assert_eq!(
                        format!("{:?}", compact.current_state()),
                        format!("{:?}", undo_expected.current_state())
                    );
                }
                assert_same(&source, &expected);
            }
        }
    }
}

#[test]
fn clone_compact_keeps_null_boundary_and_independent_rebranching() {
    let mut source = kings(150).clone_for_search();
    source.apply_null_move().unwrap();
    source.undo_null_move().unwrap();
    // undo 済み slot と余剰 chunk を持った局面を複製する。
    for _ in 0..130 {
        source.apply_null_move().unwrap();
    }
    for _ in 0..130 {
        source.undo_null_move().unwrap();
    }
    source.apply_null_move().unwrap();
    let mut compact = source.clone_compact();
    let mut expected = reference_clone(&source);
    assert_same(&compact, &expected);
    assert_eq!(compact.state_stack_depth(), 151);
    assert!(compact.state_stack.chunk_count() < source.state_stack.chunk_count());
    compact.undo_null_move().unwrap();
    expected.undo_null_move().unwrap();
    for _ in 0..3 {
        let mv = compact.last_move();
        compact.undo_move32(mv).unwrap();
        expected.undo_move32(mv).unwrap();
    }
    let mut moves = Move32List::new();
    generate_moves_move32::<LegalAll>(&compact, &mut moves);
    for mv in moves.iter().copied() {
        compact.try_apply_move32(mv).unwrap();
        expected.try_apply_move32(mv).unwrap();
        assert_same(&compact, &expected);
        compact.undo_move32(mv).unwrap();
        expected.undo_move32(mv).unwrap();
    }
    source.undo_null_move().unwrap();
    assert_eq!(source.state_stack_depth(), 150);
    assert_eq!(compact.state_stack_depth(), 147);
}

fn kings(depth: usize) -> Position {
    let mut pos = position_from_sfen("4k4/9/9/9/9/9/9/9/4K4 b - 1").unwrap();
    for usi in ["5i6i", "5a6a", "6i5i", "6a5a"].into_iter().cycle().take(depth) {
        let mv = pos.move32_from_usi(usi).unwrap();
        pos.try_apply_move32(mv).unwrap();
    }
    pos
}

#[test]
fn clone_from_replaces_history_settings_and_index_across_capacities() {
    for depth in [0, 1, 63, 64, 65, 150] {
        let mut source = kings(depth);
        source.prepare_search_state_capacity(130).unwrap();
        source.set_entering_king_rule(EnteringKingRule::Point27);
        for target_depth in [0, 63, 192] {
            let mut target = kings(target_depth).clone_for_search();
            let original_capacity = target.state_stack.chunk_count();
            copy_and_compare(&source, &mut target);
            assert!(target.state_stack.chunk_count() >= original_capacity);
            source = source.clone_for_search();
            copy_and_compare(&source, &mut target);
            target.apply_null_move().unwrap();
            target.undo_null_move().unwrap();
            assert_same(&target, &source);
            while target.state_stack_depth() > 0 {
                let mv = target.current_state().last_move();
                target.undo_move32(mv).unwrap();
            }
            copy_and_compare(&Position::empty(), &mut target);
        }
    }
}

#[test]
fn clone_from_preserves_capture_drop_promotion_and_perpetual_check() {
    let cases = [
        (
            "4r3k/9/9/9/9/9/9/9/K4R3 b P 1",
            vec!["P*5h", "5a5h", "9i8i", "P*4g", "4i4g", "1a2a", "4g4i", "2a1a", "8i9i", "5h5a"],
        ),
        (
            "lr2+R2G1/6g1k/p3ppppp/2p1b4/9/2P+b4P/P+p3PPP1/4G1SK1/LN3+p1NL b 2SNLgsn3p 1",
            vec!["2a1a", "1b2b", "1a2a", "2b1b"],
        ),
        ("4k4/9/4P4/9/9/9/9/9/4K4 b - 1", vec!["5c5b+"]),
    ];
    for (sfen, moves) in cases {
        let mut source = position_from_sfen(sfen).unwrap();
        let mut target = kings(192).clone_for_search();
        let mut applied = Vec::new();
        let repeats = if moves.len() == 1 { 1 } else { 3 };
        for usi in moves.repeat(repeats) {
            let mv = source.move32_from_usi(usi).unwrap();
            source.try_apply_move32(mv).unwrap();
            applied.push(mv);
            copy_and_compare(&source, &mut target);
        }
        for mv in applied.into_iter().rev() {
            source.undo_move32(mv).unwrap();
            target.undo_move32(mv).unwrap();
            assert_same(&target, &source);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn clone_matches_reference_after_random_apply_undo(choices in prop::collection::vec(any::<u16>(), 80..180)) {
        let mut source = hirate_position();
        let mut target = kings(192).clone_for_search();
        let mut path = Vec::new();
        for (step, choice) in choices.into_iter().enumerate() {
            if choice % 4 == 0 && !path.is_empty() {
                source.undo_move32(path.pop().unwrap()).unwrap();
            } else {
                let mut legal = Move32List::new();
                generate_moves_move32::<LegalAll>(&source, &mut legal);
                if !legal.is_empty() {
                    let mv = legal[usize::from(choice) % legal.len()];
                    source.try_apply_move32(mv).unwrap();
                    path.push(mv);
                }
            }
            if step % 13 == 0 {
                copy_and_compare(&source, &mut target);
                let mut compact = source.clone_compact();
                let mut expected = reference_clone(&source);
                let mut legal = Move32List::new();
                generate_moves_move32::<LegalAll>(&source, &mut legal);
                if !legal.is_empty() {
                    let mv = legal[usize::from(choice) % legal.len()];
                    target.try_apply_move32(mv).unwrap();
                    compact.try_apply_move32(mv).unwrap();
                    expected.try_apply_move32(mv).unwrap();
                    assert_same(&target, &expected);
                    assert_same(&compact, &expected);
                    target.undo_move32(mv).unwrap();
                    compact.undo_move32(mv).unwrap();
                    expected.undo_move32(mv).unwrap();
                }
                for &mv in path.iter().rev().take(3) {
                    target.undo_move32(mv).unwrap();
                    compact.undo_move32(mv).unwrap();
                    expected.undo_move32(mv).unwrap();
                    assert_same(&target, &expected);
                    assert_same(&compact, &expected);
                }
            }
        }
    }
}
