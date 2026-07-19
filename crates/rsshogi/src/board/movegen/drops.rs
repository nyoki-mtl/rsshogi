#![allow(clippy::inline_always)]

use crate::board::move_list::Move32List;
use crate::board::{Bitboard, Position};
use crate::types::{Color, Hand, Move, Piece, PieceType};

use super::types::MoveGenType;
use super::{ColorMarker, MoveSink};

#[inline]
fn pawn_drop_mask_for<C: ColorMarker>(pawns: Bitboard) -> Bitboard {
    let left = Bitboard::from_parts([0x4020100804020100, 0x0000000000020100]).packed_bits();
    let board_mask = (1u128 << 81) - 1;
    let mut t = left.wrapping_sub(pawns.packed_bits());
    if C::IS_BLACK {
        t = (t & left) >> 7;
        Bitboard::from_packed_bits(left ^ left.wrapping_sub(t))
    } else {
        t = (t & left) >> 8;
        Bitboard::from_packed_bits((!left & board_mask) & left.wrapping_sub(t))
    }
}

#[derive(Copy, Clone)]
struct DropPattern {
    base: u32,
}

#[derive(Copy, Clone)]
struct NonPawnDropLutEntry {
    order: [u8; 6],
    len: u8,
    // rank1 では香/桂を除外、rank2 では桂を除外するための開始位置。
    rank1_start: u8,
    rank2_start: u8,
}

const NON_PAWN_ORDER_COUNT: usize = 6;
const NON_PAWN_DROP_LUT_TABLE_SIZE: usize = 1 << NON_PAWN_ORDER_COUNT;
const HAND_PAWN_MASK: u32 = 0x1f;
const HAND_LANCE_MASK: u32 = 0x7 << 8;
const HAND_KNIGHT_MASK: u32 = 0x7 << 12;
const HAND_SILVER_MASK: u32 = 0x7 << 16;
const HAND_BISHOP_MASK: u32 = 0x3 << 20;
const HAND_ROOK_MASK: u32 = 0x3 << 24;
const HAND_GOLD_MASK: u32 = 0x7 << 28;
const HAND_NON_PAWN_PRESENCE_BITS: [(u32, usize); NON_PAWN_ORDER_COUNT] = [
    (HAND_KNIGHT_MASK, 0),
    (HAND_LANCE_MASK, 1),
    (HAND_SILVER_MASK, 2),
    (HAND_GOLD_MASK, 3),
    (HAND_BISHOP_MASK, 4),
    (HAND_ROOK_MASK, 5),
];

const fn build_non_pawn_drop_lut_entry(mask: u8) -> NonPawnDropLutEntry {
    let mut order = [0u8; NON_PAWN_ORDER_COUNT];
    let mut len = 0usize;
    let mut idx = 0usize;
    while idx < NON_PAWN_ORDER_COUNT {
        if ((mask >> idx) & 1) != 0 {
            order[len] = idx as u8;
            len += 1;
        }
        idx += 1;
    }

    let rank2_start = if (mask & 0b00_0001) != 0 { 1u8 } else { 0u8 };
    let rank1_start = rank2_start + if (mask & 0b00_0010) != 0 { 1u8 } else { 0u8 };

    NonPawnDropLutEntry { order, len: len as u8, rank1_start, rank2_start }
}

const fn build_non_pawn_drop_lut() -> [NonPawnDropLutEntry; NON_PAWN_DROP_LUT_TABLE_SIZE] {
    let empty = NonPawnDropLutEntry {
        order: [0; NON_PAWN_ORDER_COUNT],
        len: 0,
        rank1_start: 0,
        rank2_start: 0,
    };
    let mut table = [empty; NON_PAWN_DROP_LUT_TABLE_SIZE];
    let mut mask = 0usize;
    while mask < NON_PAWN_DROP_LUT_TABLE_SIZE {
        table[mask] = build_non_pawn_drop_lut_entry(mask as u8);
        mask += 1;
    }
    table
}

const NON_PAWN_DROP_LUT: [NonPawnDropLutEntry; NON_PAWN_DROP_LUT_TABLE_SIZE] =
    build_non_pawn_drop_lut();

#[inline(always)]
fn drop_pattern(piece_type: PieceType, color: Color) -> DropPattern {
    let move_base = u32::from(Move::MOVE_DROP | ((piece_type.to_index() as u16) << 7));
    let piece_bits = u32::from(Piece::from_parts(color, piece_type).raw() as u8 & 0x1f) << 16;
    DropPattern { base: piece_bits | move_base }
}

