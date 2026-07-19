use crate::board::attack_tables::{
    GOLD_ATTACKS, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS, SILVER_ATTACKS, bishop_attacks,
    check_candidate_bb, lance_attacks, rook_attacks,
};
use crate::board::{Bitboard, Move32List, Position};
use crate::types::{Color, Piece, PieceType, Rank, Square};

use super::promotions::{can_promote_move, must_promote};
use super::{Black, ColorMarker, Move32SinkAdapter, MoveSink, White};

#[inline]
fn push_move(list: &mut impl MoveSink, from: Square, to: Square, piece: Piece) {
    list.push_normal(from, to, piece);
}

#[inline]
fn push_promote(list: &mut impl MoveSink, from: Square, to: Square, piece: Piece) {
    list.push_promotion(from, to, piece);
}

fn enemy_territory(us: Color) -> Bitboard {
    Bitboard::promotion_zone(us)
}

#[inline]
fn pawn_attacks(color: Color, sq: Square) -> Bitboard {
    PAWN_ATTACKS[sq][color.to_index()]
}

#[inline]
fn knight_attacks(color: Color, sq: Square) -> Bitboard {
    KNIGHT_ATTACKS[sq][color.to_index()]
}

#[inline]
fn silver_attacks(color: Color, sq: Square) -> Bitboard {
    SILVER_ATTACKS[sq][color.to_index()]
}

#[inline]
fn gold_attacks(color: Color, sq: Square) -> Bitboard {
    GOLD_ATTACKS[sq][color.to_index()]
}

#[inline]
fn king_attacks(sq: Square) -> Bitboard {
    KING_ATTACKS[sq]
}

#[inline]
fn horse_attacks(sq: Square, occupied: Bitboard) -> Bitboard {
    bishop_attacks(sq, occupied) | king_attacks(sq)
}

#[inline]
fn dragon_attacks(sq: Square, occupied: Bitboard) -> Bitboard {
    rook_attacks(sq, occupied) | king_attacks(sq)
}

fn can_promote_sq(us: Color, sq: Square) -> bool {
    enemy_territory(us).test(sq)
}

#[allow(clippy::too_many_arguments)]
fn make_move_target_pro(
    piece: Piece,
    us: Color,
    pt: PieceType,
    all: bool,
    promote: bool,
    from: Square,
    target: Bitboard,
    list: &mut impl MoveSink,
) {
    let mut bb = target;
    while let Some(to) = bb.pop_lsb() {
        if promote {
            push_promote(list, from, to, piece);
            continue;
        }

        let allow = if pt == PieceType::PAWN {
            (!all && !can_promote_sq(us, to))
                || (all
                    && to.rank() != if us == Color::BLACK { Rank::RANK_1 } else { Rank::RANK_9 })
        } else if pt == PieceType::LANCE {
            (!all
                && ((us == Color::BLACK && to.rank() >= Rank::RANK_3)
                    || (us == Color::WHITE && to.rank() <= Rank::RANK_7)))
                || (all
                    && to.rank() != if us == Color::BLACK { Rank::RANK_1 } else { Rank::RANK_9 })
        } else if pt == PieceType::KNIGHT {
            (us == Color::BLACK && to.rank() >= Rank::RANK_3)
                || (us == Color::WHITE && to.rank() <= Rank::RANK_7)
        } else if pt == PieceType::SILVER {
            true
        } else if pt == PieceType::BISHOP || pt == PieceType::ROOK {
            !(can_promote_sq(us, from) || can_promote_sq(us, to)) || all
        } else {
            false
        };

        if allow {
            push_move(list, from, to, piece);
        }
    }
}

