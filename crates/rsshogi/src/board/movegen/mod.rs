mod checks;
mod drops;
mod evasions;
mod generate;
mod pieces;
mod promotions;
mod recaptures;
mod types;

use crate::board::{Move32List, MoveList, Position};
use crate::types::{Color, Move, Move32, Piece, PieceType, Square};
use core::marker::PhantomData;

pub use checks::{
    generate_checks, generate_checks_all_move32, generate_checks_drops_part,
    generate_checks_drops_part_move32, generate_checks_for_color, generate_checks_move32,
    generate_quiet_checks, generate_quiet_checks_move32,
};
pub use evasions::{
    generate_evasions, generate_evasions_all, generate_evasions_all_into,
    generate_evasions_all_move32, generate_evasions_all_move32_into, generate_evasions_for_color,
    generate_evasions_into, generate_evasions_move32, generate_evasions_move32_into,
    generate_legal_evasions, generate_legal_evasions_all, generate_legal_evasions_all_into,
    generate_legal_evasions_all_move32, generate_legal_evasions_all_move32_into,
    generate_legal_evasions_into, generate_legal_evasions_move32,
    generate_legal_evasions_move32_into,
};
pub use generate::{
    generate_legal_all, generate_legal_all_move32, generate_legal_all_move32_into, generate_moves,
    generate_moves_move32, generate_moves_move32_into, generate_moves_to, generate_moves_to_move32,
    generate_moves_to_move32_into,
};
pub use recaptures::{
    generate_recaptures, generate_recaptures_all, generate_recaptures_all_move32,
    generate_recaptures_move32,
};
pub use types::*;

pub trait MoveSink {
    fn push_move(&mut self, mv: Move);

    fn retain_unordered<F>(&mut self, f: F)
    where
        F: FnMut(Move) -> bool;

    fn push_normal(&mut self, from: Square, to: Square, _: Piece) {
        self.push_move(Move::normal(from, to));
    }

    fn push_promotion(&mut self, from: Square, to: Square, _: Piece) {
        self.push_move(Move::promotion(from, to));
    }

    fn push_drop(&mut self, piece_type: PieceType, to: Square, _: Color) {
        self.push_move(Move::drop(piece_type, to));
    }

    fn push_drop_targets(
        &mut self,
        piece_type: PieceType,
        mut targets: crate::board::Bitboard,
        color: Color,
    ) {
        while let Some(to) = targets.pop_lsb() {
            self.push_drop(piece_type, to, color);
        }
    }
}

impl MoveSink for MoveList {
    fn push_move(&mut self, mv: Move) {
        self.push(mv);
    }

    fn retain_unordered<F>(&mut self, f: F)
    where
        F: FnMut(Move) -> bool,
    {
        self.retain_unordered(f);
    }

    fn push_drop_targets(
        &mut self,
        piece_type: PieceType,
        mut targets: crate::board::Bitboard,
        _: Color,
    ) {
        self.ensure_additional_capacity(targets.count() as usize);
        while let Some(to) = targets.pop_lsb() {
            // SAFETY: capacity for every set bit was checked once before the loop,
            // and each iteration consumes exactly one of those bits.
            unsafe { self.push_unchecked(Move::drop(piece_type, to)) };
        }
    }
}

pub trait Move32Sink {
    fn push_move32(&mut self, mv: Move32);

    fn retain_unordered<F>(&mut self, f: F)
    where
        F: FnMut(Move32) -> bool;

    fn push_drop_targets(
        &mut self,
        piece_type: PieceType,
        mut targets: crate::board::Bitboard,
        color: Color,
    ) {
        while let Some(to) = targets.pop_lsb() {
            self.push_move32(Move32::drop(piece_type, to, color));
        }
    }
}

/// A fixed-capacity generated move view retained for callers that prefer construction over an
/// out-parameter.
pub struct MoveListGen<T: MoveGenType> {
    moves: MoveList,
    marker: PhantomData<T>,
}

impl<T: MoveGenType> MoveListGen<T> {
    #[must_use]
    pub fn new(pos: &Position) -> Self {
        let mut moves = MoveList::new();
        generate::generate_moves::<T>(pos, &mut moves);
        Self { moves, marker: PhantomData }
    }

    #[must_use]
    pub fn new_with_target(pos: &Position, target: Square) -> Self {
        let mut moves = MoveList::new();
        generate::generate_moves_to::<T>(pos, target, &mut moves);
        Self { moves, marker: PhantomData }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.moves.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }

    #[must_use]
    pub fn contains(&self, mv: Move) -> bool {
        self.moves.as_slice().contains(&mv)
    }

    #[must_use]
    pub fn at(&self, index: usize) -> Option<Move> {
        self.moves.as_slice().get(index).copied()
    }

    pub fn iter(&self) -> core::slice::Iter<'_, Move> {
        self.moves.iter()
    }

    #[must_use]
    pub fn as_slice(&self) -> &[Move] {
        self.moves.as_slice()
    }
}

impl Move32Sink for Move32List {
    fn push_move32(&mut self, mv: Move32) {
        self.push(mv);
    }

