use super::*;
use crate::board::test_support::{apply_usi_move_expect, move_from_usi_expect};

pub(super) fn move_from_usi(pos: &Position, usi: &str) -> Move32 {
    move_from_usi_expect(pos, usi)
}

pub(super) fn apply_usi_move(pos: &mut Position, usi: &str) {
    apply_usi_move_expect(pos, usi);
}
