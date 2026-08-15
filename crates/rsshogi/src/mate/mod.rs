use crate::board::movegen::generate_checks_all_move32;
use crate::board::{Move32List, Position, generate_legal_all_move32};
use crate::types::Move32;

/// 現局面での一手詰めを返す。
///
/// 非王手局面では王手候補を直接生成し、王手中では合法な回避手から候補を選ぶ。
/// 各候補の適用後に防御側の合法手を直接生成する。
/// 防御側に合法手がなければ、その候補は一手詰めである。
#[must_use]
pub fn solve_mate_in_one(position: &Position) -> Option<Move32> {
    let mut candidates = Move32List::new();
    let in_check = position.is_in_check();
    if in_check {
        generate_legal_all_move32(position, &mut candidates);
    } else {
        generate_checks_all_move32(position, &mut candidates);
        candidates.retain_unordered(|mv| position.is_legal_move32(mv));
    }

    if candidates.is_empty() {
        return None;
    }

    let mut next = position.clone();
    next.init_stack();
    let mut replies = Move32List::new();

    for &mv in candidates.iter() {
        if in_check && !position.gives_check_move32(mv) {
            continue;
        }

        next.apply_move32_with_gives_check(mv, true);
        generate_legal_all_move32(&next, &mut replies);
        if replies.is_empty() {
            return Some(mv);
        }
        next.undo_move32(mv).expect("mate search must undo the move it just applied");
    }

    None
}

#[cfg(test)]
mod tests {
    use super::solve_mate_in_one;
    use crate::board::Position;

    #[test]
    fn mate_search_preserves_input_and_handles_checked_positions() {
        let position = Position::from_sfen(
            "lnsG5/4g4/prpp1p1pp/1p4p1k/4+B4/2P1P3P/P+b1PSP1L1/4K2SL/2G2G1r1 b SP3nl3p 73",
        )
        .expect("valid mating position");
        let original = position.to_sfen(None);

        assert!(solve_mate_in_one(&position).is_some());
        assert_eq!(position.to_sfen(None), original);

        let checked =
            Position::from_sfen("4k4/9/4R4/9/9/9/9/9/4K4 w - 1").expect("valid checked position");
        assert!(checked.is_in_check());
        assert_eq!(solve_mate_in_one(&checked), None);
    }
}
