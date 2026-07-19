//! 攻撃テーブル - ビルド時に生成されたテーブルを公開
//!
//! - `*_ATTACKS` などの静的テーブル
//! - スライダー利き計算（`lance_attacks` / `bishop_attacks` / `rook_attacks`）
//! - 王手候補マス取得（[`check_candidate_bb`]）

// ビルド時に生成されたテーブルをインクルード
#[allow(clippy::unreadable_literal)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/attack_tables.rs"));
}

pub use generated::{
    BISHOP_BEAMS, BishopBeams, CHECK_CANDIDATE_BB, GOLD_ATTACKS, KING_ATTACKS, KNIGHT_ATTACKS,
    LANCE_BEAMS, LanceBeams, PAWN_ATTACKS, QUGIY_BISHOP_MASK, QUGIY_ROOK_MASK, QUGIY_STEP_ATTACKS,
    ROOK_BEAMS, RookBeams, SILVER_ATTACKS,
};

use crate::board::{Bitboard, Bitboard256};
use crate::types::{Color, PieceType, Square};

// ANCHOR: step_attacks
#[inline]
#[must_use]
pub fn pawn_attacks_unchecked(sq: Square, color: Color) -> Bitboard {
    PAWN_ATTACKS[sq][color.to_index()]
}

#[inline]
#[must_use]
pub fn knight_attacks_unchecked(sq: Square, color: Color) -> Bitboard {
    KNIGHT_ATTACKS[sq][color.to_index()]
}

#[inline]
#[must_use]
pub fn silver_attacks_unchecked(sq: Square, color: Color) -> Bitboard {
    SILVER_ATTACKS[sq][color.to_index()]
}

#[inline]
#[must_use]
pub fn gold_attacks_unchecked(sq: Square, color: Color) -> Bitboard {
    GOLD_ATTACKS[sq][color.to_index()]
}

#[inline]
#[must_use]
pub fn king_attacks_unchecked(sq: Square) -> Bitboard {
    KING_ATTACKS[sq]
}
/// 飛車の StepAttacks（空盤での利き = 4方向ビーム結合）
#[inline]
#[must_use]
pub fn rook_step_attacks(sq: Square) -> Bitboard {
    let b = ROOK_BEAMS[sq.to_index()];
    b.n | b.e | b.s | b.w
}

/// 角の StepAttacks（空盤での利き = 4方向ビーム結合）
#[inline]
#[must_use]
pub fn bishop_step_attacks(sq: Square) -> Bitboard {
    let b = BISHOP_BEAMS[sq.to_index()];
    b.ne | b.se | b.sw | b.nw
}

/// 香車の StepAttacks（空盤での片方向ビーム）
#[inline]
#[must_use]
pub fn lance_step_attacks(sq: Square, color: Color) -> Bitboard {
    let beams = LANCE_BEAMS[sq.to_index()];
    match color {
        Color::BLACK => beams.black,
        _ => beams.white,
    }
}
// ANCHOR_END: step_attacks

#[inline]
const fn check_candidate_index(piece_type: PieceType) -> Option<usize> {
    match piece_type {
        PieceType::PAWN => Some(0),
        PieceType::LANCE => Some(1),
        PieceType::KNIGHT => Some(2),
        PieceType::SILVER => Some(3),
        PieceType::GOLD => Some(4),
        PieceType::BISHOP => Some(5),
        PieceType::ROOK => Some(6),
        _ => None,
    }
}

/// 指定側、駒種、敵玉位置に対する「王手候補マス」のビットボードを返す。
///
/// `PieceType::PAWN..=PieceType::ROOK` 以外は `Bitboard::EMPTY` を返す。
#[inline]
#[must_use]
pub fn check_candidate_bb(us: Color, piece_type: PieceType, king_sq: Square) -> Bitboard {
    let Some(idx) = check_candidate_index(piece_type) else {
        return Bitboard::EMPTY;
    };
    CHECK_CANDIDATE_BB[king_sq.to_index()][idx][us.to_index()]
}

