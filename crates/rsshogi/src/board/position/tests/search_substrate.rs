use crate::board::{generate_position_state, hirate_position, position_from_sfen};
use crate::types::{Color, Move32, Piece, PieceType, Square};

use super::MoveError;

fn assert_search_move_roundtrip(mut pos: super::Position, mv: Move32) {
    let before = generate_position_state(&pos);
    let key = pos.key();
    let game_ply = pos.game_ply();
    let depth = pos.state_stack_depth();
    pos.prepare_search_state_capacity(1).expect("prepare one search state");
    let chunks = pos.state_stack().chunk_count();

    let facts = pos
        .try_apply_search_move32_with_facts(mv, pos.gives_check_move32(mv))
        .expect("apply prepared search move");
    assert_eq!(pos.state_stack().chunk_count(), chunks);
    assert_eq!(facts.mv, mv);
    pos.undo_move32(mv).expect("undo search move");

    assert_eq!(generate_position_state(&pos), before);
    assert_eq!(pos.key(), key);
    assert_eq!(pos.game_ply(), game_ply);
    assert_eq!(pos.state_stack_depth(), depth);
}

#[test]
fn prepared_search_moves_cover_normal_capture_promotion_drop_and_king() {
    let pos = hirate_position();
    let mv = pos.move32_from_usi("7g7f").expect("normal move");
    assert_search_move_roundtrip(pos, mv);

    let mut pos = hirate_position();
    for usi in ["7g7f", "3c3d", "3g3f", "3d3e"] {
        let mv = pos.move32_from_usi(usi).expect("setup move");
        pos.apply_move32(mv);
    }
    let mv = pos.move32_from_usi("3f3e").expect("capture move");
    assert_search_move_roundtrip(pos, mv);

    let pos = position_from_sfen("4k4/9/4P4/9/9/9/9/9/4K4 b - 1").unwrap();
    let mv = Move32::promotion(
        Square::from_usi("5c").unwrap(),
        Square::from_usi("5b").unwrap(),
        Piece::B_PAWN,
    );
    assert_search_move_roundtrip(pos, mv);

    let pos = position_from_sfen("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPP1PPPP/1B5R1/LNSGKGSNL b P 1")
        .unwrap();
    let mv = Move32::drop(PieceType::PAWN, Square::from_usi("5e").unwrap(), Color::BLACK);
    assert_search_move_roundtrip(pos, mv);

    let pos = position_from_sfen("4k4/9/9/9/9/9/9/9/4K4 b - 1").unwrap();
    let mv = Move32::normal(
        Square::from_usi("5i").unwrap(),
        Square::from_usi("5h").unwrap(),
        Piece::B_KING,
    );
    assert_search_move_roundtrip(pos, mv);
}

#[test]
fn search_move_capacity_failure_does_not_mutate_position() {
    let mut pos = hirate_position();
    let initial_capacity = pos.state_stack().prepared_additional_capacity();
    for _ in 0..initial_capacity {
        pos.try_apply_search_null_move().expect("consume existing physical slot");
    }
    assert!(!pos.has_prepared_search_state());

    let before = generate_position_state(&pos);
    let key = pos.key();
    let game_ply = pos.game_ply();
    let depth = pos.state_stack_depth();
    let usi = if pos.turn() == Color::BLACK { "7g7f" } else { "3c3d" };
    let mv = pos.move32_from_usi(usi).expect("legal move at exhausted state");

    assert_eq!(pos.try_apply_search_null_move(), Err(MoveError::StateCapacityExceeded));
    assert_eq!(generate_position_state(&pos), before);
    assert_eq!(pos.key(), key);
    assert_eq!(pos.game_ply(), game_ply);
    assert_eq!(pos.state_stack_depth(), depth);

    assert_eq!(
        pos.try_apply_search_move32_with_facts(mv, pos.gives_check_move32(mv)),
        Err(MoveError::StateCapacityExceeded)
    );
    assert_eq!(generate_position_state(&pos), before);
    assert_eq!(pos.key(), key);
    assert_eq!(pos.game_ply(), game_ply);
    assert_eq!(pos.state_stack_depth(), depth);
}

