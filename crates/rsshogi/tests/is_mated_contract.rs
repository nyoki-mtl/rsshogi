//! `Position::is_mated` は合法手を集めてから空判定するのと同じ結果を返す。
//!
//! 早期打ち切りの有無で答えが変わらないことを、代表局面から一定深さまで
//! 実際に指し進めた全ノードで確認する。`is_mated` は内部で `LegalAll` を
//! 打ち切りながら走査するため、`Legal` を集め切る旧実装との同値性も併せて固定する。

use rsshogi::board::{Move32List, Position, generate_legal_all_move32, position_from_sfen};

/// 王手、詰み、駒余り、駒打ちが絡む局面を含む出発点。
const ROOTS: &[&str] = &[
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
    "l4S2l/4g1gs1/5p1p1/pr2N1pkp/4Gn3/PP3PPPP/2GPP4/1K7/L3r+s2L w BS2N5Pb 1",
    "6n1l/2+S1k4/2lp4p/1np1B2b1/3PP4/1N1S3rP/1P2+pPP+p1/1p1G5/3KG2r1 b GSN2L4Pgs2p 1",
    "lnsG5/4g4/prpp1p1pp/1p4p1k/4+B4/2P1P3P/P+b1PSP1L1/4K2SL/2G2G1r1 b SP3nl3p 73",
    "R8/2K1S1SSk/4B4/9/9/9/9/9/1L1L1L3 b RBGSNLP3g3n17p 1",
    "4k4/9/4P4/9/9/9/9/9/4K4 b G2r2b3g4s4n4l17p 1",
];

fn expected_has_legal_move(pos: &Position) -> bool {
    let mut moves = Move32List::new();
    generate_legal_all_move32(pos, &mut moves);
    !moves.is_empty()
}

/// `Legal`（省略あり）と `LegalAll`（完全集合）で「空かどうか」は一致する。
fn expected_has_legal_move_via_legal(pos: &Position) -> bool {
    use rsshogi::board::MoveList;
    use rsshogi::movegen::{Legal, generate_moves};

    let mut moves = MoveList::new();
    generate_moves::<Legal>(pos, &mut moves);
    !moves.is_empty()
}

fn walk(pos: &mut Position, depth: u32, visited: &mut u64) {
    let expected = expected_has_legal_move(pos);
    assert_eq!(
        !pos.is_mated(),
        expected,
        "is_mated disagreed with the collected LegalAll set at {}",
        pos.to_sfen(None)
    );
    assert_eq!(
        expected_has_legal_move_via_legal(pos),
        expected,
        "Legal and LegalAll disagreed on emptiness at {}",
        pos.to_sfen(None)
    );
    *visited += 1;

    if depth == 0 {
        return;
    }

    let mut moves = Move32List::new();
    generate_legal_all_move32(pos, &mut moves);
    let moves = moves.as_slice().to_vec();
    for mv in moves {
        pos.apply_move32(mv);
        walk(pos, depth - 1, visited);
        pos.undo_move32(mv).expect("undo the move just applied");
    }
}

#[test]
fn is_mated_matches_collected_legal_set() {
    rsshogi::board::init();

    let mut visited = 0u64;
    for sfen in ROOTS {
        let mut pos = position_from_sfen(sfen).expect("valid root sfen");
        pos.init_stack();
        walk(&mut pos, 2, &mut visited);
    }

    assert!(visited > 10_000, "expected a meaningful node count, visited {visited}");
}

#[test]
fn is_mated_is_true_only_when_no_legal_move_exists() {
    rsshogi::board::init();

    // 玉が詰んでおり、合法手が存在しない局面。
    let mated = position_from_sfen("4k4/4G4/4P4/9/9/9/9/9/4K4 w G2r2b2g4s4n4l16p 1")
        .expect("valid mated sfen");
    assert!(mated.is_mated());
    assert_eq!(!mated.is_mated(), expected_has_legal_move(&mated));
}
