use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use rsshogi::types::moves::MoveType;
use rsshogi::types::{File, Hand, HandPiece, Move, Move32, Piece, PieceType, Rank, Square};

const fn assert_eq_hash<T: Eq + Hash>() {}

#[test]
fn primitive_eq_types_are_hashable() {
    assert_eq_hash::<HandPiece>();
    assert_eq_hash::<Hand>();
    assert_eq_hash::<PieceType>();
    assert_eq_hash::<Piece>();
    assert_eq_hash::<Square>();
    assert_eq_hash::<File>();
    assert_eq_hash::<Rank>();
    assert_eq_hash::<Move>();
    assert_eq_hash::<MoveType>();
    assert_eq_hash::<Move32>();
}

#[test]
fn hash_collections_work_with_hand_move_and_square() {
    let hand = Hand::ZERO.checked_add(HandPiece::PAWN, 2).expect("valid pawn count");
    let mut hand_cache = HashMap::new();
    hand_cache.insert(hand, 123_i32);
    assert_eq!(hand_cache.get(&hand), Some(&123));

    let from = Square::from_file_rank(File::FILE_7, Rank::RANK_7);
    let to = Square::from_file_rank(File::FILE_7, Rank::RANK_6);
    let mv16 = Move::normal(from, to);
    let mv32 = Move32::normal(from, to, Piece::B_PAWN);

    let mut move16_cache = HashMap::new();
    move16_cache.insert(mv16, 1_u8);
    assert_eq!(move16_cache.get(&mv16), Some(&1));

    let mut move32_cache = HashMap::new();
    move32_cache.insert(mv32, 2_u8);
    assert_eq!(move32_cache.get(&mv32), Some(&2));

    let mut squares = HashSet::new();
    squares.insert(to);
    assert!(squares.contains(&to));
}
