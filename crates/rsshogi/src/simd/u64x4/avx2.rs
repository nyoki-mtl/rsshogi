#![allow(clippy::missing_const_for_fn)]

use crate::types::Bitboard;

use std::arch::x86_64::{
    __m256i, _mm_cvtsi64_si128, _mm_or_si128, _mm256_add_epi64, _mm256_and_si256,
    _mm256_andnot_si256, _mm256_broadcastsi128_si256, _mm256_castsi128_si256,
    _mm256_castsi256_si128, _mm256_cmpeq_epi64, _mm256_extracti128_si256, _mm256_inserti128_si256,
    _mm256_loadu_si256, _mm256_or_si256, _mm256_set1_epi64x, _mm256_setr_epi8,
    _mm256_setzero_si256, _mm256_shuffle_epi8, _mm256_sll_epi64, _mm256_srl_epi64,
    _mm256_storeu_si256, _mm256_sub_epi64, _mm256_testz_si256, _mm256_unpackhi_epi64,
    _mm256_unpacklo_epi64, _mm256_xor_si256,
};

// このモジュールは `target_arch = "x86_64"` かつ `target_feature = "avx2"` が
// 有効な場合にのみ `simd::u64x4::mod` からコンパイルされる。
// 以下のすべての intrinsic はモジュール境界でガードされている。

#[derive(Copy, Clone)]
#[repr(align(32))]
pub struct U64x4 {
    p: [u64; 4],
}

impl PartialEq for U64x4 {
    fn eq(&self, other: &Self) -> bool {
        // SAFETY: このモジュールは AVX2 でガードされており、両オペランドは初期化済みの
        // `U64x4` に対する 256 ビットレジスタビューである。
        let neq = unsafe { _mm256_xor_si256(self.as_m256(), other.as_m256()) };
        // SAFETY: `neq` は有効なレジスタ値であり、`_mm256_testz_si256` は
        // レジスタオペランドを読み取るだけである。
        unsafe { _mm256_testz_si256(neq, neq) != 0 }
    }
}

impl Eq for U64x4 {}

impl std::fmt::Debug for U64x4 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("U64x4").field(&self.parts()).finish()
    }
}

impl U64x4 {
    #[must_use]
    pub const fn new(p0: u64, p1: u64, p2: u64, p3: u64) -> Self {
        Self { p: [p0, p1, p2, p3] }
    }

    #[must_use]
    pub const fn parts(&self) -> [u64; 4] {
        self.p
    }

    #[inline]
    pub(crate) fn as_m256(self) -> __m256i {
        // SAFETY: `U64x4` は初期化済みの連続した `u64` レーンを保持する。
        // `_mm256_loadu_si256` はアライメント要件がない。
        unsafe { _mm256_loadu_si256(self.p.as_ptr().cast::<__m256i>()) }
    }

    #[inline]
    pub(crate) fn from_m256(value: __m256i) -> Self {
        let mut p = [0u64; 4];
        // SAFETY: `p` はちょうど 32 バイトの書き込み可能な領域を持ち、
        // `_mm256_storeu_si256` はアライメント要件がない。
        unsafe { _mm256_storeu_si256(p.as_mut_ptr().cast::<__m256i>(), value) };
        Self { p }
    }

    #[must_use]
    pub fn from_bitboards(b1: Bitboard, b2: Bitboard) -> Self {
        // SAFETY: このモジュールは AVX2 でガードされている。`Bitboard::as_m128` は
        // 16 バイトアライメントのストレージに裏付けられた初期化済み SSE レジスタ値を返す。
        let mut value = unsafe { _mm256_castsi128_si256(b1.as_m128()) };
        // SAFETY: レーンインデックス 1 は `_mm256_inserti128_si256` に対して有効である。
        value = unsafe { _mm256_inserti128_si256(value, b2.as_m128(), 1) };
        Self::from_m256(value)
    }

    #[must_use]
    pub fn splat_bitboard(bb: Bitboard) -> Self {
        // SAFETY: このモジュールは AVX2 でガードされており、オペランドは初期化済みの
        // 128 ビットレジスタ値である。
        let value = unsafe { _mm256_broadcastsi128_si256(bb.as_m128()) };
        Self::from_m256(value)
    }

    #[must_use]
    pub fn and(self, other: Self) -> Self {
        // SAFETY: このモジュールは AVX2 ターゲット専用にコンパイルされる。
        unsafe { and_simd(self, other) }
    }

    #[must_use]
    pub fn wrapping_add(self, other: Self) -> Self {
        // SAFETY: このモジュールは AVX2 ターゲット専用にコンパイルされる。
        unsafe { add_simd(self, other) }
    }

    #[must_use]
    pub fn wrapping_sub(self, other: Self) -> Self {
        // SAFETY: このモジュールは AVX2 ターゲット専用にコンパイルされる。
        unsafe { sub_simd(self, other) }
    }

    #[must_use]
    pub fn shift_left(self, shift: u32) -> Self {
        // SAFETY: このモジュールは AVX2 ターゲット専用にコンパイルされる。AVX2 の可変
        // シフトはすべてのレーン数に対して結果を定義する。
        unsafe { shl_simd(self, shift) }
    }

    #[must_use]
    pub fn shift_right(self, shift: u32) -> Self {
        // SAFETY: このモジュールは AVX2 ターゲット専用にコンパイルされる。AVX2 の可変
        // シフトはすべてのレーン数に対して結果を定義する。
        unsafe { shr_simd(self, shift) }
    }