    fn retain_unordered<F>(&mut self, f: F)
    where
        F: FnMut(Move32) -> bool,
    {
        self.retain_unordered(f);
    }

    fn push_drop_targets(
        &mut self,
        piece_type: PieceType,
        mut targets: crate::board::Bitboard,
        color: Color,
    ) {
        self.ensure_additional_capacity(targets.count() as usize);
        while let Some(to) = targets.pop_lsb() {
            // SAFETY: capacity for every set bit was checked once before the loop,
            // and each iteration consumes exactly one of those bits.
            unsafe { self.push_unchecked(Move32::drop(piece_type, to, color)) };
        }
    }
}

pub(crate) struct Move32SinkAdapter<'a, S: Move32Sink> {
    pub(crate) pos: &'a Position,
    pub(crate) sink: &'a mut S,
}

impl<S: Move32Sink> MoveSink for Move32SinkAdapter<'_, S> {
    fn push_move(&mut self, mv: Move) {
        self.sink.push_move32(self.pos.move32_from_move(mv));
    }

    fn retain_unordered<F>(&mut self, mut f: F)
    where
        F: FnMut(Move) -> bool,
    {
        self.sink.retain_unordered(|mv| f(mv.to_move()));
    }

    fn push_normal(&mut self, from: Square, to: Square, piece: Piece) {
        self.sink.push_move32(Move32::normal(from, to, piece));
    }

    fn push_promotion(&mut self, from: Square, to: Square, piece: Piece) {
        self.sink.push_move32(Move32::promotion(from, to, piece));
    }

    fn push_drop(&mut self, piece_type: PieceType, to: Square, color: Color) {
        self.sink.push_move32(Move32::drop(piece_type, to, color));
    }

    fn push_drop_targets(
        &mut self,
        piece_type: PieceType,
        targets: crate::board::Bitboard,
        color: Color,
    ) {
        self.sink.push_drop_targets(piece_type, targets, color);
    }
}

pub trait ColorMarker {
    const COLOR: Color;
    const THEM: Color;
}

pub struct Black;
pub struct White;

impl ColorMarker for Black {
    const COLOR: Color = Color::BLACK;
    const THEM: Color = Color::WHITE;
}

impl ColorMarker for White {
    const COLOR: Color = Color::WHITE;
    const THEM: Color = Color::BLACK;
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    #[derive(Default)]
    struct MinimalMoveSink {
        moves: Vec<Move>,
    }

    impl MoveSink for MinimalMoveSink {
        fn push_move(&mut self, mv: Move) {
            self.moves.push(mv);
        }

        fn retain_unordered<F>(&mut self, mut f: F)
        where
            F: FnMut(Move) -> bool,
        {
            let mut index = 0;
            while index < self.moves.len() {
                if f(self.moves[index]) {
                    index += 1;
                } else {
                    self.moves.swap_remove(index);
                }
            }
        }
    }

    fn sorted_raws(moves: &[Move]) -> Vec<u16> {
        let mut raws = moves.iter().map(|mv| mv.raw()).collect::<Vec<_>>();
        raws.sort_unstable();
        raws
    }

    #[test]
    fn minimal_move_sink_matches_fixed_list_set() {
        let pos =
            crate::board::position_from_sfen("4k4/9/9/9/4B4/9/9/9/4K4 b P 1").expect("valid sfen");
        let mut expected = MoveList::new();
        generate_legal_all(&pos, &mut expected);

        let mut sink = MinimalMoveSink::default();
        generate::generate_into::<LegalAll>(&pos, None, &mut sink);

        assert_eq!(sorted_raws(expected.as_slice()), sorted_raws(&sink.moves));
    }

    #[test]
    fn fixed_move_lists_support_stable_and_unordered_retain() {
        let first = Move::from_raw(1);
        let second = Move::from_raw(2);
        let third = Move::from_raw(3);
        let mut moves = MoveList::new();
        moves.push(first);
        moves.push(second);
        moves.push(third);
        moves.retain(|mv| mv.raw() != 2);
        assert_eq!(moves.as_slice(), &[first, third]);

        moves.retain_unordered(|mv| mv.raw() == 3);
        assert_eq!(moves.as_slice(), &[third]);

        let mut moves32 = Move32List::new();
        moves32.push(Move32::from_raw(1));
        moves32.push(Move32::from_raw(2));
        moves32.retain_unordered(|mv| mv.raw() == 2);
        assert_eq!(moves32.as_slice(), &[Move32::from_raw(2)]);
    }

    #[test]
    fn move_list_gen_accessors_match_target_generation() {
        let pos = crate::board::hirate_position();
        let all = MoveListGen::<LegalAll>::new(&pos);
        assert_eq!(all.len(), all.as_slice().len());
        assert_eq!(all.is_empty(), all.as_slice().is_empty());
        assert_eq!(all.at(all.len()), None);

        let target = all.at(0).expect("start position has legal moves").to_sq();
        let generated = MoveListGen::<LegalAll>::new_with_target(&pos, target);
        assert!(generated.iter().all(|mv| mv.to_sq() == target));
        assert!(generated.iter().all(|&mv| all.contains(mv)));
    }
}

#[cfg(test)]
mod tests;