/// 香車の攻撃範囲を計算（Qugiy方式の差分抽出）
#[inline]
#[must_use]
pub fn lance_attacks(sq: Square, occupied: Bitboard, color: Color) -> Bitboard {
    let beams = LANCE_BEAMS[sq.to_index()];
    let part = usize::from(sq.to_index() >= 63);
    let [occ0, occ1] = occupied.parts();

    match color {
        Color::WHITE => {
            let mask = beams.white;
            let [mask0, mask1] = mask.parts();
            let (em, mask_part) =
                if part == 0 { (occ0 & mask0, mask0) } else { (occ1 & mask1, mask1) };
            let t = em.wrapping_sub(1);
            let result = (em ^ t) & mask_part;
            if part == 0 {
                Bitboard::from_parts([result, 0])
            } else {
                Bitboard::from_parts([0, result])
            }
        }
        Color::BLACK => {
            let mask = beams.black;
            let [mask0, mask1] = mask.parts();
            let (mocc, mask_part) =
                if part == 0 { (occ0 & mask0, mask0) } else { (occ1 & mask1, mask1) };
            let msb = (mocc | 1).ilog2();
            let fill = (!0u64) << msb;
            let result = fill & mask_part;
            if part == 0 {
                Bitboard::from_parts([result, 0])
            } else {
                Bitboard::from_parts([0, result])
            }
        }
    }
}

/// 飛車の縦方向利きを計算（香車の先後方向を1回のビーム取得で合成）。
#[inline]
#[must_use]
fn rook_file_attacks(sq: Square, occupied: Bitboard) -> Bitboard {
    let beams = LANCE_BEAMS[sq.to_index()];
    let part = usize::from(sq.to_index() >= 63);
    let [occ0, occ1] = occupied.parts();

    let [white0, white1] = beams.white.parts();
    let (white_occ, white_mask) =
        if part == 0 { (occ0 & white0, white0) } else { (occ1 & white1, white1) };
    let white_attacks = (white_occ ^ white_occ.wrapping_sub(1)) & white_mask;

    let [black0, black1] = beams.black.parts();
    let (black_occ, black_mask) =
        if part == 0 { (occ0 & black0, black0) } else { (occ1 & black1, black1) };
    let black_fill = (!0u64) << (black_occ | 1).ilog2();
    let black_attacks = black_fill & black_mask;

    let attacks = white_attacks | black_attacks;
    if part == 0 { Bitboard::from_parts([attacks, 0]) } else { Bitboard::from_parts([0, attacks]) }
}

/// 角の攻撃範囲を計算（4方向のビームと占有から計算）
#[inline]
#[must_use]
#[allow(clippy::similar_names)]
pub fn bishop_attacks(sq: Square, occupied: Bitboard) -> Bitboard {
    let mask_lo = QUGIY_BISHOP_MASK[sq.to_index()][0];
    let mask_hi = QUGIY_BISHOP_MASK[sq.to_index()][1];

    let occ2 = Bitboard256::splat(occupied);
    let rocc2 = Bitboard256::splat(occupied.byte_reverse());
    let (hi, lo) = Bitboard256::unpack(rocc2, occ2);

    let hi = hi.and(mask_hi);
    let lo = lo.and(mask_lo);
    let (t1, t0) = Bitboard256::decrement(hi, lo);
    let t1 = t1.xor(hi).and(mask_hi);
    let t0 = t0.xor(lo).and(mask_lo);
    let (hi, lo) = Bitboard256::unpack(t1, t0);
    hi.byte_reverse().or(lo).merge()
}

/// 飛車の攻撃範囲を計算（4方向のビームと占有から計算）
#[inline]
#[must_use]
pub fn rook_attacks(sq: Square, occupied: Bitboard) -> Bitboard {
    let attacks = rook_file_attacks(sq, occupied);

    let mask_lo = QUGIY_ROOK_MASK[sq.to_index()][0];
    let mask_hi = QUGIY_ROOK_MASK[sq.to_index()][1];
    let occ_rev = occupied.byte_reverse();
    let (hi, lo) = Bitboard::unpack(occ_rev, occupied);
    let hi = hi.and_raw(mask_hi);
    let lo = lo.and_raw(mask_lo);
    let (t1, t0) = Bitboard::decrement_pair(hi, lo);
    let t1 = t1.xor_raw(hi).and_raw(mask_hi);
    let t0 = t0.xor_raw(lo).and_raw(mask_lo);
    let (hi, lo) = Bitboard::unpack(t1, t0);
    attacks | hi.byte_reverse() | lo
}

#[cfg(test)]
mod tests;
