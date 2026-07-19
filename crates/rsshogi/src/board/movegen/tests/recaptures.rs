use super::*;
use crate::board::{Move32List, MoveList};
use crate::types::{Move, PieceType, Square};
use std::str::FromStr;

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
fn test_recaptures_exclude_drop_on_empty_target() {
    let sfen = "4k4/9/9/9/9/9/9/9/4K4 b P 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    let target = Square::from_str("5e").expect("valid square");

    let mut list = MoveList::new();
    generate_recaptures(&pos, target, &mut list);

    let expected = Move::drop(PieceType::PAWN, target);
    assert!(
        !list.iter().any(|m| *m == expected),
        "recaptures should exclude pawn drop to empty target"
    );
}

#[test]
fn test_recaptures_include_pawn_promotion() {
    let sfen = "4k4/9/2P6/9/9/9/9/9/4K4 b - 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    let from = Square::from_str("7c").expect("valid square");
    let target = Square::from_str("7b").expect("valid square");

    let mut list = MoveList::new();
    generate_recaptures(&pos, target, &mut list);

    let promote = Move::promotion(from, target);

    assert!(list.iter().any(|m| *m == promote), "recaptures should include promotable pawn move");
}

#[test]
fn test_recaptures_all_include_pawn_non_promotion() {
    let sfen = "4k4/9/2P6/9/9/9/9/9/4K4 b - 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    let from = Square::from_str("7c").expect("valid square");
    let target = Square::from_str("7b").expect("valid square");

    let mut list = MoveList::new();
    generate_recaptures_all(&pos, target, &mut list);

    let unpromote = Move::normal(from, target);

    assert!(
        list.iter().any(|m| *m == unpromote),
        "recaptures_all should include non-promoted pawn move"
    );
}

#[test]
fn test_generate_recaptures_move32_matches_move_list_output() {
    let sfen = "4k4/9/2P6/9/9/9/9/9/4K4 b - 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    let target = Square::from_str("7b").expect("valid square");

    let mut move_list = MoveList::new();
    generate_recaptures(&pos, target, &mut move_list);

    let mut move32_list = Move32List::new();
    generate_recaptures_move32(&pos, target, &mut move32_list);

    assert_eq!(sorted_usi_move_list(&move_list), sorted_usi_move32_list(&move32_list));
}

#[test]
fn test_generate_recaptures_all_move32_matches_move_list_output() {
    let sfen = "4k4/9/2P6/9/9/9/9/9/4K4 b - 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    let target = Square::from_str("7b").expect("valid square");

    let mut move_list = MoveList::new();
    generate_recaptures_all(&pos, target, &mut move_list);

    let mut move32_list = Move32List::new();
    generate_recaptures_all_move32(&pos, target, &mut move32_list);

    assert_eq!(sorted_usi_move_list(&move_list), sorted_usi_move32_list(&move32_list));
}
