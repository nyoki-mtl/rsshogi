use crate::types::{Color, PieceType, Square};

#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
use std::arch::x86_64::{
    _mm_and_si128, _mm_or_si128, _mm_set_epi64x, _mm_set1_epi8, _mm_setr_epi8, _mm_shuffle_epi8,
    _mm_slli_epi16, _mm_srli_epi16, _mm_storeu_si128, _mm256_add_epi64, _mm256_and_si256,
    _mm256_castsi256_si128, _mm256_cmpeq_epi64, _mm256_extracti128_si256, _mm256_set_epi64x,
    _mm256_setzero_si256, _mm256_slli_si256, _mm256_sub_epi64, _mm256_xor_si256,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LanceBeams {
    pub black: Bitboard,
    pub white: Bitboard,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BishopBeams {
    pub ne: Bitboard,
    pub se: Bitboard,
    pub sw: Bitboard,
    pub nw: Bitboard,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RookBeams {
    pub n: Bitboard,
    pub e: Bitboard,
    pub s: Bitboard,
    pub w: Bitboard,
}

#[derive(Clone, Copy)]
struct LanceRayBits {
    black: u128,
    white: u128,
}

#[derive(Clone, Copy)]
struct BishopRayBits {
    ne: u128,
    se: u128,
    sw_reversed: u128,
    nw_reversed: u128,
}

#[derive(Clone, Copy)]
struct RookRayBits {
    e: u128,
    w: u128,
    e_raw: u128,
    s_raw: u128,
    n_reversed: u128,
    w_reversed: u128,
}

include!(concat!(env!("OUT_DIR"), "/attack_tables.rs"));

const fn build_file_attack_table() -> [[u16; 512]; 9] {
    let mut table = [[0u16; 512]; 9];
    let mut origin = 0;
    while origin < 9 {
        let mut occupied = 0;
        while occupied < 512 {
            let mut attacks = 0u16;
            let mut rank = origin as i32 - 1;
            while rank >= 0 {
                let bit = 1u16 << rank;
                attacks |= bit;
                if occupied & bit as usize != 0 {
                    break;
                }
                rank -= 1;
            }
            rank = origin as i32 + 1;
            while rank < 9 {
                let bit = 1u16 << rank;
                attacks |= bit;
                if occupied & bit as usize != 0 {
                    break;
                }
                rank += 1;
            }
            table[origin][occupied] = attacks;
            occupied += 1;
        }
        origin += 1;
    }
    table
}

const FILE_ATTACKS: [[u16; 512]; 9] = build_file_attack_table();

/// Returns the pseudo-legal pawn attacks from `sq` for `color`.
///
/// # Safety
///
/// `sq` must be a valid board square.
#[must_use]
#[inline]
pub unsafe fn pawn_attacks_unchecked(sq: Square, color: Color) -> Bitboard {
    // SAFETY: the caller guarantees `sq` is in the 81-entry generated table;
    // `Color` has exactly two valid discriminants and indexes its two entries.
    unsafe { *PAWN_ATTACKS.as_slice().get_unchecked(sq.to_index()).get_unchecked(color.to_index()) }
}

/// Returns the pseudo-legal knight attacks from `sq` for `color`.
///
/// # Safety
///
/// `sq` must be a valid board square.
#[must_use]
#[inline]
pub unsafe fn knight_attacks_unchecked(sq: Square, color: Color) -> Bitboard {
    // SAFETY: the caller guarantees `sq` is in the 81-entry generated table;
    // `Color` has exactly two valid discriminants and indexes its two entries.
    unsafe {
        *KNIGHT_ATTACKS.as_slice().get_unchecked(sq.to_index()).get_unchecked(color.to_index())
    }
}

/// Returns the pseudo-legal silver attacks from `sq` for `color`.
///
/// # Safety
///
/// `sq` must be a valid board square.
#[must_use]
#[inline]
pub unsafe fn silver_attacks_unchecked(sq: Square, color: Color) -> Bitboard {
    // SAFETY: the caller guarantees `sq` is in the 81-entry generated table;
    // `Color` has exactly two valid discriminants and indexes its two entries.
    unsafe {
        *SILVER_ATTACKS.as_slice().get_unchecked(sq.to_index()).get_unchecked(color.to_index())
    }
}

/// Returns the pseudo-legal gold attacks from `sq` for `color`.
///
/// # Safety
///
/// `sq` must be a valid board square.
#[must_use]
#[inline]
pub unsafe fn gold_attacks_unchecked(sq: Square, color: Color) -> Bitboard {
    // SAFETY: the caller guarantees `sq` is in the 81-entry generated table;
    // `Color` has exactly two valid discriminants and indexes its two entries.
    unsafe { *GOLD_ATTACKS.as_slice().get_unchecked(sq.to_index()).get_unchecked(color.to_index()) }
}

/// Returns the pseudo-legal king attacks from `sq`.
///
/// # Safety
///
/// `sq` must be a valid board square.
#[must_use]
#[inline]
pub unsafe fn king_attacks_unchecked(sq: Square) -> Bitboard {
    // SAFETY: the caller guarantees `sq` is in the 81-entry generated table.
    unsafe { *KING_ATTACKS.as_slice().get_unchecked(sq.to_index()) }
}

#[inline]
fn nearest_blocker(ray_bits: u128, occupied_bits: u128, increasing: bool) -> Option<usize> {
    let blockers = ray_bits & occupied_bits;
    if blockers == 0 {
        return None;
    }

    let index = if increasing { blockers.trailing_zeros() } else { 127 - blockers.leading_zeros() };
    Some(index as usize)
}

#[inline]
fn increasing_ray_attacks(ray_bits: u128, occupied_bits: u128) -> u128 {
    let blockers = ray_bits & occupied_bits;
    ray_bits & (blockers ^ blockers.wrapping_sub(1))
}

#[inline]
fn paired_increasing_attacks(first_ray: u128, second_ray: u128, occupied_bits: u128) -> u128 {
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        let rays = unsafe {
            _mm256_set_epi64x(
                (second_ray >> 64) as i64,
                second_ray as i64,
                (first_ray >> 64) as i64,
                first_ray as i64,
            )
        };
        let occupied = unsafe {
            _mm256_set_epi64x(
                (occupied_bits >> 64) as i64,
                occupied_bits as i64,
                (occupied_bits >> 64) as i64,
                occupied_bits as i64,
            )
        };
        let blockers = unsafe { _mm256_and_si256(rays, occupied) };
        let one_in_low_lanes = unsafe { _mm256_set_epi64x(0, 1, 0, 1) };
        let low_lane_mask = unsafe { _mm256_set_epi64x(0, -1, 0, -1) };
        let zero_low_lanes = unsafe {
            _mm256_and_si256(_mm256_cmpeq_epi64(blockers, _mm256_setzero_si256()), low_lane_mask)
        };
        let borrow_into_high_lanes = unsafe { _mm256_slli_si256::<8>(zero_low_lanes) };
        let decremented = unsafe {
            _mm256_add_epi64(_mm256_sub_epi64(blockers, one_in_low_lanes), borrow_into_high_lanes)
        };
        let attacks = unsafe { _mm256_and_si256(rays, _mm256_xor_si256(blockers, decremented)) };
        let combined = unsafe {
            _mm_or_si128(_mm256_castsi256_si128(attacks), _mm256_extracti128_si256::<1>(attacks))
        };
        let mut parts = [0u64; 2];
        unsafe { _mm_storeu_si128(parts.as_mut_ptr().cast(), combined) };
        (parts[0] as u128) | ((parts[1] as u128) << 64)
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "avx2")))]
    {
        increasing_ray_attacks(first_ray, occupied_bits)
            | increasing_ray_attacks(second_ray, occupied_bits)
    }
}

