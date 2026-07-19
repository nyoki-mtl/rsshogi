//! 256 ビットビットボード
//!
//! [`Bitboard256`] は [`Bitboard`] 2 枚分（162 ビット）を 256 ビットに
//! パックした表現。先手と後手の利き情報を同時に処理するなど、
//! 2 つのビットボードをまとめて SIMD 演算する用途で使用する。
//!
//! AVX2 環境では `__m256i` を使用し、それ以外ではスカラー実装に
//! フォールバックする。

use crate::simd::u64x4::U64x4 as SimdU64x4;

use crate::types::Bitboard;

/// 256 ビットビットボード（[`Bitboard`] 2 枚分のパック表現）。
#[repr(align(32))]
#[derive(Clone, Copy, Debug)]
pub struct Bitboard256 {
    inner: SimdU64x4,
}

impl PartialEq for Bitboard256 {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Bitboard256 {}

impl Bitboard256 {
    #[must_use]
    pub const fn new(p0: u64, p1: u64, p2: u64, p3: u64) -> Self {
        Self { inner: SimdU64x4::new(p0, p1, p2, p3) }
    }

    #[must_use]
    pub const fn parts(&self) -> [u64; 4] {
        self.inner.parts()
    }

    #[must_use]
    pub fn from_bitboards(b1: Bitboard, b2: Bitboard) -> Self {
        Self { inner: SimdU64x4::from_bitboards(b1, b2) }
    }

    #[must_use]
    pub fn splat(bb: Bitboard) -> Self {
        Self { inner: SimdU64x4::splat_bitboard(bb) }
    }

    #[must_use]
    pub fn and(self, other: Self) -> Self {
        Self { inner: self.inner.and(other.inner) }
    }

    #[must_use]
    pub fn or(self, other: Self) -> Self {
        Self { inner: self.inner.or(other.inner) }
    }

    #[must_use]
    pub fn xor(self, other: Self) -> Self {
        Self { inner: self.inner.xor(other.inner) }
    }

    #[must_use]
    /// AND NOT 演算（self & !other）
    ///
    /// `!self & other` を返す。
    pub fn and_not(self, other: Self) -> Self {
        Self { inner: self.inner.and_not(other.inner) }
    }

    #[must_use]
    pub fn wrapping_add(self, other: Self) -> Self {
        Self { inner: self.inner.wrapping_add(other.inner) }
    }

    #[must_use]
    pub fn wrapping_sub(self, other: Self) -> Self {
        Self { inner: self.inner.wrapping_sub(other.inner) }
    }

    #[must_use]
    pub fn shift_left(self, shift: u32) -> Self {
        Self { inner: self.inner.shift_left(shift) }
    }

    #[must_use]
    pub fn shift_right(self, shift: u32) -> Self {
        Self { inner: self.inner.shift_right(shift) }
    }

    #[must_use]
    pub fn decrement_each(self) -> Self {
        let [low0, high0, low1, high1] = self.inner.parts();
        let next_low0 = low0.wrapping_sub(1);
        let next_high0 = high0.wrapping_sub(u64::from(low0 == 0));
        let next_low1 = low1.wrapping_sub(1);
        let next_high1 = high1.wrapping_sub(u64::from(low1 == 0));
        Self::new(next_low0, next_high0, next_low1, next_high1)
    }

    #[must_use]
    pub fn byte_reverse(self) -> Self {
        Self { inner: self.inner.byte_reverse() }
    }

    #[must_use]
    pub fn unpack(hi_in: Self, lo_in: Self) -> (Self, Self) {
        let (hi, lo) = SimdU64x4::unpack(hi_in.inner, lo_in.inner);
        (Self { inner: hi }, Self { inner: lo })
    }

    #[must_use]
    pub fn decrement(hi_in: Self, lo_in: Self) -> (Self, Self) {
        let (hi, lo) = SimdU64x4::decrement_pair(hi_in.inner, lo_in.inner);
        (Self { inner: hi }, Self { inner: lo })
    }

