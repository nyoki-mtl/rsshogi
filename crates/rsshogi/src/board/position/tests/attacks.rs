use super::*;
use crate::types::Bitboard;
use crate::types::square::{SQ_53, SQ_54, SQ_55};

#[test]
fn test_attackers_to_startpos_is_empty() {
    let pos = crate::board::hirate_position();
    let occupied = pos.bitboards().occupied();

    // 初期局面で5五を攻撃している駒はいない
    let attackers = pos.attackers_to(SQ_55, occupied);
    assert!(attackers.is_empty());
}

#[test]
fn test_square_not_attacked_in_startpos() {
    let pos = crate::board::hirate_position();

    // 初期局面で5五は攻撃されていない
    assert!(!pos.is_attacked_by(SQ_55, Color::BLACK));
    assert!(!pos.is_attacked_by(SQ_55, Color::WHITE));
}

#[test]
fn test_is_attacked_by_color_matches_attackers_to_color_with_custom_occupied() {
    let pos =
        crate::board::position_from_sfen("4k4/9/9/4p4/4R4/9/9/9/4K4 b - 1").expect("valid SFEN");
    let occupied = pos.bitboards().occupied();

    assert!(!pos.is_attacked_by_color(Color::BLACK, SQ_53, occupied));
    assert!(pos.attackers_to_color(Color::BLACK, SQ_53, occupied).is_empty());

    let occupied_without_blocker = occupied.and_not(Bitboard::from_square(SQ_54));
    assert!(pos.is_attacked_by_color(Color::BLACK, SQ_53, occupied_without_blocker));
    assert!(!pos.attackers_to_color(Color::BLACK, SQ_53, occupied_without_blocker).is_empty());
}
