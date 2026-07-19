use super::{NonEvasionsAll, generate_moves};
use crate::board::MoveList;
use crate::types::{SQ_55, SQ_91};

#[test]
fn test_pro_pawn_moves_from_corner() {
    let pos = crate::board::position_from_sfen("+P8/9/9/9/9/9/9/9/9 b - 1").unwrap();
    let mut list = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut list);

    let moves: Vec<_> = list.iter().filter(|m| m.from_sq() == SQ_91).collect();
    assert_eq!(moves.len(), 2, "盤端では金の利きが2方向（横と後ろ）");
    assert!(moves.iter().all(|m| !m.is_promotion()));
}

#[test]
fn test_pro_lance_moves_from_corner() {
    let pos = crate::board::position_from_sfen("+L8/9/9/9/9/9/9/9/9 b - 1").unwrap();
    let mut list = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut list);

    let moves: Vec<_> = list.iter().filter(|m| m.from_sq() == SQ_91).collect();
    assert_eq!(moves.len(), 2, "盤端では金の利きが2方向（横と後ろ）");
    assert!(moves.iter().all(|m| !m.is_promotion()));
}

#[test]
fn test_pro_knight_moves_from_corner() {
    let pos = crate::board::position_from_sfen("+N8/9/9/9/9/9/9/9/9 b - 1").unwrap();
    let mut list = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut list);

    let moves: Vec<_> = list.iter().filter(|m| m.from_sq() == SQ_91).collect();
    assert_eq!(moves.len(), 2, "盤端では金の利きが2方向（横と後ろ）");
    assert!(moves.iter().all(|m| !m.is_promotion()));
}

#[test]
fn test_pro_silver_moves_from_corner() {
    let pos = crate::board::position_from_sfen("+S8/9/9/9/9/9/9/9/9 b - 1").unwrap();
    let mut list = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut list);

    let moves: Vec<_> = list.iter().filter(|m| m.from_sq() == SQ_91).collect();
    assert_eq!(moves.len(), 2, "盤端では金の利きが2方向（横と後ろ）");
    assert!(moves.iter().all(|m| !m.is_promotion()));
}

#[test]
fn test_horse_moves_combine_bishop_and_king() {
    let pos = crate::board::position_from_sfen("9/9/9/9/4+B4/9/9/9/9 b - 1").unwrap();
    let mut list = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut list);

    let moves: Vec<_> = list.iter().filter(|m| m.from_sq() == SQ_55).collect();
    assert_eq!(moves.len(), 20, "馬は中央から20方向に移動可能");
    assert!(moves.iter().all(|m| !m.is_promotion()));
}

#[test]
fn test_dragon_moves_combine_rook_and_king() {
    let pos = crate::board::position_from_sfen("9/9/9/9/4+R4/9/9/9/9 b - 1").unwrap();
    let mut list = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut list);

    let moves: Vec<_> = list.iter().filter(|m| m.from_sq() == SQ_55).collect();
    assert_eq!(moves.len(), 20, "龍は中央から20方向に移動可能");
    assert!(moves.iter().all(|m| !m.is_promotion()));
}
