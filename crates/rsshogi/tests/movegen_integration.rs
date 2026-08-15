use std::collections::BTreeSet;

use rsshogi::board::{self, MoveList, NonEvasionsAll, generate_moves};

#[test]
fn test_startpos_generates_all_legal_moves() {
    let pos = board::hirate_position();
    let mut moves = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut moves);

    let legal: Vec<_> = moves
        .iter()
        .copied()
        .filter(|&mv| {
            let mv_move = pos.move32_from_move(mv);
            mv_move.is_normal() && pos.is_legal_move32(mv_move)
        })
        .collect();
    assert_eq!(legal.len(), 30, "startpos must yield 30 legal moves");

    let unique: BTreeSet<String> = legal.iter().map(|mv| mv.to_usi()).collect();
    assert_eq!(unique.len(), legal.len(), "generated moves must be unique");
}
