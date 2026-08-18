use std::collections::HashMap;

use proptest::prelude::*;
use rsshogi::board::{self, Move32List, Position, generate_legal_all_move32, position_from_sfen};
use rsshogi::types::{Ki2Notation, Move};

fn legal_moves(position: &Position) -> Move32List {
    let mut moves = Move32List::new();
    generate_legal_all_move32(position, &mut moves);
    moves
}

fn assert_unique_ki2_notation(position: &Position) {
    let mut seen = HashMap::new();
    for &mv in &legal_moves(position) {
        let notation = mv.to_ki2(position).expect("legal move must have KI2 notation");
        let value =
            mv.to_ki2_notation(position).expect("legal move must have structured KI2 notation");
        assert_eq!(value.to_string(), notation);
        assert_eq!(notation.parse::<Ki2Notation>(), Ok(value));
        if let Some(previous) = seen.insert(notation.clone(), mv) {
            panic!(
                "different legal moves have the same KI2 notation: {previous:?} and {mv:?} -> {notation}; sfen={}",
                position.to_sfen(None)
            );
        }
    }
}

#[test]
fn sideways_gold_moves_are_disambiguated_for_white() {
    board::init();
    let position =
        position_from_sfen("lnsg1gsnl/1r3k1b1/ppppppppp/9/8P/9/PPPPPPPP1/1B5R1/LNSGKGSNL w - 4")
            .expect("valid sfen");

    let left = position.move32_from_move(Move::from_usi("4a5a").expect("valid move"));
    let right = position.move32_from_move(Move::from_usi("6a5a").expect("valid move"));

    assert_eq!(left.to_ki2(&position).as_deref(), Some("△５一金左"));
    assert_eq!(right.to_ki2(&position).as_deref(), Some("△５一金右"));
    assert_unique_ki2_notation(&position);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn legal_moves_have_unique_ki2_notation(
        path in proptest::collection::vec(any::<u16>(), 0..96),
    ) {
        board::init();
        let mut position = board::hirate_position();
        assert_unique_ki2_notation(&position);

        for choice in path {
            let moves = legal_moves(&position);
            if moves.is_empty() {
                break;
            }
            let mv = moves[usize::from(choice) % moves.len()];
            let notation = mv
                .to_ki2_notation(&position)
                .expect("legal move must have structured KI2 notation");
            prop_assert_eq!(notation.to_move32(&position), Ok(mv));
            position.apply_move32(mv);
            assert_unique_ki2_notation(&position);
        }
    }
}
