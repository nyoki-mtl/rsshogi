use super::*;
use crate::board::{PositionStateError, generate_position_state, hirate_position};
use crate::types::{Color, Hand, HandPiece, Move, Move32, Piece, Square};

fn assert_update_error(
    pos: &mut Position,
    expected: MoveError,
    operation: impl FnOnce(&mut Position) -> Result<(), MoveError>,
) {
    let before = generate_position_state(pos);
    let state = format!("{:?}", pos.current_state());
    let depth = pos.state_stack_depth();
    assert_eq!(operation(pos), Err(expected));
    assert_eq!(generate_position_state(pos), before);
    assert_eq!(format!("{:?}", pos.current_state()), state);
    assert_eq!(pos.state_stack_depth(), depth);
}

#[test]
fn fallible_updates_reject_invalid_moves_without_mutation() {
    let mut pos = hirate_position();
    for mv in [Move32::from_raw(0x4400), Move32::MOVE_NONE, Move32::from_raw(0xffff)] {
        assert_update_error(&mut pos, MoveError::InvalidMove, |p| p.try_apply_move32(mv));
        assert_update_error(&mut pos, MoveError::InvalidMove, |p| {
            p.try_apply_move32_with_facts(mv).map(|_| ())
        });
        assert_update_error(&mut pos, MoveError::InvalidMove, |p| {
            p.try_apply_search_move32_with_facts(mv, false).map(|_| ())
        });
    }
    let illegal = pos.move32_from_move(Move::from_usi("7g7e").unwrap());
    assert_update_error(&mut pos, MoveError::InvalidMove, |p| p.try_apply_move32(illegal));
}

#[test]
fn infallible_update_contract_panics_before_mutation() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for facts in [false, true] {
        let mut pos = hirate_position();
        let before = generate_position_state(&pos);
        let key = pos.key();
        let bad = Move32::from_raw(0x4400);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                if facts {
                    let _ = pos.apply_move32_with_facts(bad, false);
                } else {
                    pos.apply_move32(bad);
                }
            }))
            .is_err()
        );
        assert_eq!(generate_position_state(&pos), before);
        assert_eq!(pos.key(), key);
        assert_eq!(pos.state_stack_depth(), 0);
    }
}

#[test]
fn capture_rejects_non_hand_piece_and_hand_overflow_before_mutation() {
    for captured in [Piece::W_KING, Piece::W_PAWN] {
        let mut state = generate_position_state(&Position::empty());
        state.board.set(Square::from_usi("5e").unwrap(), Piece::B_ROOK);
        state.board.set(Square::from_usi("5d").unwrap(), captured);
        if captured == Piece::W_PAWN {
            state.hands[Color::BLACK.to_index()].add(HandPiece::PAWN, 31);
        }
        let mut pos = Position::empty();
        pos.set_position_state(&state);
        let mv = pos.move32_from_move(Move::from_usi("5e5d").unwrap());
        assert_update_error(&mut pos, MoveError::InvalidMove, |p| p.try_apply_move32(mv));
    }
}

#[test]
fn position_state_import_rejects_invalid_representation_before_mutation() {
    let mut pos = hirate_position();
    pos.apply_move(Move::from_usi("7g7f").unwrap());
    let before = generate_position_state(&pos);
    let key = pos.key();
    let square = Square::from_usi("5e").unwrap();
    for piece in [Piece::B_GOLD_LIKE, Piece::W_GOLD_LIKE, Piece::new(127)] {
        let mut state = before;
        state.board.set(square, piece);
        assert_eq!(
            pos.try_set_position_state(&state),
            Err(PositionStateError::InvalidPiece { square, piece })
        );
        assert_eq!(generate_position_state(&pos), before);
        assert_eq!(pos.key(), key);
        assert_eq!(pos.state_stack_depth(), 1);
    }
    let mut state = before;
    state.hands[0] = Hand::from_bits(1 << 5);
    assert_eq!(
        pos.try_set_position_state(&state),
        Err(PositionStateError::InvalidHand(Color::BLACK))
    );
    assert_eq!(generate_position_state(&pos), before);
    assert_eq!(pos.key(), key);
    pos.undo_move32(pos.last_move()).unwrap();
    assert_eq!(generate_position_state(&pos), generate_position_state(&hirate_position()));
}

#[test]
fn ply_limits_reject_before_mutation_and_keep_maximum_ply_undoable() {
    let mut state = generate_position_state(&hirate_position());
    state.ply = u16::MAX - 1;
    let mut pos = Position::empty();
    pos.set_position_state(&state);
    let mv = pos.move32_from_move(Move::from_usi("7g7f").unwrap());
    let facts = pos.try_apply_move32_with_facts(mv).unwrap();
    assert_eq!(facts.mv, mv);
    assert_eq!(pos.game_ply(), u16::MAX);
    let reply = pos.move32_from_move(Move::from_usi("3c3d").unwrap());
    assert_update_error(&mut pos, MoveError::CounterOverflow, |p| p.try_apply_move32(reply));
    assert_update_error(&mut pos, MoveError::CounterOverflow, |p| {
        p.try_apply_search_move32_with_facts(reply, false).map(|_| ())
    });
    assert_update_error(&mut pos, MoveError::CounterOverflow, Position::apply_null_move);
    pos.undo_move32(mv).unwrap();
    assert_eq!(generate_position_state(&pos), state);
    pos.apply_null_move().unwrap();
    assert_eq!(pos.game_ply(), u16::MAX);
    pos.undo_null_move().unwrap();
    assert_eq!(generate_position_state(&pos), state);
}

#[test]
fn history_counters_reject_overflow_before_mutation() {
    let mut pos = hirate_position();
    let mv = pos.move32_from_move(Move::from_usi("7g7f").unwrap());
    pos.state_stack_mut().current_mut().plies_from_null = u16::MAX;
    assert_update_error(&mut pos, MoveError::CounterOverflow, |p| p.try_apply_move32(mv));

    let mut pos = Position::from_sfen("4k4/9/9/3R5/9/9/9/9/4K4 b - 1").unwrap();
    let mv = pos.move32_from_move(Move::from_usi("6d5d").unwrap());
    assert!(pos.gives_check_move32(mv));
    pos.state_stack_mut().current_mut().continuous_check[Color::BLACK.to_index()] = u16::MAX - 1;
    assert_update_error(&mut pos, MoveError::CounterOverflow, |p| p.try_apply_move32(mv));
}