#[inline]
fn emit_drops_unrolled(
    targets: Bitboard,
    drops: [DropPattern; 6],
    start_idx: usize,
    count: usize,
    list: &mut impl MoveSink,
) {
    if count == 0 {
        return;
    }
    debug_assert!(count <= 6, "drop piece count must be <= 6");
    // `count` は局面ごとに固定なので、match をループの外に出して分岐を削る。
    match count {
        1 => {
            let d0 = drops[start_idx];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                list.push_drop_encoded(d0.base, to);
            }
        }
        2 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                list.push_drop_encoded(d0.base, to);
                list.push_drop_encoded(d1.base, to);
            }
        }
        3 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let d2 = drops[start_idx + 2];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                list.push_drop_encoded(d0.base, to);
                list.push_drop_encoded(d1.base, to);
                list.push_drop_encoded(d2.base, to);
            }
        }
        4 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let d2 = drops[start_idx + 2];
            let d3 = drops[start_idx + 3];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                list.push_drop_encoded(d0.base, to);
                list.push_drop_encoded(d1.base, to);
                list.push_drop_encoded(d2.base, to);
                list.push_drop_encoded(d3.base, to);
            }
        }
        5 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let d2 = drops[start_idx + 2];
            let d3 = drops[start_idx + 3];
            let d4 = drops[start_idx + 4];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                list.push_drop_encoded(d0.base, to);
                list.push_drop_encoded(d1.base, to);
                list.push_drop_encoded(d2.base, to);
                list.push_drop_encoded(d3.base, to);
                list.push_drop_encoded(d4.base, to);
            }
        }
        6 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let d2 = drops[start_idx + 2];
            let d3 = drops[start_idx + 3];
            let d4 = drops[start_idx + 4];
            let d5 = drops[start_idx + 5];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                list.push_drop_encoded(d0.base, to);
                list.push_drop_encoded(d1.base, to);
                list.push_drop_encoded(d2.base, to);
                list.push_drop_encoded(d3.base, to);
                list.push_drop_encoded(d4.base, to);
                list.push_drop_encoded(d5.base, to);
            }
        }
        _ => unsafe {
            // SAFETY: `count` は hand の種類数由来で `1..=6` に限定される。
            core::hint::unreachable_unchecked()
        },
    }
}

#[inline]
fn emit_drops_unrolled_rank1(
    targets: Bitboard,
    drops: [DropPattern; 6],
    start_idx: usize,
    count: usize,
    list: &mut impl MoveSink,
) {
    if count == 0 {
        return;
    }
    debug_assert!(count <= 4, "rank1 drop piece count must be <= 4");
    match count {
        1 => {
            let d0 = drops[start_idx];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                list.push_drop_encoded(d0.base, to);
            }
        }
        2 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                list.push_drop_encoded(d0.base, to);
                list.push_drop_encoded(d1.base, to);
            }
        }
        3 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let d2 = drops[start_idx + 2];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                list.push_drop_encoded(d0.base, to);
                list.push_drop_encoded(d1.base, to);
                list.push_drop_encoded(d2.base, to);
            }
        }
        4 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let d2 = drops[start_idx + 2];
            let d3 = drops[start_idx + 3];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                list.push_drop_encoded(d0.base, to);
                list.push_drop_encoded(d1.base, to);
                list.push_drop_encoded(d2.base, to);
                list.push_drop_encoded(d3.base, to);
            }
        }
        _ => unsafe {
            // SAFETY: rank1 では香/桂が除外されるため `count` は `1..=4` に限定される。
            core::hint::unreachable_unchecked()
        },
    }
}

