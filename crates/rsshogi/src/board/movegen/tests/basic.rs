use super::*;
use crate::board::{Move32List, MoveList};
use crate::types::{Move, Move32};

#[derive(Default)]
struct TestMove32Sink {
    moves: Vec<Move32>,
}

impl Move32Sink for TestMove32Sink {
    fn push_move32(&mut self, mv: Move32) {
        self.moves.push(mv);
    }

    fn retain_unordered<F>(&mut self, mut f: F)
    where
        F: FnMut(Move32) -> bool,
    {
        let mut i = 0;
        while i < self.moves.len() {
            if f(self.moves[i]) {
                i += 1;
            } else {
                self.moves.swap_remove(i);
            }
        }
    }
}

fn sorted_raws(list: &[Move32]) -> Vec<u32> {
    let mut raws = list.iter().map(|mv| mv.raw()).collect::<Vec<_>>();
    raws.sort_unstable();
    raws
}

fn sorted_move_raws(list: &[Move]) -> Vec<u16> {
    let mut raws = list.iter().map(|mv| mv.raw()).collect::<Vec<_>>();
    raws.sort_unstable();
    raws
}

#[test]
fn test_move_gen_type_flags_are_wired() {
    #[allow(clippy::missing_const_for_fn)]
    fn assert_movegen_type<T: MoveGenType>() {}
    assert_movegen_type::<Captures>();
    assert_movegen_type::<Quiets>();
    assert_movegen_type::<Evasions>();
    assert_movegen_type::<NonEvasionsAll>();
    assert_movegen_type::<QuietChecks>();
}

#[test]
fn test_generate_moves_invocations_compile() {
    let pos = crate::board::hirate_position();
    let mut list = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut list);
    generate_moves::<Captures>(&pos, &mut list);
    generate_moves::<Quiets>(&pos, &mut list);
    generate_quiet_checks(&pos, &mut list);
}

#[test]
fn test_generate_legal_all_matches_legal_all_generic() {
    for (label, pos) in [
        ("startpos", crate::board::hirate_position()),
        (
            "underpromotion",
            crate::board::position_from_sfen("4k4/9/9/9/4B4/9/9/9/4K4 b - 1").expect("valid sfen"),
        ),
        (
            "in-check",
            crate::board::position_from_sfen(
                "l4S2l/4g1gs1/5p1p1/pr2N1pkp/4Gn3/PP3PPPP/2GPP4/1K2r4/L4+s2L b BS2N5Pb 2",
            )
            .expect("valid sfen"),
        ),
        (
            "hands-and-drops",
            crate::board::position_from_sfen(
                "lnsgkgsnl/1r5b1/p1pppp1pp/9/7P1/P1P2P3/1P1PP1P1P/1B5R1/LNSGKGSNL b Pp 1",
            )
            .expect("valid sfen"),
        ),
        (
            "white-to-move",
            crate::board::position_from_sfen(
                "lnsgkgsnl/6gb1/pppppp1pp/6p2/9/4B2P1/PPPPPPP1P/7R1/LNSGKGSNL w Rb 1",
            )
            .expect("valid sfen"),
        ),
    ] {
        let mut direct = MoveList::new();
        generate_legal_all(&pos, &mut direct);

        let mut generic = MoveList::new();
        generate_moves::<LegalAll>(&pos, &mut generic);

        assert_eq!(
            sorted_move_raws(direct.as_slice()),
            sorted_move_raws(generic.as_slice()),
            "{label}: generate_legal_all and generate_moves::<LegalAll> must match",
        );
    }
}

#[test]
fn test_generate_moves_move32_into_matches_move32_list() {
    let pos = crate::board::hirate_position();

    let mut expected = Move32List::new();
    generate_moves_move32::<NonEvasionsAll>(&pos, &mut expected);

    let mut sink = TestMove32Sink::default();
    generate_moves_move32_into::<NonEvasionsAll, _>(&pos, &mut sink);

    assert_eq!(sorted_raws(expected.as_slice()), sorted_raws(&sink.moves));
}

#[test]
fn test_generate_legal_all_move32_into_matches_move32_list() {
    let pos =
        crate::board::position_from_sfen("4k4/9/4G4/9/9/9/9/9/4K4 w - 1").expect("valid sfen");

    let mut expected = Move32List::new();
    generate_legal_all_move32(&pos, &mut expected);

    let mut sink = TestMove32Sink::default();
    generate_legal_all_move32_into(&pos, &mut sink);

    assert_eq!(sorted_raws(expected.as_slice()), sorted_raws(&sink.moves));
}

#[test]
fn test_generate_legal_evasions_move32_into_matches_move32_list() {
    let pos = crate::board::position_from_sfen(
        "l4S2l/4g1gs1/5p1p1/pr2N1pkp/4Gn3/PP3PPPP/2GPP4/1K2r4/L4+s2L b BS2N5Pb 2",
    )
    .expect("valid sfen");

    let mut expected = Move32List::new();
    generate_legal_evasions_move32(&pos, &mut expected);

    let mut sink = TestMove32Sink::default();
    generate_legal_evasions_move32_into(&pos, &mut sink);

    assert_eq!(sorted_raws(expected.as_slice()), sorted_raws(&sink.moves));
}

#[test]
fn test_generate_legal_evasions_all_move32_into_matches_move32_list() {
    let pos = crate::board::position_from_sfen(
        "l4S2l/4g1gs1/5p1p1/pr2N1pkp/4Gn3/PP3PPPP/2GPP4/1K2r4/L4+s2L b BS2N5Pb 2",
    )
    .expect("valid sfen");

    let mut expected = Move32List::new();
    generate_legal_evasions_all_move32(&pos, &mut expected);

    let mut sink = TestMove32Sink::default();
    generate_legal_evasions_all_move32_into(&pos, &mut sink);

    assert_eq!(sorted_raws(expected.as_slice()), sorted_raws(&sink.moves));
}
