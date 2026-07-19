use crate::board::{Move32List, MoveList, movegen};
use crate::types::{Piece, PieceType};
use std::collections::BTreeSet;

const SFEN_ROOK_CHECK: &str =
    "l4S2l/4g1gs1/5p1p1/pr2N1pkp/4Gn3/PP3PPPP/2GPP4/1K2r4/L4+s2L b BS2N5Pb 2";
const SFEN_PSEUDO_ONLY_EVASION: &str =
    "1r5+Pl/5g1g1/l4pk2/6ppP/2PPPP1g1/pPs4l1/8R/1g7/KNb+n5 b BS4P2s2nl3p 1";
const SFEN_ROOK_ADVANCES: &str =
    "l4S2l/4g1gs1/5p1p1/p3N1pkp/4Gn3/Pr3PPPP/2GPP4/1K7/L3r+s2L b BS2N5Pbp 2";
const SFEN_BISHOP_DROP: &str =
    "l4S2l/4g1gs1/5p1p1/pr2N1pkp/4Gn3/PP3PPPP/2GPP4/1K7/L1b1r+s2L b BS2N5P 2";

#[test]
fn test_evasion_moves_clear_check_and_roundtrip() {
    // この fixture では pseudo/legal 差が出ないことを利用して、
    // 王手回避の apply/undo が整合性を保つことを検証する。
    // `Evasions` 一般が legal-only でないことは
    // `test_evasions_all_is_pseudo_legal_not_legal_only` で別途確認する。
    let mut pos = crate::board::position_from_sfen(SFEN_ROOK_CHECK).expect("parse SFEN");

    let checkers_before = pos.checkers();
    assert!(!checkers_before.is_empty(), "Initial scenario must start in check to test evasions");

    let mut list = MoveList::new();
    movegen::generate_evasions(&pos, &mut list);
    assert!(!list.is_empty(), "Evasion generator should produce at least one move");

    let zobrist_before = pos.key();
    let depth_before = pos.state_stack().depth();

    for &mv in list.iter() {
        let mv_move = pos.move32_from_move(mv);
        pos.apply_move32(mv_move);
        assert!(
            pos.checkers().is_empty(),
            "After applying an evasion move, the side to move must not be in check"
        );
        assert_eq!(
            pos.state_stack().depth(),
            depth_before + 1,
            "StateStack depth should increase after apply_move32"
        );

        pos.undo_move32(mv_move).expect("undo evasion move");
        assert_eq!(
            pos.state_stack().depth(),
            depth_before,
            "StateStack depth should return to baseline after undo_move32"
        );
        assert_eq!(pos.key(), zobrist_before, "Zobrist key must round-trip");
        assert_eq!(pos.checkers(), checkers_before, "Undo should restore original checkers");
    }
}