#[inline]
fn emit_drops_unrolled_rank2(
    targets: Bitboard,
    drops: [DropPattern; 6],
    start_idx: usize,
    count: usize,
    list: &mut impl MoveSink,
) {
    if count == 0 {
        return;
    }
    debug_assert!(count <= 5, "rank2 drop piece count must be <= 5");
    match count {
        1 => {
            let d0 = drops[start_idx];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                list.push_drop_encoded(d0.base, to);
            }
        }
        2 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                list.push_drop_encoded(d0.base, to);
                list.push_drop_encoded(d1.base, to);
            }
        }
        3 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let d2 = drops[start_idx + 2];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                list.push_drop_encoded(d0.base, to);
                list.push_drop_encoded(d1.base, to);
                list.push_drop_encoded(d2.base, to);
            }
        }
        4 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let d2 = drops[start_idx + 2];
            let d3 = drops[start_idx + 3];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                list.push_drop_encoded(d0.base, to);
                list.push_drop_encoded(d1.base, to);
                list.push_drop_encoded(d2.base, to);
                list.push_drop_encoded(d3.base, to);
            }
        }
        5 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let d2 = drops[start_idx + 2];
            let d3 = drops[start_idx + 3];
            let d4 = drops[start_idx + 4];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                list.push_drop_encoded(d0.base, to);
                list.push_drop_encoded(d1.base, to);
                list.push_drop_encoded(d2.base, to);
                list.push_drop_encoded(d3.base, to);
                list.push_drop_encoded(d4.base, to);
            }
        }
        _ => unsafe {
            // SAFETY: rank2 では桂が除外されるため `count` は `1..=5` に限定される。
            core::hint::unreachable_unchecked()
        },
    }
}

#[inline]
fn emit_drops_unrolled_move32(
    targets: Bitboard,
    drops: [u32; 6],
    start_idx: usize,
    count: usize,
    list: &mut Move32List,
) {
    if count == 0 {
        return;
    }
    debug_assert!(count <= 6, "drop piece count must be <= 6");
    match count {
        1 => {
            let d0 = drops[start_idx];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                let to_raw = to.raw() as u32;
                // SAFETY: 手生成内では Move32List 容量超過しない前提のホットパス。
                unsafe { list.push_drop1_unchecked(d0, to_raw) };
            }
        }
        2 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                let to_raw = to.raw() as u32;
                // SAFETY: 手生成内では Move32List 容量超過しない前提のホットパス。
                unsafe { list.push_drop2_unchecked([d0, d1], to_raw) };
            }
        }
        3 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let d2 = drops[start_idx + 2];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                let to_raw = to.raw() as u32;
                // SAFETY: 手生成内では Move32List 容量超過しない前提のホットパス。
                unsafe { list.push_drop3_unchecked([d0, d1, d2], to_raw) };
            }
        }
        4 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let d2 = drops[start_idx + 2];
            let d3 = drops[start_idx + 3];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                let to_raw = to.raw() as u32;
                // SAFETY: 手生成内では Move32List 容量超過しない前提のホットパス。
                unsafe { list.push_drop4_unchecked([d0, d1, d2, d3], to_raw) };
            }
        }
        5 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let d2 = drops[start_idx + 2];
            let d3 = drops[start_idx + 3];
            let d4 = drops[start_idx + 4];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                let to_raw = to.raw() as u32;
                // SAFETY: 手生成内では Move32List 容量超過しない前提のホットパス。
                unsafe { list.push_drop5_unchecked([d0, d1, d2, d3, d4], to_raw) };
            }
        }
        6 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let d2 = drops[start_idx + 2];
            let d3 = drops[start_idx + 3];
            let d4 = drops[start_idx + 4];
            let d5 = drops[start_idx + 5];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                let to_raw = to.raw() as u32;
                // SAFETY: 手生成内では Move32List 容量超過しない前提のホットパス。
                unsafe { list.push_drop6_unchecked([d0, d1, d2, d3, d4, d5], to_raw) };
            }
        }
        _ => unsafe {
            // SAFETY: `count` は hand の種類数由来で `1..=6` に限定される。
            core::hint::unreachable_unchecked()
        },
    }
}

#[inline]
fn emit_drops_unrolled_rank1_move32(
    targets: Bitboard,
    drops: [u32; 6],
    start_idx: usize,
    count: usize,
    list: &mut Move32List,
) {
    if count == 0 {
        return;
    }
    debug_assert!(count <= 4, "rank1 drop piece count must be <= 4");
    match count {
        1 => {
            let d0 = drops[start_idx];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                let to_raw = to.raw() as u32;
                // SAFETY: 手生成内では Move32List 容量超過しない前提のホットパス。
                unsafe { list.push_drop1_unchecked(d0, to_raw) };
            }
        }
        2 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                let to_raw = to.raw() as u32;
                // SAFETY: 手生成内では Move32List 容量超過しない前提のホットパス。
                unsafe { list.push_drop2_unchecked([d0, d1], to_raw) };
            }
        }
        3 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let d2 = drops[start_idx + 2];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                let to_raw = to.raw() as u32;
                // SAFETY: 手生成内では Move32List 容量超過しない前提のホットパス。
                unsafe { list.push_drop3_unchecked([d0, d1, d2], to_raw) };
            }
        }
        4 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let d2 = drops[start_idx + 2];
            let d3 = drops[start_idx + 3];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                let to_raw = to.raw() as u32;
                // SAFETY: 手生成内では Move32List 容量超過しない前提のホットパス。
                unsafe { list.push_drop4_unchecked([d0, d1, d2, d3], to_raw) };
            }
        }
        _ => unsafe {
            // SAFETY: rank1 では香/桂が除外されるため `count` は `1..=4` に限定される。
            core::hint::unreachable_unchecked()
        },
    }
}