#[inline]
fn reverse_bits(value: u128) -> u128 {
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        let bytes = unsafe { _mm_set_epi64x((value >> 64) as i64, value as i64) };
        let reverse_bytes =
            unsafe { _mm_setr_epi8(15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0) };
        let reversed_bytes = unsafe { _mm_shuffle_epi8(bytes, reverse_bytes) };
        let nibble_mask = unsafe { _mm_set1_epi8(0x0f) };
        let reverse_nibbles =
            unsafe { _mm_setr_epi8(0, 8, 4, 12, 2, 10, 6, 14, 1, 9, 5, 13, 3, 11, 7, 15) };
        let low_nibbles = unsafe { _mm_and_si128(reversed_bytes, nibble_mask) };
        let high_nibbles =
            unsafe { _mm_and_si128(_mm_srli_epi16::<4>(reversed_bytes), nibble_mask) };
        let reversed_low = unsafe { _mm_shuffle_epi8(reverse_nibbles, low_nibbles) };
        let reversed_high = unsafe { _mm_shuffle_epi8(reverse_nibbles, high_nibbles) };
        let result = unsafe { _mm_or_si128(_mm_slli_epi16::<4>(reversed_low), reversed_high) };
        let mut parts = [0u64; 2];
        unsafe { _mm_storeu_si128(parts.as_mut_ptr().cast(), result) };
        (parts[0] as u128) | ((parts[1] as u128) << 64)
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "avx2")))]
    {
        value.reverse_bits()
    }
}

