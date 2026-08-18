use crate::board::{Move32List, MoveList, movegen};
use crate::types::{Move, Move32, Piece, PieceType};
use std::cell::Cell;
use std::collections::BTreeSet;

const SFEN_ROOK_CHECK: &str =
    "l4S2l/4g1gs1/5p1p1/pr2N1pkp/4Gn3/PP3PPPP/2GPP4/1K2r4/L4+s2L b BS2N5Pb 2";
const SFEN_PSEUDO_ONLY_EVASION: &str =
    "1r5+Pl/5g1g1/l4pk2/6ppP/2PPPP1g1/pPs4l1/8R/1g7/KNb+n5 b BS4P2s2nl3p 1";
const SFEN_ROOK_ADVANCES: &str =
    "l4S2l/4g1gs1/5p1p1/p3N1pkp/4Gn3/Pr3PPPP/2GPP4/1K7/L3r+s2L b BS2N5Pbp 2";
const SFEN_BISHOP_DROP: &str =
    "l4S2l/4g1gs1/5p1p1/pr2N1pkp/4Gn3/PP3PPPP/2GPP4/1K7/L1b1r+s2L b BS2N5P 2";
const SFEN_BLACK_PROMOTION_AND_DROP_EVASION: &str = "4r3k/5S3/9/9/9/9/9/9/4K4 b G 1";
const SFEN_WHITE_PROMOTION_AND_DROP_EVASION: &str = "4K4/9/9/9/9/9/9/5s3/4R3k w g 1";
const SFEN_DOUBLE_CHECK: &str = "4r3k/9/9/9/8b/9/9/9/4K4 b - 1";

#[derive(Default)]
struct StopAfterFirstMoveSink {
    moves: Vec<Move>,
    observed_after_first: Cell<bool>,
}

impl movegen::MoveSink for StopAfterFirstMoveSink {
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

    fn stop(&self) -> bool {
        let stop = !self.moves.is_empty();
        if stop {
            self.observed_after_first.set(true);
        }
        stop
    }
}

#[derive(Default)]
struct StopAfterFirstMove32Sink {
    moves: Vec<Move32>,
    observed_after_first: Cell<bool>,
}

impl movegen::Move32Sink for StopAfterFirstMove32Sink {
    fn push_move32(&mut self, mv: Move32) {
        self.moves.push(mv);
    }

    fn retain_unordered<F>(&mut self, mut f: F)
    where
        F: FnMut(Move32) -> bool,
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

    fn stop(&self) -> bool {
        let stop = !self.moves.is_empty();
        if stop {
            self.observed_after_first.set(true);
        }
        stop
    }
}

fn assert_stop_checkpoint_before_drops(
    observed_after_first: bool,
    emitted: usize,
    complete: usize,
    mut emitted_is_drop: impl Iterator<Item = bool>,
) {
    assert!(observed_after_first, "stop must be observed after the first accepted move");
    assert_ne!(emitted, 0);
    assert!(emitted < complete, "generation must end before the full list");
    assert!(!emitted_is_drop.any(|is_drop| is_drop), "stop must skip the drop phase");
}

#[test]
fn test_legal_evasions_into_preserve_legacy_raw_sequences() {
    for (label, sfen) in [
        ("black-promotion-and-drop", SFEN_BLACK_PROMOTION_AND_DROP_EVASION),
        ("white-promotion-and-drop", SFEN_WHITE_PROMOTION_AND_DROP_EVASION),
        ("double-check", SFEN_DOUBLE_CHECK),
    ] {
        let pos = crate::board::position_from_sfen(sfen).expect("parse SFEN");
        assert!(!pos.checkers().is_empty(), "{label}: fixture must start in check");

        let mut expected = MoveList::new();
        movegen::generate_legal_evasions(&pos, &mut expected);
        if label == "double-check" {
            assert!(pos.checkers().more_than_one(), "fixture must have a double check");
        } else {
            assert!(expected.iter().any(|mv| mv.is_promotion()), "{label}: fixture must promote");
            assert!(expected.iter().any(|mv| mv.is_drop()), "{label}: fixture must have drops");
        }
        let mut actual = MoveList::new();
        movegen::generate_legal_evasions_into(&pos, &mut actual);
        assert_eq!(
            actual.iter().map(|mv| mv.raw()).collect::<Vec<_>>(),
            expected.iter().map(|mv| mv.raw()).collect::<Vec<_>>(),
            "{label}: Move Legal sequence",
        );

        let mut expected_all = MoveList::new();
        movegen::generate_legal_evasions_all(&pos, &mut expected_all);
        let mut actual_all = MoveList::new();
        movegen::generate_legal_evasions_all_into(&pos, &mut actual_all);
        assert_eq!(
            actual_all.iter().map(|mv| mv.raw()).collect::<Vec<_>>(),
            expected_all.iter().map(|mv| mv.raw()).collect::<Vec<_>>(),
            "{label}: Move LegalAll sequence",
        );

        let mut expected32 = Move32List::new();
        movegen::generate_legal_evasions_move32(&pos, &mut expected32);
        let mut actual32 = Move32List::new();
        movegen::generate_legal_evasions_move32_into(&pos, &mut actual32);
        assert_eq!(
            actual32.iter().map(|mv| mv.raw()).collect::<Vec<_>>(),
            expected32.iter().map(|mv| mv.raw()).collect::<Vec<_>>(),
            "{label}: Move32 Legal sequence",
        );

        let mut expected32_all = Move32List::new();
        movegen::generate_legal_evasions_all_move32(&pos, &mut expected32_all);
        let mut actual32_all = Move32List::new();
        movegen::generate_legal_evasions_all_move32_into(&pos, &mut actual32_all);
        assert_eq!(
            actual32_all.iter().map(|mv| mv.raw()).collect::<Vec<_>>(),
            expected32_all.iter().map(|mv| mv.raw()).collect::<Vec<_>>(),
            "{label}: Move32 LegalAll sequence",
        );
    }
}