#[inline]
fn emit_drops_unrolled_rank2_move32(
    targets: Bitboard,
    drops: [u32; 6],
    start_idx: usize,
    count: usize,
    list: &mut Move32List,
) {
    if count == 0 {
        return;
    }
    debug_assert!(count <= 5, "rank2 drop piece count must be <= 5");
    match count {
        1 => {
            let d0 = drops[start_idx];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                let to_raw = to.raw() as u32;
                // SAFETY: 手生成内では Move32List 容量超過しない前提のホットパス。
                unsafe { list.push_drop1_unchecked(d0, to_raw) };
            }
        }
        2 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                let to_raw = to.raw() as u32;
                // SAFETY: 手生成内では Move32List 容量超過しない前提のホットパス。
                unsafe { list.push_drop2_unchecked([d0, d1], to_raw) };
            }
        }
        3 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let d2 = drops[start_idx + 2];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                let to_raw = to.raw() as u32;
                // SAFETY: 手生成内では Move32List 容量超過しない前提のホットパス。
                unsafe { list.push_drop3_unchecked([d0, d1, d2], to_raw) };
            }
        }
        4 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let d2 = drops[start_idx + 2];
            let d3 = drops[start_idx + 3];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                let to_raw = to.raw() as u32;
                // SAFETY: 手生成内では Move32List 容量超過しない前提のホットパス。
                unsafe { list.push_drop4_unchecked([d0, d1, d2, d3], to_raw) };
            }
        }
        5 => {
            let d0 = drops[start_idx];
            let d1 = drops[start_idx + 1];
            let d2 = drops[start_idx + 2];
            let d3 = drops[start_idx + 3];
            let d4 = drops[start_idx + 4];
            let mut squares = targets;
            while let Some(to) = squares.pop_lsb() {
                let to_raw = to.raw() as u32;
                // SAFETY: 手生成内では Move32List 容量超過しない前提のホットパス。
                unsafe { list.push_drop5_unchecked([d0, d1, d2, d3, d4], to_raw) };
            }
        }
        _ => unsafe {
            // SAFETY: rank2 では桂が除外されるため `count` は `1..=5` に限定される。
            core::hint::unreachable_unchecked()
        },
    }
}

#[inline(never)]
fn generate_pawn_drops_for_target_color<C: ColorMarker>(
    pos: &Position,
    list: &mut impl MoveSink,
    empty_target: Bitboard,
    hand_bits: u32,
) {
    use crate::board::attack_tables::PAWN_ATTACKS;

    if (hand_bits & HAND_PAWN_MASK) == 0 {
        return;
    }

    let us = C::COLOR;
    let them = C::THEM;
    let bb = pos.bitboards();

    let pawns = bb.pieces_for(PieceType::PAWN, us);
    let mut target2 = empty_target & pawn_drop_mask_for::<C>(pawns);
    let pawn_drop_check = bb
        .pieces_for(PieceType::KING, them)
        .lsb()
        .map_or(Bitboard::EMPTY, |king_sq| PAWN_ATTACKS[king_sq][them.to_index()]);
    if !pawn_drop_check.is_empty() {
        let to = pawn_drop_check.lsb().expect("pawn drop check square exists");
        if target2.test(to) && !pos.is_legal_drop(to) {
            target2 = target2.and_not(pawn_drop_check);
        }
    }

    let mut drop_targets = target2;
    while let Some(to) = drop_targets.pop_lsb() {
        list.push_drop(PieceType::PAWN, to, us);
    }
}

