use super::*;
use crate::board::{Move32List, MoveList};
use crate::types::Piece;

fn sorted_usi_move_list(list: &MoveList) -> Vec<String> {
    let mut moves = list.iter().map(|mv| mv.to_usi()).collect::<Vec<_>>();
    moves.sort();
    moves
}

fn sorted_usi_move32_list(list: &Move32List) -> Vec<String> {
    let mut moves = list.iter().map(|mv| mv.to_usi()).collect::<Vec<_>>();
    moves.sort();
    moves
}

#[test]
fn test_generate_checks_move32_matches_move_list_output() {
    let sfen = "4k4/9/9/9/9/4R4/9/9/4K4 b BGSNLP 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid sfen");

    let mut move_list = MoveList::new();
    generate_checks(&pos, &mut move_list);

    let mut move32_list = Move32List::new();
    generate_checks_move32(&pos, &mut move32_list);

    assert_eq!(sorted_usi_move_list(&move_list), sorted_usi_move32_list(&move32_list));
    assert!(move32_list.iter().all(|mv| mv.piece_after_move() != Piece::NONE));
}

#[test]
fn test_generate_quiet_checks_move32_matches_move_list_output() {
    let sfen = "4k4/9/9/9/9/4R4/9/9/4K4 b BGSNLP 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid sfen");

    let mut move_list = MoveList::new();
    generate_quiet_checks(&pos, &mut move_list);

    let mut move32_list = Move32List::new();
    generate_quiet_checks_move32(&pos, &mut move32_list);

    assert_eq!(sorted_usi_move_list(&move_list), sorted_usi_move32_list(&move32_list));
    assert!(move32_list.iter().all(|mv| mv.piece_after_move() != Piece::NONE));
}

#[test]
fn test_generate_checks_drops_part_move32_matches_move_list_output() {
    let sfen = "4k4/9/9/9/9/9/9/9/4K4 b RBGSNLP 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid sfen");

    let mut move_list = MoveList::new();
    generate_checks_drops_part(&pos, &mut move_list);

    let mut move32_list = Move32List::new();
    generate_checks_drops_part_move32(&pos, &mut move32_list);

    assert_eq!(sorted_usi_move_list(&move_list), sorted_usi_move32_list(&move32_list));
    assert!(move32_list.iter().all(|mv| mv.piece_after_move() != Piece::NONE));
    assert!(move32_list.iter().all(|mv| mv.is_drop()));
}

#[test]
fn test_generate_evasions_move32_matches_move_list_output() {
    let sfen = "l4S2l/4g1gs1/5p1p1/pr2N1pkp/4Gn3/PP3PPPP/2GPP4/1K2r4/L4+s2L b BS2N5Pb 2";
    let pos = crate::board::position_from_sfen(sfen).expect("valid sfen");

    let mut move_list = MoveList::new();
    generate_evasions(&pos, &mut move_list);

    let mut move32_list = Move32List::new();
    generate_evasions_move32(&pos, &mut move32_list);

    assert_eq!(sorted_usi_move_list(&move_list), sorted_usi_move32_list(&move32_list));
    assert!(move32_list.iter().all(|mv| mv.piece_after_move() != Piece::NONE));
}