#[test]
fn test_legal_evasions_into_leave_non_check_sinks_unchanged() {
    let pos = crate::board::hirate_position();
    assert!(pos.checkers().is_empty(), "fixture must not start in check");

    let mut move_sink = MoveList::new();
    move_sink.push(Move::from_raw(1));
    movegen::generate_legal_evasions_into(&pos, &mut move_sink);
    movegen::generate_legal_evasions_all_into(&pos, &mut move_sink);
    assert_eq!(move_sink.as_slice(), &[Move::from_raw(1)]);

    let mut move32_sink = Move32List::new();
    move32_sink.push(Move32::from_raw(1));
    movegen::generate_legal_evasions_move32_into(&pos, &mut move32_sink);
    movegen::generate_legal_evasions_all_move32_into(&pos, &mut move32_sink);
    assert_eq!(move32_sink.as_slice(), &[Move32::from_raw(1)]);
}

#[test]
fn test_legal_evasions_into_stops_at_checkpoints_before_drops() {
    let pos = crate::board::position_from_sfen(SFEN_BLACK_PROMOTION_AND_DROP_EVASION)
        .expect("parse SFEN");

    let mut expected = MoveList::new();
    movegen::generate_legal_evasions(&pos, &mut expected);
    assert!(expected.iter().any(|mv| mv.is_drop()), "fixture must have legal drops");

    let mut sink = StopAfterFirstMoveSink::default();
    movegen::generate_legal_evasions_into(&pos, &mut sink);
    assert_stop_checkpoint_before_drops(
        sink.observed_after_first.get(),
        sink.moves.len(),
        expected.len(),
        sink.moves.iter().map(|mv| mv.is_drop()),
    );

    let mut expected_all = MoveList::new();
    movegen::generate_legal_evasions_all(&pos, &mut expected_all);
    assert!(expected_all.iter().any(|mv| mv.is_drop()), "fixture must have legal drops");

    let mut sink_all = StopAfterFirstMoveSink::default();
    movegen::generate_legal_evasions_all_into(&pos, &mut sink_all);
    assert_stop_checkpoint_before_drops(
        sink_all.observed_after_first.get(),
        sink_all.moves.len(),
        expected_all.len(),
        sink_all.moves.iter().map(|mv| mv.is_drop()),
    );

    let mut expected32 = Move32List::new();
    movegen::generate_legal_evasions_move32(&pos, &mut expected32);
    assert!(expected32.iter().any(|mv| mv.is_drop()), "fixture must have legal drops");

    let mut sink32 = StopAfterFirstMove32Sink::default();
    movegen::generate_legal_evasions_move32_into(&pos, &mut sink32);
    assert_stop_checkpoint_before_drops(
        sink32.observed_after_first.get(),
        sink32.moves.len(),
        expected32.len(),
        sink32.moves.iter().map(|mv| mv.is_drop()),
    );

    let mut expected32_all = Move32List::new();
    movegen::generate_legal_evasions_all_move32(&pos, &mut expected32_all);
    assert!(expected32_all.iter().any(|mv| mv.is_drop()), "fixture must have legal drops");

    let mut sink32_all = StopAfterFirstMove32Sink::default();
    movegen::generate_legal_evasions_all_move32_into(&pos, &mut sink32_all);
    assert_stop_checkpoint_before_drops(
        sink32_all.observed_after_first.get(),
        sink32_all.moves.len(),
        expected32_all.len(),
        sink32_all.moves.iter().map(|mv| mv.is_drop()),
    );
}

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