#[inline(never)]
fn generate_non_pawn_drops_for_target_color<C: ColorMarker>(
    list: &mut impl MoveSink,
    empty_target: Bitboard,
    hand_bits: u32,
) {
    let us = C::COLOR;

    let mut non_pawn_presence_mask = 0usize;
    for (hand_mask, bit_index) in HAND_NON_PAWN_PRESENCE_BITS {
        non_pawn_presence_mask |= usize::from((hand_bits & hand_mask) != 0) << bit_index;
    }
    let lut = NON_PAWN_DROP_LUT[non_pawn_presence_mask];
    let drops_len = usize::from(lut.len);
    if drops_len == 0 {
        return;
    }

    let ordered_patterns = [
        drop_pattern(PieceType::KNIGHT, us),
        drop_pattern(PieceType::LANCE, us),
        drop_pattern(PieceType::SILVER, us),
        drop_pattern(PieceType::GOLD, us),
        drop_pattern(PieceType::BISHOP, us),
        drop_pattern(PieceType::ROOK, us),
    ];
    let mut drops = [DropPattern { base: 0 }; NON_PAWN_ORDER_COUNT];
    for (drop_idx, drop_slot) in drops.iter_mut().enumerate().take(drops_len) {
        let pattern_idx = usize::from(lut.order[drop_idx]);
        *drop_slot = ordered_patterns[pattern_idx];
    }

    let rank1_start = usize::from(lut.rank1_start);
    if rank1_start == 0 {
        emit_drops_unrolled(empty_target, drops, 0, drops_len, list);
        return;
    }

    let rank1 = if C::IS_BLACK {
        Bitboard::rank_mask(crate::types::Rank::RANK_1)
    } else {
        Bitboard::rank_mask(crate::types::Rank::RANK_9)
    };
    let rank2 = if C::IS_BLACK {
        Bitboard::rank_mask(crate::types::Rank::RANK_2)
    } else {
        Bitboard::rank_mask(crate::types::Rank::RANK_8)
    };
    let rank12 = rank1 | rank2;
    let target1 = empty_target & rank1;
    let target2 = empty_target & rank2;
    let target3_to_9 = empty_target.and_not(rank12);

    emit_drops_unrolled_rank1(target1, drops, rank1_start, drops_len - rank1_start, list);
    let rank2_start = usize::from(lut.rank2_start);
    emit_drops_unrolled_rank2(target2, drops, rank2_start, drops_len - rank2_start, list);
    emit_drops_unrolled(target3_to_9, drops, 0, drops_len, list);
}

#[inline(never)]
fn generate_pawn_drops_for_target_color_move32<C: ColorMarker>(
    pos: &Position,
    list: &mut Move32List,
    empty_target: Bitboard,
    hand_bits: u32,
) {
    use crate::board::attack_tables::PAWN_ATTACKS;

    if (hand_bits & HAND_PAWN_MASK) == 0 {
        return;
    }

    let us = C::COLOR;
    let them = C::THEM;
    let bb = pos.bitboards();

    let pawns = bb.pieces_for(PieceType::PAWN, us);
    let mut target2 = empty_target & pawn_drop_mask_for::<C>(pawns);
    let pawn_drop_check = bb
        .pieces_for(PieceType::KING, them)
        .lsb()
        .map_or(Bitboard::EMPTY, |king_sq| PAWN_ATTACKS[king_sq][them.to_index()]);
    if !pawn_drop_check.is_empty() {
        let to = pawn_drop_check.lsb().expect("pawn drop check square exists");
        if target2.test(to) && !pos.is_legal_drop(to) {
            target2 = target2.and_not(pawn_drop_check);
        }
    }

    let pawn_drop = drop_pattern(PieceType::PAWN, us).base;
    let mut drop_targets = target2;
    while let Some(to) = drop_targets.pop_lsb() {
        let to_raw = to.raw() as u32;
        // SAFETY: 手生成内では Move32List 容量超過しない前提のホットパス。
        unsafe { list.push_drop1_unchecked(pawn_drop, to_raw) };
    }
}