#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
fn make_move_target_general(
    pos: &Position,
    piece: Piece,
    from: Square,
    target: Bitboard,
    list: &mut impl MoveSink,
    all: bool,
    us: Color,
) {
    let occupied = pos.bitboards().occupied();
    let pt = piece.piece_type();

    let attacks = match pt {
        PieceType::PAWN => pawn_attacks(us, from),
        PieceType::LANCE => lance_attacks(from, occupied, us),
        PieceType::KNIGHT => knight_attacks(us, from),
        PieceType::SILVER => silver_attacks(us, from),
        PieceType::GOLD
        | PieceType::PRO_PAWN
        | PieceType::PRO_LANCE
        | PieceType::PRO_KNIGHT
        | PieceType::PRO_SILVER => gold_attacks(us, from),
        PieceType::BISHOP => bishop_attacks(from, occupied),
        PieceType::ROOK => rook_attacks(from, occupied),
        PieceType::HORSE => horse_attacks(from, occupied),
        PieceType::DRAGON => dragon_attacks(from, occupied),
        PieceType::KING => king_attacks(from),
        _ => Bitboard::EMPTY,
    };

    let mut target = attacks & target;
    if target.is_empty() {
        return;
    }

    match pt {
        PieceType::PAWN => {
            while let Some(to) = target.pop_lsb() {
                if can_promote_move(from, to, us) {
                    push_promote(list, from, to, piece);
                    if all
                        && to.rank() != if us == Color::BLACK { Rank::RANK_1 } else { Rank::RANK_9 }
                    {
                        push_move(list, from, to, piece);
                    }
                } else {
                    push_move(list, from, to, piece);
                }
            }
        }
        PieceType::LANCE => {
            let enemy = enemy_territory(us);
            let mut promote = target & enemy;
            while let Some(to) = promote.pop_lsb() {
                push_promote(list, from, to, piece);
            }
            let non_promote_mask = if all {
                if us == Color::BLACK {
                    Bitboard::ALL.and_not(Bitboard::rank_mask(Rank::RANK_1))
                } else {
                    Bitboard::ALL.and_not(Bitboard::rank_mask(Rank::RANK_9))
                }
            } else if us == Color::BLACK {
                Bitboard::ALL
                    .and_not(Bitboard::rank_mask(Rank::RANK_1))
                    .and_not(Bitboard::rank_mask(Rank::RANK_2))
            } else {
                Bitboard::ALL
                    .and_not(Bitboard::rank_mask(Rank::RANK_8))
                    .and_not(Bitboard::rank_mask(Rank::RANK_9))
            };
            let mut non_promote = target & non_promote_mask;
            while let Some(to) = non_promote.pop_lsb() {
                if must_promote(pt, to, us) {
                    continue;
                }
                push_move(list, from, to, piece);
            }
        }
        PieceType::KNIGHT => {
            while let Some(to) = target.pop_lsb() {
                if can_promote_move(from, to, us) {
                    push_promote(list, from, to, piece);
                }
                if (us == Color::BLACK && to.rank() >= Rank::RANK_3)
                    || (us == Color::WHITE && to.rank() <= Rank::RANK_7)
                {
                    push_move(list, from, to, piece);
                }
            }
        }
        PieceType::SILVER => {
            let enemy = enemy_territory(us);
            if enemy.test(from) {
                let mut bb = target;
                while let Some(to) = bb.pop_lsb() {
                    push_promote(list, from, to, piece);
                    push_move(list, from, to, piece);
                }
            } else {
                let mut promote = target & enemy;
                while let Some(to) = promote.pop_lsb() {
                    push_promote(list, from, to, piece);
                    push_move(list, from, to, piece);
                }
                let mut non_promote = target.and_not(enemy);
                while let Some(to) = non_promote.pop_lsb() {
                    push_move(list, from, to, piece);
                }
            }
        }
        PieceType::BISHOP | PieceType::ROOK => {
            let enemy = enemy_territory(us);
            if enemy.test(from) {
                let mut bb = target;
                while let Some(to) = bb.pop_lsb() {
                    push_promote(list, from, to, piece);
                    if all {
                        push_move(list, from, to, piece);
                    }
                }
            } else {
                let mut promote = target & enemy;
                while let Some(to) = promote.pop_lsb() {
                    push_promote(list, from, to, piece);
                    if all {
                        push_move(list, from, to, piece);
                    }
                }
                let mut non_promote = target.and_not(enemy);
                while let Some(to) = non_promote.pop_lsb() {
                    push_move(list, from, to, piece);
                }
            }
        }
        _ => {
            while let Some(to) = target.pop_lsb() {
                push_move(list, from, to, piece);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn make_move_check(
    pos: &Position,
    piece: Piece,
    from: Square,
    ksq: Square,
    target: Bitboard,
    list: &mut impl MoveSink,
    all: bool,
    us: Color,
    them: Color,
) {
    let pt = piece.piece_type();
    let enemy = enemy_territory(us);
    let occupied = pos.bitboards().occupied();

    match pt {
        PieceType::PAWN => {
            let mut dst = pawn_attacks(us, from) & gold_attacks(them, ksq) & target;
            if !enemy.test(from) {
                dst &= enemy;
            }
            make_move_target_pro(piece, us, pt, all, true, from, dst, list);
            let dst = pawn_attacks(us, from) & pawn_attacks(them, ksq) & target;
            make_move_target_pro(piece, us, pt, all, false, from, dst, list);
        }
        PieceType::KNIGHT => {
            let mut dst = knight_attacks(us, from) & gold_attacks(them, ksq) & target;
            if !enemy.test(from) {
                dst &= enemy;
            }
            make_move_target_pro(piece, us, pt, all, true, from, dst, list);
            let dst = knight_attacks(us, from) & knight_attacks(them, ksq) & target;
            make_move_target_pro(piece, us, pt, all, false, from, dst, list);
        }
        PieceType::SILVER => {
            let mut dst = silver_attacks(us, from) & gold_attacks(them, ksq) & target;
            if !enemy.test(from) {
                dst &= enemy;
            }
            make_move_target_pro(piece, us, pt, all, true, from, dst, list);
            let dst = silver_attacks(us, from) & silver_attacks(them, ksq) & target;
            make_move_target_pro(piece, us, pt, all, false, from, dst, list);
        }
        PieceType::LANCE => {
            let mut dst = lance_attacks(from, occupied, us) & gold_attacks(them, ksq) & target;
            if !enemy.test(from) {
                dst &= enemy;
            }
            make_move_target_pro(piece, us, pt, all, true, from, dst, list);

            if from.file() == ksq.file() {
                let between = Bitboard::between(from, ksq) & occupied;
                if between.count() <= 1 {
                    let dst =
                        pos.bitboards().color_pieces(them) & Bitboard::between(from, ksq) & target;
                    make_move_target_pro(piece, us, pt, all, false, from, dst, list);
                }
            }
        }
        PieceType::BISHOP => {
            let mut dst = bishop_attacks(from, occupied) & horse_attacks(ksq, occupied) & target;
            if !enemy.test(from) {
                dst &= enemy;
            }
            make_move_target_pro(piece, us, pt, all, true, from, dst, list);
            let dst = bishop_attacks(from, occupied) & bishop_attacks(ksq, occupied) & target;
            make_move_target_pro(piece, us, pt, all, false, from, dst, list);
        }
        PieceType::ROOK => {
            let mut dst = rook_attacks(from, occupied) & dragon_attacks(ksq, occupied) & target;
            if !enemy.test(from) {
                dst &= enemy;
            }
            make_move_target_pro(piece, us, pt, all, true, from, dst, list);
            let dst = rook_attacks(from, occupied) & rook_attacks(ksq, occupied) & target;
            make_move_target_pro(piece, us, pt, all, false, from, dst, list);
        }
        PieceType::GOLD
        | PieceType::PRO_PAWN
        | PieceType::PRO_LANCE
        | PieceType::PRO_KNIGHT
        | PieceType::PRO_SILVER => {
            let dst = gold_attacks(us, from) & gold_attacks(them, ksq) & target;
            let mut bb = dst;
            while let Some(to) = bb.pop_lsb() {
                push_move(list, from, to, piece);
            }
        }
        PieceType::HORSE => {
            let dst = horse_attacks(from, occupied) & horse_attacks(ksq, occupied) & target;
            let mut bb = dst;
            while let Some(to) = bb.pop_lsb() {
                push_move(list, from, to, piece);
            }
        }
        PieceType::DRAGON => {
            let dst = dragon_attacks(from, occupied) & dragon_attacks(ksq, occupied) & target;
            let mut bb = dst;
            while let Some(to) = bb.pop_lsb() {
                push_move(list, from, to, piece);
            }
        }
        _ => {}
    }
}

fn generate_checks_internal(
    pos: &Position,
    list: &mut impl MoveSink,
    all: bool,
    quiet_only: bool,
    us: Color,
    them: Color,
) {
    let bb = pos.bitboards();
    let Some(ksq) = bb.pieces_for(PieceType::KING, them).lsb() else {
        return;
    };

    let golds = bb.pieces_for(PieceType::GOLD, us)
        | bb.pieces_for(PieceType::PRO_PAWN, us)
        | bb.pieces_for(PieceType::PRO_LANCE, us)
        | bb.pieces_for(PieceType::PRO_KNIGHT, us)
        | bb.pieces_for(PieceType::PRO_SILVER, us);

    let x = (bb.pieces_for(PieceType::PAWN, us) & check_candidate_bb(us, PieceType::PAWN, ksq))
        | (bb.pieces_for(PieceType::LANCE, us) & check_candidate_bb(us, PieceType::LANCE, ksq))
        | (bb.pieces_for(PieceType::KNIGHT, us) & check_candidate_bb(us, PieceType::KNIGHT, ksq))
        | (bb.pieces_for(PieceType::SILVER, us) & check_candidate_bb(us, PieceType::SILVER, ksq))
        | (golds & check_candidate_bb(us, PieceType::GOLD, ksq))
        | (bb.pieces_for(PieceType::BISHOP, us) & check_candidate_bb(us, PieceType::BISHOP, ksq))
        | (bb.pieces_for(PieceType::ROOK, us) | bb.pieces_for(PieceType::DRAGON, us))
        | (bb.pieces_for(PieceType::HORSE, us) & check_candidate_bb(us, PieceType::ROOK, ksq));

    let y = pos.blockers_for_king(them) & bb.color_pieces(us);

    let target = if quiet_only { bb.empties() } else { !bb.color_pieces(us) };

    let mut src = y;
    while let Some(from) = src.pop_lsb() {
        let piece = pos.piece_on(from);
        let pin_line = Bitboard::line(ksq, from);
        make_move_target_general(pos, piece, from, target.and_not(pin_line), list, all, us);
        if x.test(from) {
            make_move_check(pos, piece, from, ksq, pin_line & target, list, all, us, them);
        }
    }

    let mut src = x.and_not(y);
    while let Some(from) = src.pop_lsb() {
        let piece = pos.piece_on(from);
        make_move_check(pos, piece, from, ksq, target, list, all, us, them);
    }

    match us {
        Color::BLACK => generate_checks_drops_part_color::<Black>(pos, list),
        Color::WHITE => generate_checks_drops_part_color::<White>(pos, list),
    }
}

/// 王手となる指し手を生成
#[inline]
pub fn generate_checks(pos: &Position, list: &mut impl MoveSink) {
    match pos.turn() {
        Color::BLACK => generate_checks_for_color::<Black>(pos, list, false, false),
        Color::WHITE => generate_checks_for_color::<White>(pos, list, false, false),
    }
}

/// 王手となる指し手を `Move32` リストとして生成
#[inline]
pub fn generate_checks_move32(pos: &Position, list: &mut Move32List) {
    let mut adapter = Move32SinkAdapter { pos, sink: list };
    generate_checks(pos, &mut adapter);
}

/// 王手となる静かな手を生成
#[inline]
pub fn generate_quiet_checks(pos: &Position, list: &mut impl MoveSink) {
    match pos.turn() {
        Color::BLACK => generate_checks_for_color::<Black>(pos, list, false, true),
        Color::WHITE => generate_checks_for_color::<White>(pos, list, false, true),
    }
}

/// 王手となる静かな手を `Move32` リストとして生成
#[inline]
pub fn generate_quiet_checks_move32(pos: &Position, list: &mut Move32List) {
    let mut adapter = Move32SinkAdapter { pos, sink: list };
    generate_quiet_checks(pos, &mut adapter);
}

#[inline]
pub fn generate_checks_for_color<C: ColorMarker>(
    pos: &Position,
    list: &mut impl MoveSink,
    all: bool,
    quiet_only: bool,
) {
    generate_checks_internal(pos, list, all, quiet_only, C::COLOR, C::THEM);
}

#[inline]
pub fn generate_checks_drops_part(pos: &Position, list: &mut impl MoveSink) {
    match pos.turn() {
        Color::BLACK => generate_checks_drops_part_color::<Black>(pos, list),
        Color::WHITE => generate_checks_drops_part_color::<White>(pos, list),
    }
}

/// 王手となる打ち手の部分集合を `Move32` リストとして生成
#[inline]
pub fn generate_checks_drops_part_move32(pos: &Position, list: &mut Move32List) {
    let mut adapter = Move32SinkAdapter { pos, sink: list };
    generate_checks_drops_part(pos, &mut adapter);
}

#[inline]
pub fn generate_checks_drops_part_color<C: ColorMarker>(pos: &Position, list: &mut impl MoveSink) {
    use crate::types::HandPiece;

    let us = C::COLOR;
    let bb = pos.bitboards();
    let hand = pos.hand(us);
    let empty = !bb.occupied();

    for piece_type in [
        PieceType::PAWN,
        PieceType::LANCE,
        PieceType::KNIGHT,
        PieceType::SILVER,
        PieceType::GOLD,
        PieceType::BISHOP,
        PieceType::ROOK,
    ] {
        let Some(hand_piece) = HandPiece::from_piece_type(piece_type) else {
            continue;
        };
        if hand.count(hand_piece) == 0 {
            continue;
        }

        let mut target = empty & pos.check_square(piece_type);
        while let Some(to) = target.pop_lsb() {
            if piece_type == PieceType::PAWN && !pos.is_legal_pawn_drop(us, to) {
                continue;
            }
            list.push_drop(piece_type, to, us);
        }
    }
}
