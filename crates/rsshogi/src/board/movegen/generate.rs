use crate::board::{Move32List, MoveList, Position};
use crate::types::Square;

use super::drops::generate_drops;
use super::pieces::generate_board_moves;
use super::{
    Evasions, EvasionsAll, LegalAll, Move32Sink, Move32SinkAdapter, MoveGenType, MoveSink,
};

pub(super) fn generate_into<T: MoveGenType>(
    pos: &Position,
    only_to: Option<Square>,
    list: &mut impl MoveSink,
) {
    if T::EVASIONS && pos.checkers().is_empty() {
        return;
    }
    generate_board_moves::<T>(pos, only_to, list);
    if only_to.is_none() && !list.stop() {
        generate_drops::<T>(pos, list);
    }
}

pub fn generate_moves<T: MoveGenType>(pos: &Position, list: &mut MoveList) {
    list.clear();
    generate_into::<T>(pos, None, list);
}

pub fn generate_moves_to<T: MoveGenType>(pos: &Position, to: Square, list: &mut MoveList) {
    list.clear();
    generate_into::<T>(pos, Some(to), list);
}

pub fn generate_moves_move32<T: MoveGenType>(pos: &Position, list: &mut Move32List) {
    list.clear();
    generate_moves_move32_into::<T, _>(pos, list);
}

pub fn generate_moves_move32_into<T: MoveGenType, S: Move32Sink>(pos: &Position, sink: &mut S) {
    let mut adapter = Move32SinkAdapter { pos, sink };
    generate_into::<T>(pos, None, &mut adapter);
}

pub fn generate_moves_to_move32<T: MoveGenType>(pos: &Position, to: Square, list: &mut Move32List) {
    list.clear();
    generate_moves_to_move32_into::<T, _>(pos, to, list);
}

pub fn generate_moves_to_move32_into<T: MoveGenType, S: Move32Sink>(
    pos: &Position,
    to: Square,
    sink: &mut S,
) {
    let mut adapter = Move32SinkAdapter { pos, sink };
    generate_into::<T>(pos, Some(to), &mut adapter);
}

pub fn generate_legal_all(pos: &Position, list: &mut MoveList) {
    generate_moves::<LegalAll>(pos, list);
}

pub fn generate_legal_all_move32(pos: &Position, list: &mut Move32List) {
    generate_moves_move32::<LegalAll>(pos, list);
}

pub fn generate_legal_all_move32_into<S: Move32Sink>(pos: &Position, sink: &mut S) {
    generate_moves_move32_into::<LegalAll, _>(pos, sink);
}

pub(super) fn generate_evasion_pseudo(pos: &Position, all: bool, list: &mut MoveList) {
    if all {
        generate_moves::<EvasionsAll>(pos, list);
    } else {
        generate_moves::<Evasions>(pos, list);
    }
}

pub(super) fn generate_evasion_legal(pos: &Position, all: bool, list: &mut MoveList) {
    let mut pseudo = MoveList::new();
    generate_evasion_pseudo(pos, all, &mut pseudo);
    list.clear();
    for &mv in pseudo.iter() {
        if pos.is_legal_move(mv) {
            list.push(mv);
        }
    }
}

pub(super) fn generate_evasion_legal_move32<S: Move32Sink>(
    pos: &Position,
    all: bool,
    sink: &mut S,
) {
    let mut moves = MoveList::new();
    generate_evasion_legal(pos, all, &mut moves);
    for &mv in moves.iter() {
        sink.push_move32(pos.move32_from_move(mv));
    }
}