    #[must_use]
    pub fn or(self, other: Self) -> Self {
        // SAFETY: このモジュールは AVX2 ターゲット専用にコンパイルされる。
        unsafe { or_simd(self, other) }
    }

    #[must_use]
    pub fn xor(self, other: Self) -> Self {
        // SAFETY: このモジュールは AVX2 ターゲット専用にコンパイルされる。
        unsafe { xor_simd(self, other) }
    }

    #[must_use]
    pub fn and_not(self, other: Self) -> Self {
        // SAFETY: このモジュールは AVX2 ターゲット専用にコンパイルされる。
        unsafe { andnot_simd(self, other) }
    }

    #[must_use]
    pub fn byte_reverse(self) -> Self {
        // SAFETY: このモジュールは AVX2 ターゲット専用にコンパイルされる。
        unsafe { byte_reverse_simd(self) }
    }

    #[must_use]
    pub fn unpack(hi_in: Self, lo_in: Self) -> (Self, Self) {
        // SAFETY: このモジュールは AVX2 ターゲット専用にコンパイルされる。
        unsafe { unpack_simd(hi_in, lo_in) }
    }

    #[must_use]
    pub fn decrement_pair(hi_in: Self, lo_in: Self) -> (Self, Self) {
        // SAFETY: このモジュールは AVX2 ターゲット専用にコンパイルされる。
        unsafe { decrement_pair_simd(hi_in, lo_in) }
    }

    #[must_use]
    pub fn merge(self) -> [u64; 2] {
        // SAFETY: このモジュールは AVX2 ターゲット専用にコンパイルされる。
        unsafe { merge_simd(self) }
    }
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn and_simd(lhs: U64x4, rhs: U64x4) -> U64x4 {
    let l = lhs.as_m256();
    let r = rhs.as_m256();
    U64x4::from_m256(_mm256_and_si256(l, r))
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn add_simd(lhs: U64x4, rhs: U64x4) -> U64x4 {
    let l = lhs.as_m256();
    let r = rhs.as_m256();
    U64x4::from_m256(_mm256_add_epi64(l, r))
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn sub_simd(lhs: U64x4, rhs: U64x4) -> U64x4 {
    let l = lhs.as_m256();
    let r = rhs.as_m256();
    U64x4::from_m256(_mm256_sub_epi64(l, r))
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn shl_simd(value: U64x4, shift: u32) -> U64x4 {
    let v = value.as_m256();
    let count = _mm_cvtsi64_si128(i64::from(shift));
    U64x4::from_m256(_mm256_sll_epi64(v, count))
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn shr_simd(value: U64x4, shift: u32) -> U64x4 {
    let v = value.as_m256();
    let count = _mm_cvtsi64_si128(i64::from(shift));
    U64x4::from_m256(_mm256_srl_epi64(v, count))
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn or_simd(lhs: U64x4, rhs: U64x4) -> U64x4 {
    let l = lhs.as_m256();
    let r = rhs.as_m256();
    U64x4::from_m256(_mm256_or_si256(l, r))
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn xor_simd(lhs: U64x4, rhs: U64x4) -> U64x4 {
    let l = lhs.as_m256();
    let r = rhs.as_m256();
    U64x4::from_m256(_mm256_xor_si256(l, r))
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn andnot_simd(lhs: U64x4, rhs: U64x4) -> U64x4 {
    let l = lhs.as_m256();
    let r = rhs.as_m256();
    U64x4::from_m256(_mm256_andnot_si256(r, l))
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn byte_reverse_simd(value: U64x4) -> U64x4 {
    let v = value.as_m256();
    let shuffle = _mm256_setr_epi8(
        15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6,
        5, 4, 3, 2, 1, 0,
    );
    U64x4::from_m256(_mm256_shuffle_epi8(v, shuffle))
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn unpack_simd(hi_in: U64x4, lo_in: U64x4) -> (U64x4, U64x4) {
    let hi = hi_in.as_m256();
    let lo = lo_in.as_m256();
    let hi_out = _mm256_unpackhi_epi64(lo, hi);
    let lo_out = _mm256_unpacklo_epi64(lo, hi);
    (U64x4::from_m256(hi_out), U64x4::from_m256(lo_out))
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn decrement_pair_simd(hi_in: U64x4, lo_in: U64x4) -> (U64x4, U64x4) {
    let hi = hi_in.as_m256();
    let lo = lo_in.as_m256();
    let borrow = _mm256_cmpeq_epi64(lo, _mm256_setzero_si256());
    let hi_out = _mm256_add_epi64(hi, borrow);
    let lo_out = _mm256_add_epi64(lo, _mm256_set1_epi64x(-1));
    (U64x4::from_m256(hi_out), U64x4::from_m256(lo_out))
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn merge_simd(value: U64x4) -> [u64; 2] {
    let v = value.as_m256();
    let lo = _mm256_castsi256_si128(v);
    let hi = _mm256_extracti128_si256(v, 1);
    let merged = _mm_or_si128(lo, hi);
    let mut out = [0u64; 2];
    unsafe {
        std::arch::x86_64::_mm_storeu_si128(
            out.as_mut_ptr().cast::<std::arch::x86_64::__m128i>(),
            merged,
        );
    }
    out
}
