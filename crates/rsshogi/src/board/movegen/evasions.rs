use crate::board::{Move32List, MoveList, Position};

use super::generate::{
    generate_evasion_legal, generate_evasion_legal_move32, generate_evasion_pseudo,
    generate_moves_move32_into,
};
use super::{ColorMarker, Legal, LegalAll, Move32Sink, Move32SinkAdapter, MoveSink};

pub fn generate_evasions(pos: &Position, list: &mut MoveList) {
    generate_evasion_pseudo(pos, false, list);
}

pub fn generate_evasions_into<S: MoveSink>(pos: &Position, sink: &mut S) {
    super::generate::generate_into::<super::Evasions>(pos, None, sink);
}

pub fn generate_evasions_all(pos: &Position, list: &mut MoveList) {
    generate_evasion_pseudo(pos, true, list);
}

pub fn generate_evasions_all_into<S: MoveSink>(pos: &Position, sink: &mut S) {
    super::generate::generate_into::<super::EvasionsAll>(pos, None, sink);
}

pub fn generate_evasions_move32(pos: &Position, list: &mut Move32List) {
    list.clear();
    generate_evasions_move32_into(pos, list);
}

pub fn generate_evasions_move32_into<S: Move32Sink>(pos: &Position, sink: &mut S) {
    let mut adapter = Move32SinkAdapter { pos, sink };
    generate_evasions_into(pos, &mut adapter);
}

pub fn generate_evasions_all_move32(pos: &Position, list: &mut Move32List) {
    list.clear();
    generate_evasions_all_move32_into(pos, list);
}

pub fn generate_evasions_all_move32_into<S: Move32Sink>(pos: &Position, sink: &mut S) {
    let mut adapter = Move32SinkAdapter { pos, sink };
    generate_evasions_all_into(pos, &mut adapter);
}

pub fn generate_legal_evasions(pos: &Position, list: &mut MoveList) {
    generate_evasion_legal(pos, false, list);
}

pub fn generate_legal_evasions_into<S: MoveSink>(pos: &Position, sink: &mut S) {
    if pos.checkers().is_empty() {
        return;
    }
    super::generate::generate_into::<Legal>(pos, None, sink);
}

pub fn generate_legal_evasions_all(pos: &Position, list: &mut MoveList) {
    generate_evasion_legal(pos, true, list);
}

pub fn generate_legal_evasions_all_into<S: MoveSink>(pos: &Position, sink: &mut S) {
    if pos.checkers().is_empty() {
        return;
    }
    super::generate::generate_into::<LegalAll>(pos, None, sink);
}

pub fn generate_legal_evasions_move32(pos: &Position, list: &mut Move32List) {
    list.clear();
    generate_evasion_legal_move32(pos, false, list);
}

pub fn generate_legal_evasions_all_move32(pos: &Position, list: &mut Move32List) {
    list.clear();
    generate_evasion_legal_move32(pos, true, list);
}

pub fn generate_legal_evasions_move32_into<S: Move32Sink>(pos: &Position, sink: &mut S) {
    if pos.checkers().is_empty() {
        return;
    }
    generate_moves_move32_into::<Legal, _>(pos, sink);
}

pub fn generate_legal_evasions_all_move32_into<S: Move32Sink>(pos: &Position, sink: &mut S) {
    if pos.checkers().is_empty() {
        return;
    }
    generate_moves_move32_into::<LegalAll, _>(pos, sink);
}

#[inline]
pub fn generate_evasions_for_color<C: ColorMarker>(
    pos: &Position,
    sink: &mut impl MoveSink,
    all: bool,
) {
    debug_assert_eq!(pos.turn(), C::COLOR);
    if all {
        generate_evasions_all_into(pos, sink);
    } else {
        generate_evasions_into(pos, sink);
    }
}