#[test]
fn search_null_preserves_game_ply_and_roundtrips() {
    let mut pos = hirate_position();
    pos.prepare_search_state_capacity(1).expect("prepare search null state");
    let before = generate_position_state(&pos);
    let key = pos.key();
    let game_ply = pos.game_ply();
    let depth = pos.state_stack_depth();

    pos.try_apply_search_null_move().expect("apply search null");
    assert_eq!(pos.game_ply(), game_ply);
    assert_ne!(pos.key(), key);
    assert_eq!(pos.state_stack_depth(), depth + 1);
    pos.undo_search_null_move().expect("undo search null");

    assert_eq!(generate_position_state(&pos), before);
    assert_eq!(pos.key(), key);
    assert_eq!(pos.game_ply(), game_ply);
    assert_eq!(pos.state_stack_depth(), depth);
}

#[test]
fn ordinary_null_still_advances_game_ply() {
    let mut pos = hirate_position();
    let game_ply = pos.game_ply();
    pos.apply_null_move().expect("apply ordinary null");
    assert_eq!(pos.game_ply(), game_ply + 1);
    pos.undo_null_move().expect("undo ordinary null");
    assert_eq!(pos.game_ply(), game_ply);
}

#[test]
fn null_undo_underflow_is_atomic_for_ordinary_and_search_apis() {
    let mut ordinary =
        position_from_sfen("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 42")
            .unwrap();
    assert_eq!(ordinary.state_stack_depth(), 0);
    assert!(ordinary.game_ply() > 0);
    let ordinary_state = generate_position_state(&ordinary);
    let ordinary_key = ordinary.key();
    let ordinary_ply = ordinary.game_ply();
    assert_eq!(ordinary.undo_null_move(), Err(MoveError::StackUnderflow));
    assert_eq!(generate_position_state(&ordinary), ordinary_state);
    assert_eq!(ordinary.key(), ordinary_key);
    assert_eq!(ordinary.game_ply(), ordinary_ply);
    assert_eq!(ordinary.state_stack_depth(), 0);

    let mut search = hirate_position();
    let search_state = generate_position_state(&search);
    let search_key = search.key();
    let search_ply = search.game_ply();
    assert_eq!(search.undo_search_null_move(), Err(MoveError::StackUnderflow));
    assert_eq!(generate_position_state(&search), search_state);
    assert_eq!(search.key(), search_key);
    assert_eq!(search.game_ply(), search_ply);
    assert_eq!(search.state_stack_depth(), 0);
}

#[test]
fn prepared_capacity_is_cloned_without_sharing_state_storage() {
    let mut original = hirate_position();
    original.prepare_search_state_capacity(65).expect("prepare capacity before clone");
    let original_chunks = original.state_stack().chunk_count();
    let original_state_ptr = std::ptr::from_ref(original.current_state());

    let mut cloned = original.clone();
    assert_eq!(cloned.state_stack().chunk_count(), original_chunks);
    assert_ne!(std::ptr::from_ref(cloned.current_state()), original_state_ptr);

    for _ in 0..64 {
        original.try_apply_search_null_move().expect("original reaches prepared extra chunk");
    }
    let original_extra_ptr = std::ptr::from_ref(original.current_state());
    original.state_stack_mut().current_mut().repetition_counter = 77;

    for _ in 0..64 {
        cloned.try_apply_search_null_move().expect("clone reaches cloned extra chunk");
    }
    assert_ne!(std::ptr::from_ref(cloned.current_state()), original_extra_ptr);
    assert_eq!(original.current_state().repetition_counter, 77);
    assert_ne!(cloned.current_state().repetition_counter, 77);
}