#[inline(never)]
fn generate_non_pawn_drops_for_target_color_move32<C: ColorMarker>(
    list: &mut Move32List,
    empty_target: Bitboard,
    hand_bits: u32,
) {
    let us = C::COLOR;

    let has_knight = (hand_bits & HAND_KNIGHT_MASK) != 0;
    let has_lance = (hand_bits & HAND_LANCE_MASK) != 0;
    let has_silver = (hand_bits & HAND_SILVER_MASK) != 0;
    let has_gold = (hand_bits & HAND_GOLD_MASK) != 0;
    let has_bishop = (hand_bits & HAND_BISHOP_MASK) != 0;
    let has_rook = (hand_bits & HAND_ROOK_MASK) != 0;

    let mut drops = [0u32; NON_PAWN_ORDER_COUNT];
    let mut drops_len = 0usize;
    if has_knight {
        drops[drops_len] = drop_pattern(PieceType::KNIGHT, us).base;
        drops_len += 1;
    }
    if has_lance {
        drops[drops_len] = drop_pattern(PieceType::LANCE, us).base;
        drops_len += 1;
    }
    if has_silver {
        drops[drops_len] = drop_pattern(PieceType::SILVER, us).base;
        drops_len += 1;
    }
    if has_gold {
        drops[drops_len] = drop_pattern(PieceType::GOLD, us).base;
        drops_len += 1;
    }
    if has_bishop {
        drops[drops_len] = drop_pattern(PieceType::BISHOP, us).base;
        drops_len += 1;
    }
    if has_rook {
        drops[drops_len] = drop_pattern(PieceType::ROOK, us).base;
        drops_len += 1;
    }
    if drops_len == 0 {
        return;
    }

    let rank2_start = usize::from(has_knight);
    let rank1_start = rank2_start + usize::from(has_lance);
    if rank1_start == 0 {
        emit_drops_unrolled_move32(empty_target, drops, 0, drops_len, list);
        return;
    }

    let rank1 = if C::IS_BLACK {
        Bitboard::rank_mask(crate::types::Rank::RANK_1)
    } else {
        Bitboard::rank_mask(crate::types::Rank::RANK_9)
    };
    let rank2 = if C::IS_BLACK {
        Bitboard::rank_mask(crate::types::Rank::RANK_2)
    } else {
        Bitboard::rank_mask(crate::types::Rank::RANK_8)
    };
    let rank12 = rank1 | rank2;
    let target1 = empty_target & rank1;
    let target2 = empty_target & rank2;
    let target3_to_9 = empty_target.and_not(rank12);

    emit_drops_unrolled_rank1_move32(target1, drops, rank1_start, drops_len - rank1_start, list);
    emit_drops_unrolled_rank2_move32(target2, drops, rank2_start, drops_len - rank2_start, list);
    emit_drops_unrolled_move32(target3_to_9, drops, 0, drops_len, list);
}

#[allow(clippy::extra_unused_type_parameters, clippy::too_many_lines)]
#[inline]
pub(super) fn generate_drops_color<T: MoveGenType, C: ColorMarker>(
    pos: &Position,
    list: &mut impl MoveSink,
) {
    let empty = !pos.bitboards().occupied();
    generate_drops_for_target_color::<T, C>(pos, list, empty);
}

#[allow(clippy::extra_unused_type_parameters, clippy::too_many_lines)]
#[inline]
pub(super) fn generate_drops_for_target_color<T: MoveGenType, C: ColorMarker>(
    pos: &Position,
    list: &mut impl MoveSink,
    target: Bitboard,
) {
    let us = C::COLOR;
    let hand = pos.hand(us);
    let hand_bits = hand.bits();
    if hand == Hand::ZERO {
        return;
    }

    let empty = !pos.bitboards().occupied();
    let empty_target = empty.and(target);
    if empty_target.is_empty() {
        return;
    }

    generate_pawn_drops_for_target_color::<C>(pos, list, empty_target, hand_bits);
    generate_non_pawn_drops_for_target_color::<C>(list, empty_target, hand_bits);
}

#[allow(clippy::extra_unused_type_parameters, clippy::too_many_lines)]
#[inline]
pub(super) fn generate_drops_color_move32<T: MoveGenType, C: ColorMarker>(
    pos: &Position,
    list: &mut Move32List,
) {
    let empty = !pos.bitboards().occupied();
    generate_drops_for_target_color_move32::<T, C>(pos, list, empty);
}

#[allow(clippy::extra_unused_type_parameters, clippy::too_many_lines)]
#[inline]
fn generate_drops_for_target_color_move32<T: MoveGenType, C: ColorMarker>(
    pos: &Position,
    list: &mut Move32List,
    target: Bitboard,
) {
    let us = C::COLOR;
    let hand = pos.hand(us);
    let hand_bits = hand.bits();
    if hand == Hand::ZERO {
        return;
    }

    let empty = !pos.bitboards().occupied();
    let empty_target = empty.and(target);
    if empty_target.is_empty() {
        return;
    }

    generate_pawn_drops_for_target_color_move32::<C>(pos, list, empty_target, hand_bits);
    generate_non_pawn_drops_for_target_color_move32::<C>(list, empty_target, hand_bits);
}