#[test]
fn test_rook_line_check_generates_expected_evasions() {
    let pos = crate::board::position_from_sfen(SFEN_ROOK_CHECK).expect("parse SFEN");

    let mut list = MoveList::new();
    movegen::generate_evasions(&pos, &mut list);

    assert!(!list.is_empty(), "王手回避手が1手以上生成されるべき");

    let actual: BTreeSet<String> = list.iter().map(|mv| mv.to_usi()).collect();
    let expected: BTreeSet<String> = [
        "8h7i", "8h8g", "8h8i", "8h9g", "N*6h", "S*6h", "B*6h", "7g7h", "P*7h", "N*7h", "S*7h",
        "B*7h",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    assert_eq!(actual, expected, "既知の王手回避手集合と一致するべき");
}

#[test]
fn test_evasions_all_is_pseudo_legal_not_legal_only() {
    let pos = crate::board::position_from_sfen(SFEN_PSEUDO_ONLY_EVASION).expect("parse SFEN");

    let mut pseudo = Move32List::new();
    movegen::generate_moves_move32::<movegen::EvasionsAll>(&pos, &mut pseudo);
    assert_eq!(pseudo.len(), 1, "この局面では pseudo evasion が 1 手だけ出る");

    let mv = pseudo.as_slice()[0];
    assert_eq!(mv.to_usi(), "9i8h");
    assert!(pos.is_pseudo_legal_move32(mv, true), "生成手は pseudo-legal であるべき");
    assert!(!pos.is_legal_after_pseudo_move32(mv), "split legality 後段では違法手として落ちるべき");
    assert!(!pos.is_legal_move32(mv), "full legality でも違法手であるべき");

    let mut legal = Move32List::new();
    movegen::generate_legal_evasions_all_move32(&pos, &mut legal);
    assert!(legal.is_empty(), "legal-only evasions では 0 手になるべき");
}

#[test]
fn test_legal_evasions_all_matches_legal_all_on_known_check_position() {
    let pos = crate::board::position_from_sfen(SFEN_ROOK_CHECK).expect("parse SFEN");

    let mut legal_evasions = Move32List::new();
    movegen::generate_legal_evasions_all_move32(&pos, &mut legal_evasions);

    let mut legal_all = Move32List::new();
    movegen::generate_legal_all_move32(&pos, &mut legal_all);

    let actual: BTreeSet<String> = legal_evasions.iter().map(|mv| mv.to_usi()).collect();
    let expected: BTreeSet<String> = legal_all.iter().map(|mv| mv.to_usi()).collect();

    assert_eq!(actual, expected, "王手局面では legal evasions all と legal all が一致するべき");
    assert_eq!(actual.len(), 12, "既知局面の legal evasions は 12 手のまま");
}

#[test]
fn test_move_side_legal_evasions_match_move32() {
    // 新設した Move 側 legal-evasion API が、対応する Move32 版と同じ手集合を返すことを確認する。
    // SFEN_ROOK_CHECK は legal evasion あり、SFEN_PSEUDO_ONLY_EVASION は legal evasion なし。
    for sfen in [SFEN_ROOK_CHECK, SFEN_PSEUDO_ONLY_EVASION] {
        let pos = crate::board::position_from_sfen(sfen).expect("parse SFEN");
        assert!(!pos.checkers().is_empty(), "fixture は王手局面であること");

        let mut move_list = MoveList::new();
        movegen::generate_legal_evasions(&pos, &mut move_list);
        let mut move32_list = Move32List::new();
        movegen::generate_legal_evasions_move32(&pos, &mut move32_list);
        let move_set: BTreeSet<String> = move_list.iter().map(|mv| mv.to_usi()).collect();
        let move32_set: BTreeSet<String> = move32_list.iter().map(|mv| mv.to_usi()).collect();
        assert_eq!(
            move_set, move32_set,
            "{sfen}: Move 版 legal evasions が Move32 版と一致するべき"
        );

        let mut move_all = MoveList::new();
        movegen::generate_legal_evasions_all(&pos, &mut move_all);
        let mut move32_all = Move32List::new();
        movegen::generate_legal_evasions_all_move32(&pos, &mut move32_all);
        let move_all_set: BTreeSet<String> = move_all.iter().map(|mv| mv.to_usi()).collect();
        let move32_all_set: BTreeSet<String> = move32_all.iter().map(|mv| mv.to_usi()).collect();
        assert_eq!(
            move_all_set, move32_all_set,
            "{sfen}: Move 版 legal evasions all が Move32 版と一致するべき"
        );
    }
}

#[test]
fn test_rook_line_evasions_comprehensive() {
    // 飛車の王手回避：駒取り、合駒、玉移動などすべての種類の回避手を検証
    let pos = crate::board::position_from_sfen(SFEN_ROOK_ADVANCES).expect("parse SFEN");

    let mut list = MoveList::new();
    movegen::generate_evasions(&pos, &mut list);

    assert!(!list.is_empty(), "王手回避手が1手以上生成されるべき");

    // 王手駒を取る手が含まれていることを確認
    let has_capture =
        list.iter().any(|mv| !mv.is_drop() && pos.piece_on(mv.to_sq()) != Piece::NONE);
    assert!(has_capture, "王手をかけている駒を取る回避手が含まれているべき");

    // 合駒となる駒打ちが含まれていることを確認
    assert!(list.iter().any(|mv| mv.is_drop()), "合駒となる駒打ちが生成されるべき");

    // 玉の移動や駒移動による回避も含まれていることを確認
    assert!(list.iter().any(|mv| !mv.is_drop()), "玉の移動や駒移動による回避も含まれるべき");
}

#[test]
fn test_double_check_only_king_moves() {
    let pos = crate::board::position_from_sfen(SFEN_BISHOP_DROP).expect("parse SFEN");

    let mut list = MoveList::new();
    movegen::generate_evasions(&pos, &mut list);

    assert!(!list.is_empty(), "両王手でも最低1手は生成されるべき");

    for mv in list.iter() {
        assert!(!mv.is_drop(), "両王手下では打ち駒による回避は発生しない");
        assert_eq!(
            pos.piece_on(mv.from_sq()).piece_type(),
            PieceType::KING,
            "両王手下では玉の移動のみが許容されるべき"
        );
    }
}