    #[must_use]
    pub fn merge(self) -> Bitboard {
        Bitboard::from_parts(self.inner.merge())
    }

    #[must_use]
    pub const fn to_bitboards(self) -> (Bitboard, Bitboard) {
        let [p0, p1, p2, p3] = self.inner.parts();
        (Bitboard::from_parts([p0, p1]), Bitboard::from_parts([p2, p3]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Square;

    #[test]
    fn test_bitboard256_merge_unpack_byte_reverse() {
        let bb1 = Bitboard::from_packed_bits(0x11u128);
        let bb2 = Bitboard::from_packed_bits(0x22u128);
        let merged = Bitboard256::from_bitboards(bb1, bb2).merge();
        let [merged_low, merged_high] = merged.parts();
        let [bb1_low, bb1_high] = bb1.parts();
        let [bb2_low, bb2_high] = bb2.parts();
        assert_eq!(merged_low, bb1_low | bb2_low);
        assert_eq!(merged_high, bb1_high | bb2_high);

        let hi = Bitboard256::new(1, 2, 3, 4);
        let lo = Bitboard256::new(5, 6, 7, 8);
        let (hi_out, lo_out) = Bitboard256::unpack(hi, lo);
        assert_eq!(hi_out.parts(), [6, 2, 8, 4]);
        assert_eq!(lo_out.parts(), [5, 1, 7, 3]);

        let value = Bitboard256::new(
            0x0123_4567_89ab_cdef,
            0x0011_2233_4455_6677,
            0x89ab_cdef_0123_4567,
            0xfedc_ba98_7654_3210,
        );
        let reversed = value.byte_reverse().parts();
        let expected = [
            0x7766_5544_3322_1100,
            0xefcd_ab89_6745_2301,
            0x1032_5476_98ba_dcfe,
            0x6745_2301_efcd_ab89,
        ];
        assert_eq!(reversed, expected);

        let packed = Bitboard256::new(0, 5, 1, 7);
        let decremented = packed.decrement_each().parts();
        assert_eq!(decremented, [u64::MAX, 4, 0, 7]);
    }

    #[test]
    fn test_bitboard256_roundtrip() {
        let bb1 = Bitboard::from_packed_bits(0x11u128);
        let bb2 = Bitboard::from_packed_bits(0x22u128);
        let b256 = Bitboard256::from_bitboards(bb1, bb2);
        let (out1, out2) = b256.to_bitboards();
        assert_eq!(out1, bb1);
        assert_eq!(out2, bb2);
    }

    #[test]
    fn test_bitboard256_to_bitboards_matches_yaneuraou() {
        for sq in 0..81 {
            let sq = i8::try_from(sq).expect("square index within board bounds");
            let b1 = Bitboard::from_square(Square::new(sq));
            let b2 = Bitboard::from_square(Square::new(80 - sq));
            let b256 = Bitboard256::from_bitboards(b1, b2);
            let (out1, out2) = b256.to_bitboards();
            assert_eq!(out1, b1);
            assert_eq!(out2, b2);
        }
    }

    #[test]
    fn test_bitboard256_byte_reverse_matches_yaneuraou() {
        let b1_raw = (0x0123_4567_89ab_cdefu128) | ((0x1234_5678_9abc_def0u128) << 64);
        let b2_raw = (0x2345_6789_abcd_ef01u128) | ((0x3456_789a_bcde_f012u128) << 64);
        let b1 = Bitboard::from_raw_bits_unmasked(b1_raw);
        let b2 = Bitboard::from_raw_bits_unmasked(b2_raw);
        let b256 = Bitboard256::from_bitboards(b1, b2);
        let reversed = b256.byte_reverse();
        assert_eq!(
            reversed.parts(),
            [
                0xf0de_bc9a_7856_3412,
                0xefcd_ab89_6745_2301,
                0x12f0_debc_9a78_5634,
                0x01ef_cdab_8967_4523
            ]
        );
    }
}
