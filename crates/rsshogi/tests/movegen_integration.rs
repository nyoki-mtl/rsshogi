use std::collections::BTreeSet;

use serde::Deserialize;

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

#[derive(Deserialize)]
struct LegalFixture {
    sfen: String,
    moves: Vec<String>,
}

#[test]
fn test_yaneuraou_snapshots_match_generated_legal_moves() {
    let fixtures: std::collections::BTreeMap<String, LegalFixture> =
        serde_json::from_str(include_str!("test_data/yaneuraou_legal_moves.json"))
            .expect("valid move fixture json");

    for (name, fixture) in fixtures {
        let pos = board::position_from_sfen(&fixture.sfen).expect("parse sfen");
        let mut moves = MoveList::new();
        generate_moves::<NonEvasionsAll>(&pos, &mut moves);

        let mut generated: Vec<String> = moves
            .iter()
            .copied()
            .filter(|&mv| {
                let mv_move = pos.move32_from_move(mv);
                mv_move.is_normal() && pos.is_legal_move32(mv_move)
            })
            .map(rsshogi::types::Move::to_usi)
            .collect();
        generated.sort();

        let mut expected = fixture.moves.clone();
        expected.sort();

        assert_eq!(generated, expected, "legal move mismatch for {name}");

        assert!(!generated.is_empty(), "fixture {name}: legal move list must not be empty");
    }
}
