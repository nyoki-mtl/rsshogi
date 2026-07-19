#![allow(clippy::missing_const_for_fn)]
#![allow(dead_code)]

#[inline]
#[must_use]
pub fn byte_reverse(p0: u64, p1: u64) -> (u64, u64) {
    (p1.swap_bytes(), p0.swap_bytes())
}

#[inline]
#[must_use]
pub fn and(a: (u64, u64), b: (u64, u64)) -> (u64, u64) {
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    unsafe {
        // SAFETY: SSE2 が有効なときのみコンパイル・呼び出される。
        and_sse2(a, b)
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
    {
        (a.0 & b.0, a.1 & b.1)
    }
}

#[inline]
#[must_use]
pub fn or(a: (u64, u64), b: (u64, u64)) -> (u64, u64) {
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    unsafe {
        // SAFETY: SSE2 が有効なときのみコンパイル・呼び出される。
        or_sse2(a, b)
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
    {
        (a.0 | b.0, a.1 | b.1)
    }
}

#[inline]
#[must_use]
pub fn xor(a: (u64, u64), b: (u64, u64)) -> (u64, u64) {
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    unsafe {
        // SAFETY: SSE2 が有効なときのみコンパイル・呼び出される。
        xor_sse2(a, b)
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
    {
        (a.0 ^ b.0, a.1 ^ b.1)
    }
}

#[inline]
#[must_use]
pub fn and_not(a: (u64, u64), b: (u64, u64)) -> (u64, u64) {
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    unsafe {
        // SAFETY: SSE2 が有効なときのみコンパイル・呼び出される。
        andnot_sse2(a, b)
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
    {
        (a.0 & !b.0, a.1 & !b.1)
    }
}

#[inline]
#[must_use]
pub fn testz(a: (u64, u64), b: (u64, u64)) -> bool {
    #[cfg(all(target_arch = "x86_64", target_feature = "sse4.1"))]
    unsafe {
        // SAFETY: SSE4.1 が有効なときのみコンパイル・呼び出される。
        testz_sse41(a, b)
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "sse4.1")))]
    {
        (a.0 & b.0) == 0 && (a.1 & b.1) == 0
    }
}

#[inline]
#[must_use]
pub fn unpack(hi_in: (u64, u64), lo_in: (u64, u64)) -> ((u64, u64), (u64, u64)) {
    let hi_out = (lo_in.1, hi_in.1);
    let lo_out = (lo_in.0, hi_in.0);
    (hi_out, lo_out)
}

#[inline]
#[must_use]
pub fn decrement_pair(hi_in: (u64, u64), lo_in: (u64, u64)) -> ((u64, u64), (u64, u64)) {
    let hi_out = (
        hi_in.0.wrapping_sub(u64::from(lo_in.0 == 0)),
        hi_in.1.wrapping_sub(u64::from(lo_in.1 == 0)),
    );
    let lo_out = (lo_in.0.wrapping_sub(1), lo_in.1.wrapping_sub(1));
    (hi_out, lo_out)
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
use std::arch::x86_64::{
    __m128i, _mm_and_si128, _mm_andnot_si128, _mm_or_si128, _mm_set_epi64x, _mm_storeu_si128,
    _mm_xor_si128,
};

#[cfg(all(target_arch = "x86_64", target_feature = "sse4.1"))]
use std::arch::x86_64::_mm_testz_si128;

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[inline]
#[target_feature(enable = "sse2")]
unsafe fn and_sse2(a: (u64, u64), b: (u64, u64)) -> (u64, u64) {
    // SAFETY: SSE2 が有効な場合のみコンパイル・呼び出される。
    let av = unsafe { to_m128i(a) };
    // SAFETY: SSE2 が有効な場合のみコンパイル・呼び出される。
    let bv = unsafe { to_m128i(b) };
    // SAFETY: SSE2 が有効な場合のみコンパイル・呼び出される。
    unsafe { from_m128i(_mm_and_si128(av, bv)) }
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[inline]
#[target_feature(enable = "sse2")]
unsafe fn or_sse2(a: (u64, u64), b: (u64, u64)) -> (u64, u64) {
    // SAFETY: SSE2 が有効な場合のみコンパイル・呼び出される。
    let av = unsafe { to_m128i(a) };
    // SAFETY: SSE2 が有効な場合のみコンパイル・呼び出される。
    let bv = unsafe { to_m128i(b) };
    // SAFETY: SSE2 が有効な場合のみコンパイル・呼び出される。
    unsafe { from_m128i(_mm_or_si128(av, bv)) }
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[inline]
#[target_feature(enable = "sse2")]
unsafe fn xor_sse2(a: (u64, u64), b: (u64, u64)) -> (u64, u64) {
    // SAFETY: SSE2 が有効な場合のみコンパイル・呼び出される。
    let av = unsafe { to_m128i(a) };
    // SAFETY: SSE2 が有効な場合のみコンパイル・呼び出される。
    let bv = unsafe { to_m128i(b) };
    // SAFETY: SSE2 が有効な場合のみコンパイル・呼び出される。
    unsafe { from_m128i(_mm_xor_si128(av, bv)) }
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse4.1"))]
#[inline]
#[target_feature(enable = "sse4.1")]
unsafe fn testz_sse41(a: (u64, u64), b: (u64, u64)) -> bool {
    // SAFETY: SSE4.1 が有効な場合のみコンパイル・呼び出される。
    let av = unsafe { to_m128i(a) };
    // SAFETY: SSE4.1 が有効な場合のみコンパイル・呼び出される。
    let bv = unsafe { to_m128i(b) };
    _mm_testz_si128(av, bv) != 0
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[inline]
#[target_feature(enable = "sse2")]
unsafe fn andnot_sse2(a: (u64, u64), b: (u64, u64)) -> (u64, u64) {
    // SAFETY: SSE2 が有効な場合のみコンパイル・呼び出される。
    let av = unsafe { to_m128i(a) };
    // SAFETY: SSE2 が有効な場合のみコンパイル・呼び出される。
    let bv = unsafe { to_m128i(b) };
    // SAFETY: SSE2 が有効な場合のみコンパイル・呼び出される。
    unsafe { from_m128i(_mm_andnot_si128(bv, av)) }
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[inline]
#[target_feature(enable = "sse2")]
unsafe fn to_m128i(value: (u64, u64)) -> __m128i {
    let lo = i64::from_ne_bytes(value.0.to_ne_bytes());
    let hi = i64::from_ne_bytes(value.1.to_ne_bytes());
    _mm_set_epi64x(hi, lo)
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[inline]
#[target_feature(enable = "sse2")]
unsafe fn from_m128i(value: __m128i) -> (u64, u64) {
    #[repr(align(16))]
    struct AlignedU64x2([u64; 2]);

    let mut out = AlignedU64x2([0u64; 2]);
    let ptr = std::ptr::addr_of_mut!(out).cast::<__m128i>();
    // SAFETY: `out` は少なくとも 16 バイトの書き込み領域を持ち、`_mm_storeu_si128`
    // はアライメント要件がない。
    unsafe { _mm_storeu_si128(ptr, value) };
    (out.0[0], out.0[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u64x2_ops_helpers() {
        let (p0, p1) = byte_reverse(0x1122_3344_5566_7788, 0x99AA_BBCC_DDEE_FF00);
        assert_eq!(p0, 0x00FF_EEDD_CCBB_AA99);
        assert_eq!(p1, 0x8877_6655_4433_2211);

        let and_out = and((0b1100, 0b1010), (0b1010, 0b1100));
        assert_eq!(and_out, (0b1000, 0b1000));
        let or_out = or((0b1100, 0b1010), (0b1010, 0b1100));
        assert_eq!(or_out, (0b1110, 0b1110));
        let xor_out = xor((0b1100, 0b1010), (0b1010, 0b1100));
        assert_eq!(xor_out, (0b0110, 0b0110));
        let and_not_out = and_not((0b1100, 0b1010), (0b1010, 0b1100));
        assert_eq!(and_not_out, (0b0100, 0b0010));
        assert!(testz((0b1000, 0b0000), (0b0001, 0b0000)));
        assert!(!testz((0b1000, 0b0000), (0b1000, 0b0000)));

        let hi_in = (1u64, 2u64);
        let lo_in = (3u64, 4u64);
        let (hi_out, lo_out) = unpack(hi_in, lo_in);
        assert_eq!(hi_out, (4, 2));
        assert_eq!(lo_out, (3, 1));

        let (dec_hi, dec_lo) = decrement_pair((5, 6), (0, 7));
        assert_eq!(dec_hi, (4, 6));
        assert_eq!(dec_lo, (u64::MAX, 6));
    }
}
