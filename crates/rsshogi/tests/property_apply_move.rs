use proptest::prelude::*;

use rsshogi::board::{self, MoveList, NonEvasionsAll, generate_moves};
use rsshogi::types::Move32;

proptest! {
    #[test]
    fn test_generated_move_sequences_do_not_panic_on_apply(seq in proptest::collection::vec(any::<u8>(), 0..24)) {
        let mut pos = board::hirate_position();
        let mut history: Vec<Move32> = Vec::new();

        for byte in seq {
            let mut moves = MoveList::new();
            generate_moves::<NonEvasionsAll>(&pos, &mut moves);

            if moves.is_empty() {
                break;
            }

            let legal: Vec<_> = moves
                .iter()
                .copied()
                .filter(|&mv| {
                    let mv_move = pos.move32_from_move(mv);
                    mv_move.is_normal() && pos.is_legal_move32(mv_move)
                })
                .collect();
            if legal.is_empty() {
                break;
            }
            let idx = (byte as usize) % legal.len();
            let mv = legal[idx];
            let mv_move = pos.move32_from_move(mv);
            pos.apply_move32(mv_move);
            history.push(mv_move);
        }

        while let Some(mv) = history.pop() {
            pos.undo_move32(mv).expect("generated move must be undoable");
        }

        // `undo_move32` がルートの Zobrist を復元することを確認する。状態スタックがルートへ戻っているかチェック。
        prop_assert_eq!(pos.state_stack_depth(), 0);
    }
}
