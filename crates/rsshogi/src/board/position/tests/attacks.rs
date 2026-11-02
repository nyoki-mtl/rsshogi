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

fn assert_attack_existence_matches_piece_attacks(pos: &Position) {
    let occupied = pos.bitboards().occupied();
    for occupancy in [occupied, Bitboard::EMPTY, occupied ^ Bitboard::from_square(SQ_53)] {
        for color in [Color::BLACK, Color::WHITE] {
            let mut expected = Bitboard::EMPTY;
            for from in Square::iter() {
                let piece = pos.piece_on(from);
                if !piece.is_empty() && piece.color() == color {
                    expected |= Position::piece_attacks(piece.piece_type(), from, color, occupancy);
                }
            }
            for to in Square::iter() {
                assert_eq!(
                    pos.is_attacked_by_color(color, to, occupancy),
                    expected.test(to),
                    "attack mismatch: {color:?}, {to:?}, {}",
                    pos.to_sfen(None),
                );
            }
        }
    }
}

#[test]
fn attack_existence_matches_forward_piece_attacks_for_every_square() {
    for sfen in [
        crate::board::STARTPOS_SFEN,
        "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2Sp1/1PPb2P1P/P5GS1/R8/LN4bKL w RGgsn5p 1",
        "4k4/9/1+P+L+N+S4/2+B1+R4/9/2+b1+r4/1+p+l+n+s4/9/4K4 b - 1",
    ] {
        let pos = crate::board::position_from_sfen(sfen).expect("valid attack fixture");
        assert_attack_existence_matches_piece_attacks(&pos);
    }
}

#[test]
fn attack_existence_matches_forward_piece_attacks_during_play() {
    for mut seed in [1_u64, 0x1234_5678, 0xdead_beef, 0x0123_4567_89ab_cdef] {
        let mut pos = crate::board::hirate_position();
        let mut moves = crate::board::Move32List::new();
        for _ in 0..32 {
            assert_attack_existence_matches_piece_attacks(&pos);
            crate::board::generate_legal_all_move32(&pos, &mut moves);
            if moves.is_empty() {
                break;
            }
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let count = u64::try_from(moves.len()).expect("move count fits u64");
            let index = usize::try_from(seed % count).expect("move index fits usize");
            pos.apply_move32(moves.as_slice()[index]);
        }
    }
}