#[inline]
fn file_attacks_bits(sq: Square, occupied_bits: u128) -> u128 {
    let square = sq.raw() as u32;
    let file_start = square / 9 * 9;
    let rank = (square - file_start) as usize;
    let file_occupied = ((occupied_bits >> file_start) & 0x1ff) as usize;
    (FILE_ATTACKS[rank][file_occupied] as u128) << file_start
}

#[must_use]
#[inline]
pub fn lance_step_attacks(sq: Square, color: Color) -> Bitboard {
    let beams = LANCE_BEAMS[sq.to_index()];
    match color {
        Color::BLACK => beams.black,
        Color::WHITE => beams.white,
    }
}

#[must_use]
#[inline]
pub fn bishop_step_attacks(sq: Square) -> Bitboard {
    let beams = BISHOP_BEAMS[sq.to_index()];
    beams.ne | beams.se | beams.sw | beams.nw
}

#[must_use]
#[inline]
pub fn rook_step_attacks(sq: Square) -> Bitboard {
    let beams = ROOK_BEAMS[sq.to_index()];
    beams.n | beams.e | beams.s | beams.w
}

#[must_use]
#[inline]
pub fn lance_attacks(sq: Square, occupied: Bitboard, color: Color) -> Bitboard {
    let beams = LANCE_RAY_BITS[sq.to_index()];
    let ray = match color {
        Color::BLACK => beams.black,
        Color::WHITE => beams.white,
    };
    let occupied_bits = occupied.packed_bits();
    let attacks = file_attacks_bits(sq, occupied_bits) & ray;
    Bitboard::from_packed_bits(attacks)
}

#[must_use]
#[inline]
pub fn bishop_attacks(sq: Square, occupied: Bitboard) -> Bitboard {
    let beams = BISHOP_RAY_BITS[sq.to_index()];
    let occupied_bits = occupied.raw_bits();
    let forward = paired_increasing_attacks(beams.ne, beams.se, occupied_bits);
    let reverse = paired_increasing_attacks(
        beams.sw_reversed,
        beams.nw_reversed,
        reverse_bits(occupied_bits),
    );
    let attacks = forward | reverse_bits(reverse);
    Bitboard::from_raw_bits_unmasked(attacks)
}

#[must_use]
#[inline]
pub fn rook_file_attacks(sq: Square, occupied: Bitboard) -> Bitboard {
    let occupied_bits = occupied.packed_bits();
    let attacks = file_attacks_bits(sq, occupied_bits);
    Bitboard::from_packed_bits(attacks)
}

#[must_use]
#[inline]
pub fn rook_rank_attacks(sq: Square, occupied: Bitboard) -> Bitboard {
    let beams = ROOK_RAY_BITS[sq.to_index()];
    let occupied_bits = occupied.packed_bits();
    let east = increasing_ray_attacks(beams.e, occupied_bits);
    let west = nearest_blocker(beams.w, occupied_bits, false)
        .map_or(beams.w, |blocker| beams.w ^ ROOK_RAY_BITS[blocker].w);
    let attacks = east | west;
    Bitboard::from_packed_bits(attacks)
}

#[must_use]
#[inline]
pub fn rook_attacks(sq: Square, occupied: Bitboard) -> Bitboard {
    let beams = ROOK_RAY_BITS[sq.to_index()];
    let occupied_bits = occupied.raw_bits();
    let forward = paired_increasing_attacks(beams.e_raw, beams.s_raw, occupied_bits);
    let reverse =
        paired_increasing_attacks(beams.n_reversed, beams.w_reversed, reverse_bits(occupied_bits));
    let attacks = forward | reverse_bits(reverse);
    Bitboard::from_raw_bits_unmasked(attacks)
}

#[must_use]
pub fn check_candidate_bb(us: Color, piece_type: PieceType, king_sq: Square) -> Bitboard {
    let index = match piece_type {
        PieceType::PAWN => 0,
        PieceType::LANCE => 1,
        PieceType::KNIGHT => 2,
        PieceType::SILVER => 3,
        PieceType::GOLD => 4,
        PieceType::BISHOP => 5,
        PieceType::ROOK => 6,
        _ => return Bitboard::EMPTY,
    };
    CHECK_CANDIDATE_BB[king_sq.to_index()][index][us.to_index()]
}

#[cfg(test)]
mod tests;
