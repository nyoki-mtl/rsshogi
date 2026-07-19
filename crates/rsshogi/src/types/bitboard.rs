//! 81 マス将棋盤用ビットボード
//!
//! [`Bitboard`] は 9x9 = 81 マスの状態を各 1 ビットで表現するデータ構造である。
//! 駒の配置や利きの計算をビット演算で高速に処理できる。
//!
//! # 内部表現
//!
//! 81 ビットを下位 63 ビット（1～7 筋）と上位 18 ビット（8～9 筋）の
//! 2 つの `u64` で保持する。x86_64 環境では SSE2 の `__m128i` を使用し、
//! それ以外ではスカラー実装にフォールバックする。
//!
//! # 基本操作
//!
//! - ビット演算: `&`, `|`, `^`, `!`（`BitAnd`, `BitOr`, `BitXor`, `Not`）
//! - 要素数: [`count()`](Bitboard::count), [`is_empty()`](Bitboard::is_empty)
//! - ビットスキャン: [`lsb()`](Bitboard::lsb)（最下位ビット）, [`pop_lsb()`](Bitboard::pop_lsb)（取り出し）
//! - マスク生成: [`file_mask()`](Bitboard::file_mask), [`rank_mask()`](Bitboard::rank_mask)
//! - イテレーション: `BitIter` でセットされている各マスを走査
#![allow(unsafe_code)]

#[cfg(any(
    miri,
    not(all(
        target_arch = "x86_64",
        target_feature = "sse2",
        target_feature = "sse4.1",
        target_feature = "ssse3"
    ))
))]
use crate::simd::u64x2_ops;
use crate::types::{Color, File, Rank, Square};

use std::fmt;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
use std::arch::x86_64::{
    __m128i, _mm_and_si128, _mm_andnot_si128, _mm_loadu_si128, _mm_or_si128, _mm_storeu_si128,
    _mm_unpackhi_epi64, _mm_unpacklo_epi64, _mm_xor_si128,
};

#[cfg(all(not(miri), target_arch = "x86_64", target_feature = "sse4.1"))]
use std::arch::x86_64::{_mm_add_epi64, _mm_cmpeq_epi64, _mm_set1_epi64x, _mm_setzero_si128};

#[cfg(all(
    target_arch = "x86_64",
    target_feature = "sse2",
    not(all(target_feature = "sse4.1", target_feature = "ssse3"))
))]
use std::arch::x86_64::{_mm_set_epi64x, _mm_slli_si128, _mm_srli_epi64, _mm_sub_epi64};

#[cfg(all(target_arch = "x86_64", target_feature = "ssse3"))]
use std::arch::x86_64::{_mm_alignr_epi8, _mm_setr_epi8, _mm_shuffle_epi8};

#[cfg(all(target_arch = "x86_64", target_feature = "sse4.1"))]
use std::arch::x86_64::_mm_testz_si128;

const BOARD_MASK_LOW: u64 = 0x7FFF_FFFF_FFFF_FFFF;
const BOARD_MASK_HIGH: u64 = 0x0000_0000_0003_FFFF;

#[inline]
#[allow(dead_code)]
const fn raw_from_parts(parts: (u64, u64)) -> u128 {
    (parts.0 as u128) | ((parts.1 as u128) << 64)
}

const fn file_mask_bits(file: i8) -> u128 {
    let mut bits = 0u128;
    let mut rank = 0u128;
    while rank < 9 {
        let idx = (file as u128) * 9 + rank;
        bits |= 1u128 << idx;
        rank += 1;
    }
    bits
}

const fn rank_mask_bits(rank: i8) -> u128 {
    let mut bits = 0u128;
    let mut file = 0u128;
    while file < 9 {
        let idx = file * 9 + (rank as u128);
        bits |= 1u128 << idx;
        file += 1;
    }
    bits
}

// ANCHOR: bitboard_file_rank_masks
const fn build_file_masks() -> [Bitboard; 9] {
    let mut masks = [Bitboard::EMPTY; 9];
    let mut file = 0u8;
    while file < 9 {
        masks[file as usize] = Bitboard::from_packed_bits(file_mask_bits(file as i8));
        file += 1;
    }
    masks
}

const fn build_rank_masks() -> [Bitboard; 9] {
    let mut masks = [Bitboard::EMPTY; 9];
    let mut rank = 0u8;
    while rank < 9 {
        masks[rank as usize] = Bitboard::from_packed_bits(rank_mask_bits(rank as i8));
        rank += 1;
    }
    masks
}

const FILE_MASKS: [Bitboard; 9] = build_file_masks();
const RANK_MASKS: [Bitboard; 9] = build_rank_masks();

/// 先手と後手の敵陣（成りゾーン）ビットボード [Color::BLACK, Color::WHITE]
const PROMOTION_ZONES: [Bitboard; 2] = {
    let black = rank_mask_bits(0) | rank_mask_bits(1) | rank_mask_bits(2);
    let white = rank_mask_bits(6) | rank_mask_bits(7) | rank_mask_bits(8);
    [Bitboard::from_packed_bits(black), Bitboard::from_packed_bits(white)]
};
// ANCHOR_END: bitboard_file_rank_masks

// ANCHOR: bitboard_struct
/// 81マス将棋盤のビット表現
///
/// 下位81ビットを使用し、各ビットが1つのマスに対応する。
/// `SQ_11`（0）が最下位ビット、`SQ_99`（80）がbit80となる。
#[derive(Copy, Clone)]
#[repr(C, align(16))]
pub struct Bitboard {
    p: [u64; 2],
}

const _: () = {
    assert!(core::mem::size_of::<Bitboard>() == 16);
    assert!(core::mem::align_of::<Bitboard>() == 16);
};
// ANCHOR_END: bitboard_struct

impl Default for Bitboard {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl PartialEq for Bitboard {
    fn eq(&self, other: &Self) -> bool {
        #[cfg(all(target_arch = "x86_64", target_feature = "sse4.1"))]
        {
            // SAFETY: SSE4.1 が有効な場合にのみコンパイルされる。
            // `as_m128()` は初期化済みの `[u64; 2]` をレジスタにロードする。
            let neq = unsafe { _mm_xor_si128(self.as_m128(), other.as_m128()) };
            // SAFETY: `neq` は有効な SSE レジスタ値であり、`_mm_testz_si128`
            // はレジスタオペランドの読み取りのみを行う。
            unsafe { _mm_testz_si128(neq, neq) != 0 }
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse4.1")))]
        {
            let [a0, a1] = self.parts();
            let [b0, b1] = other.parts();
            a0 == b0 && a1 == b1
        }
    }
}

impl Eq for Bitboard {}

impl Bitboard {
    /// 空のビットボード
    pub const EMPTY: Self = { Self { p: [0, 0] } };

    /// 全マスが立っているビットボード（81マス分）
    pub const ALL: Self = { Self { p: [BOARD_MASK_LOW, BOARD_MASK_HIGH] } };

    /// Packed u128 (0..80) からBitboardを生成
    #[inline]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn from_packed_bits(packed_bits: u128) -> Self {
        let packed = packed_bits & ((1u128 << 81) - 1);
        let low = (packed & BOARD_MASK_LOW as u128) as u64;
        let high = ((packed >> 63) & BOARD_MASK_HIGH as u128) as u64;
        Self { p: [low, high] }
    }

    #[inline]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) const fn from_packed_bits_unchecked(packed_bits: u128) -> Self {
        let low = packed_bits as u64;
        let high = (packed_bits >> 64) as u64;
        Self { p: [low, high] }
    }

    /// 単一のマスからビットボードを作成
    #[inline]
    #[must_use]
    pub fn from_square(sq: Square) -> Self {
        debug_assert!(sq.is_on_board(), "Invalid square: {sq:?}");
        let idx = sq.raw();
        if idx < 63 {
            // `idx` は 0..=62 のはず（ホットパス）。
            let shift = idx as u32;
            Self { p: [1u64 << shift, 0] }
        } else {
            let shift = (idx - 63) as u32;
            Self { p: [0, 1u64 << shift] }
        }
    }

    /// 内部のビット表現を取得
    #[inline]
    #[must_use]
    pub const fn packed_bits(&self) -> u128 {
        let parts = self.parts();
        (parts[0] as u128) | ((parts[1] as u128) << 63)
    }

    /// 内部のビットを u128 として取得（parts[1] を 64bit 位置にシフト）。
    ///
    /// `packed_bits()` と異なり、parts[1] を 64bit シフトする。
    /// 非 SSE2 フォールバックパスのビット演算で使用。
    #[inline]
    #[must_use]
    #[allow(dead_code)]
    pub(crate) const fn raw_bits(&self) -> u128 {
        let parts = self.parts();
        (parts[0] as u128) | ((parts[1] as u128) << 64)
    }

    /// 内部の `[u64; 2]` 表現を取得する。
    ///
    /// `parts[0]` は 1～7 筋（下位 63 ビット）、
    /// `parts[1]` は 8～9 筋（下位 18 ビット）に対応する。
    #[inline]
    #[must_use]
    pub const fn parts(&self) -> [u64; 2] {
        self.p
    }

    /// 内部の2x u64表現からBitboardを生成
    #[inline]
    #[must_use]
    pub const fn from_parts(parts: [u64; 2]) -> Self {
        let low = parts[0] & BOARD_MASK_LOW;
        let high = parts[1] & BOARD_MASK_HIGH;
        Self { p: [low, high] }
    }

    /// u128 からマスクなしで Bitboard を構築する。
    ///
    /// 非 SSE2 フォールバックパスのビット演算で使用。
    #[inline]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(dead_code)]
    pub(crate) const fn from_raw_bits_unmasked(raw: u128) -> Self {
        let low = raw as u64;
        let high = (raw >> 64) as u64;
        Self { p: [low, high] }
    }

    #[inline]
    fn parts_mut(&mut self) -> &mut [u64; 2] {
        &mut self.p
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    #[inline]
    #[allow(clippy::missing_const_for_fn)]
    pub(crate) fn as_m128(self) -> __m128i {
        // SAFETY: `Bitboard` は連続した初期化済みの `u64` レーンを保持する。
        // `_mm_loadu_si128` にアライメント要件はない。
        unsafe { _mm_loadu_si128(self.p.as_ptr().cast::<__m128i>()) }
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    #[inline]
    #[allow(clippy::missing_const_for_fn)]
    pub(crate) fn from_m128(value: __m128i) -> Self {
        let mut p = [0u64; 2];
        // SAFETY: `p` はちょうど 16 バイトの書き込み領域を持ち、`_mm_storeu_si128`
        // にアライメント要件はない。
        unsafe { _mm_storeu_si128(p.as_mut_ptr().cast::<__m128i>(), value) };
        Self { p }
    }

    /// 指定マスのビットを立てる
    #[inline]
    pub fn set(&mut self, sq: Square) {
        debug_assert!(sq.is_on_board(), "Invalid square: {sq:?}");
        let idx = sq.raw();
        if idx < 63 {
            let shift = idx as u32;
            let parts = self.parts_mut();
            parts[0] |= 1u64 << shift;
        } else {
            let shift = (idx - 63) as u32;
            let parts = self.parts_mut();
            parts[1] |= 1u64 << shift;
        }
    }

    /// 指定マスのビットをクリア
    #[inline]
    pub fn clear(&mut self, sq: Square) {
        debug_assert!(sq.is_on_board(), "Invalid square: {sq:?}");
        let idx = sq.raw();
        if idx < 63 {
            let shift = idx as u32;
            let parts = self.parts_mut();
            parts[0] &= !(1u64 << shift);
        } else {
            let shift = (idx - 63) as u32;
            let parts = self.parts_mut();
            parts[1] &= !(1u64 << shift);
        }
    }

    /// 指定マスのビットが立っているか判定
    #[inline]
    #[must_use]
    pub fn test(&self, sq: Square) -> bool {
        debug_assert!(sq.is_on_board(), "Invalid square: {sq:?}");
        let idx = sq.raw();
        if idx < 63 {
            let shift = idx as u32;
            let parts = self.parts();
            (parts[0] & (1u64 << shift)) != 0
        } else {
            let shift = (idx - 63) as u32;
            let parts = self.parts();
            (parts[1] & (1u64 << shift)) != 0
        }
    }

    /// 生インデックス（`0..Square::COUNT`）でマスのビットを判定する。
    ///
    /// [`Bitboard::test`] の生インデックス版。ホットパスで検証済みのインデックスが
    /// 既にある場合を除き、通常は [`Bitboard::test`] を使う。
    #[inline]
    #[must_use]
    pub fn test_index(&self, index: usize) -> bool {
        debug_assert!(index < Square::COUNT, "Invalid square index: {index}");
        let parts = self.parts();
        if index < 63 {
            (parts[0] & (1u64 << index)) != 0
        } else if index < Square::COUNT {
            (parts[1] & (1u64 << (index - 63))) != 0
        } else {
            false
        }
    }

    // ANCHOR: bitboard_pop_lsb
    /// 最下位の立っているビットを取り出してクリア
    #[inline]
    pub fn pop_lsb(&mut self) -> Option<Square> {
        let parts = self.parts_mut();
        if parts[0] != 0 {
            let lsb = parts[0].trailing_zeros();
            parts[0] = blsr_u64(parts[0]);
            // lsb は 0..=62
            Some(Square::new(lsb as i8))
        } else if parts[1] != 0 {
            let lsb = parts[1].trailing_zeros();
            parts[1] = blsr_u64(parts[1]);
            // parts[1] は 63..=80 を保持する（lsb は 0..=17）
            Some(Square::new((63 + lsb) as i8))
        } else {
            None
        }
    }

    /// 最下位の立っているビットを取り出してクリアする。
    ///
    /// # Safety
    ///
    /// 呼び出し側は、この bitboard が空でないことを保証しなければならない。
    #[inline]
    #[must_use]
    pub unsafe fn pop_lsb_unchecked(&mut self) -> Square {
        let parts = self.parts_mut();
        if parts[0] != 0 {
            let lsb = parts[0].trailing_zeros();
            parts[0] = blsr_u64(parts[0]);
            Square::new(lsb as i8)
        } else {
            debug_assert!(parts[1] != 0, "pop_lsb_unchecked requires a non-empty bitboard");
            let lsb = parts[1].trailing_zeros();
            parts[1] = blsr_u64(parts[1]);
            Square::new((63 + lsb) as i8)
        }
    }
    // ANCHOR_END: bitboard_pop_lsb

    /// 最下位ビット（LSB）の位置を取得（破壊的でない）
    #[inline]
    #[must_use]
    pub fn lsb(&self) -> Option<Square> {
        let parts = self.parts();
        if parts[0] == 0 && parts[1] == 0 {
            return None;
        }
        if parts[0] != 0 {
            let lsb = parts[0].trailing_zeros();
            Some(Square::new(lsb as i8))
        } else {
            let lsb = parts[1].trailing_zeros();
            Some(Square::new((63 + lsb) as i8))
        }
    }

    /// 最上位ビット（MSB）の位置を取得
    #[inline]
    #[must_use]
    pub fn msb(&self) -> Option<Square> {
        let parts = self.parts();
        if parts[0] == 0 && parts[1] == 0 {
            return None;
        }
        if parts[1] != 0 {
            let msb = parts[1].ilog2();
            Some(Square::new((63 + msb) as i8))
        } else {
            let msb = parts[0].ilog2();
            Some(Square::new(msb as i8))
        }
    }

    // ANCHOR: bitboard_count
    /// 立っているビットの数を数える
    #[inline]
    #[must_use]
    pub const fn count(&self) -> u32 {
        let parts = self.parts();
        parts[0].count_ones() + parts[1].count_ones()
    }
    // ANCHOR_END: bitboard_count

    /// ビットごとのAND演算
    #[inline]
    #[must_use]
    pub fn and(&self, other: Self) -> Self {
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        {
            // SAFETY: SSE2 が有効な場合にのみコンパイルされる。両入力とも
            // 初期化済みビットボードから生成された有効な 128 ビットレジスタ値である。
            let value = unsafe { _mm_and_si128((*self).as_m128(), other.as_m128()) };
            Self::from_m128(value)
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
        {
            let parts = u64x2_ops::and(
                (self.parts()[0], self.parts()[1]),
                (other.parts()[0], other.parts()[1]),
            );
            Self::from_raw_bits_unmasked(raw_from_parts(parts))
        }
    }

    /// ビットごとのOR演算
    #[inline]
    #[must_use]
    pub fn or(&self, other: Self) -> Self {
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        {
            // SAFETY: SSE2 が有効な場合にのみコンパイルされ、
            // レジスタ値のみを操作する。
            let value = unsafe { _mm_or_si128((*self).as_m128(), other.as_m128()) };
            Self::from_m128(value)
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
        {
            let parts = u64x2_ops::or(
                (self.parts()[0], self.parts()[1]),
                (other.parts()[0], other.parts()[1]),
            );
            Self::from_raw_bits_unmasked(raw_from_parts(parts))
        }
    }

    /// ビットごとのXOR演算
    #[inline]
    #[must_use]
    pub fn xor(&self, other: Self) -> Self {
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        {
            // SAFETY: SSE2 が有効な場合にのみコンパイルされ、
            // レジスタ値のみを操作する。
            let value = unsafe { _mm_xor_si128((*self).as_m128(), other.as_m128()) };
            Self::from_m128(value)
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
        {
            let parts = u64x2_ops::xor(
                (self.parts()[0], self.parts()[1]),
                (other.parts()[0], other.parts()[1]),
            );
            Self::from_raw_bits_unmasked(raw_from_parts(parts))
        }
    }

    /// 指定した筋のビットボードを作成
    #[must_use]
    pub fn file_mask(file: File) -> Self {
        debug_assert!(file.is_valid(), "file must be valid");
        FILE_MASKS[file.raw() as usize]
    }

    /// 指定した段のビットボードを作成
    #[must_use]
    pub fn rank_mask(rank: Rank) -> Self {
        debug_assert!(rank.is_valid(), "rank must be valid");
        RANK_MASKS[rank.raw() as usize]
    }

    /// 指定した手番にとっての敵陣（成りゾーン）を返す。
    #[inline]
    #[must_use]
    pub fn promotion_zone(us: Color) -> Self {
        PROMOTION_ZONES[us.to_index()]
    }

    /// ビット反転（81マスの範囲内）
    #[inline]
    #[must_use]
    pub const fn not(&self) -> Self {
        let parts = self.parts();
        let masked = [(!parts[0]) & BOARD_MASK_LOW, (!parts[1]) & BOARD_MASK_HIGH];
        Self { p: masked }
    }

    /// 盤面を180度回転させたBitboardを返す。
    #[must_use]
    pub fn flip(&self) -> Self {
        let mut out = Self::EMPTY;
        for raw in 0..Square::COUNT as i8 {
            let square = Square::new(raw);
            if self.test(square) {
                let flipped = Square::new((Square::COUNT as i8 - 1) - raw);
                out.set(flipped);
            }
        }
        out
    }

    /// 盤面を左右反転させたBitboardを返す。
    #[must_use]
    pub fn mirror_file(&self) -> Self {
        let mut out = Self::EMPTY;
        for raw in 0..Square::COUNT as i8 {
            let square = Square::new(raw);
            if self.test(square) {
                let file = raw / 9;
                let rank = raw % 9;
                let flipped_file = 8 - file;
                out.set(Square::new(flipped_file * 9 + rank));
            }
        }
        out
    }

    /// AND NOT演算（self & !other）
    ///
    /// YaneuraOu の`andnot`は `!self & other` を指すため注意すること。
    #[inline]
    #[must_use]
    pub fn and_not(&self, other: Self) -> Self {
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        {
            // SAFETY: SSE2 が有効な場合にのみコンパイルされる。`_mm_andnot`
            // はレジスタオペランドを読み取り `!other & self` を返す。
            let value = unsafe { _mm_andnot_si128(other.as_m128(), (*self).as_m128()) };
            Self::from_m128(value)
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
        {
            let parts = u64x2_ops::and_not(
                (self.parts()[0], self.parts()[1]),
                (other.parts()[0], other.parts()[1]),
            );
            Self::from_raw_bits_unmasked(raw_from_parts(parts))
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn and_raw(self, other: Self) -> Self {
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        {
            // SAFETY: SSE2 が有効な場合にのみコンパイルされ、
            // レジスタ値のみを操作する。
            let value = unsafe { _mm_and_si128(self.as_m128(), other.as_m128()) };
            Self::from_m128(value)
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
        {
            let parts = u64x2_ops::and(
                (self.parts()[0], self.parts()[1]),
                (other.parts()[0], other.parts()[1]),
            );
            Self::from_raw_bits_unmasked(raw_from_parts(parts))
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn xor_raw(self, other: Self) -> Self {
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        {
            // SAFETY: SSE2 が有効な場合にのみコンパイルされ、
            // レジスタ値のみを操作する。
            let value = unsafe { _mm_xor_si128(self.as_m128(), other.as_m128()) };
            Self::from_m128(value)
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
        {
            let parts = u64x2_ops::xor(
                (self.parts()[0], self.parts()[1]),
                (other.parts()[0], other.parts()[1]),
            );
            Self::from_raw_bits_unmasked(raw_from_parts(parts))
        }
    }

    /// 他のBitboardと交差しているか判定する。
    #[inline]
    #[must_use]
    pub fn intersects(&self, other: Self) -> bool {
        #[cfg(all(target_arch = "x86_64", target_feature = "sse4.1"))]
        {
            // SAFETY: SSE4.1 が有効な場合にのみコンパイルされ、
            // `_mm_testz_si128` はレジスタオペランドの読み取りのみを行う。
            unsafe { _mm_testz_si128((*self).as_m128(), other.as_m128()) == 0 }
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse4.1")))]
        {
            !u64x2_ops::testz(
                (self.parts()[0], self.parts()[1]),
                (other.parts()[0], other.parts()[1]),
            )
        }
    }

    /// byte単位で入れ替えたBitboardを返す。
    #[inline]
    #[must_use]
    pub fn byte_reverse(&self) -> Self {
        #[cfg(all(target_arch = "x86_64", target_feature = "ssse3"))]
        {
            // SAFETY: SSSE3 が有効な場合にのみコンパイルされる。シャッフルマスクは
            // レジスタ値としてのみ使用される定数ベクタである。
            let shuffle =
                unsafe { _mm_setr_epi8(15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0) };
            // SAFETY: `self.as_m128()` と `shuffle` はいずれも有効なレジスタ値である。
            let value = unsafe { _mm_shuffle_epi8(self.as_m128(), shuffle) };
            Self::from_m128(value)
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "ssse3")))]
        {
            let parts = self.parts();
            let (p0, p1) = u64x2_ops::byte_reverse(parts[0], parts[1]);
            let parts: [u64; 2] = (p0, p1).into();
            Self { p: parts }
        }
    }

    /// SSE2のunpackを実行した結果を返す。
    #[inline]
    #[must_use]
    pub fn unpack(hi_in: Self, lo_in: Self) -> (Self, Self) {
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        {
            // SAFETY: SSE2 が有効な場合にのみコンパイルされる。アンパック
            // 命令は初期化済みのレジスタ値を操作する。
            let hi = unsafe { _mm_unpackhi_epi64(lo_in.as_m128(), hi_in.as_m128()) };
            let lo = unsafe { _mm_unpacklo_epi64(lo_in.as_m128(), hi_in.as_m128()) };
            (Self::from_m128(hi), Self::from_m128(lo))
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
        {
            let (hi, lo) = u64x2_ops::unpack(
                (hi_in.parts()[0], hi_in.parts()[1]),
                (lo_in.parts()[0], lo_in.parts()[1]),
            );
            (
                Self::from_raw_bits_unmasked(raw_from_parts(hi)),
                Self::from_raw_bits_unmasked(raw_from_parts(lo)),
            )
        }
    }

    /// 2組のBitboardを128bit整数とみなして1減算した結果を返す。
    #[inline]
    #[must_use]
    pub fn decrement_pair(hi_in: Self, lo_in: Self) -> (Self, Self) {
        #[cfg(miri)]
        {
            let (hi, lo) = u64x2_ops::decrement_pair(
                (hi_in.parts()[0], hi_in.parts()[1]),
                (lo_in.parts()[0], lo_in.parts()[1]),
            );
            (
                Self::from_raw_bits_unmasked(raw_from_parts(hi)),
                Self::from_raw_bits_unmasked(raw_from_parts(lo)),
            )
        }
        #[cfg(all(not(miri), target_arch = "x86_64", target_feature = "sse4.1"))]
        {
            let hi = hi_in.as_m128();
            let lo = lo_in.as_m128();
            // SAFETY: SSE4.1 が有効な場合にのみコンパイルされる。
            // 比較と加算命令は初期化済みのレジスタ値を操作する。
            let borrow = unsafe { _mm_cmpeq_epi64(lo, _mm_setzero_si128()) };
            let hi_out = unsafe { _mm_add_epi64(hi, borrow) };
            let lo_out = unsafe { _mm_add_epi64(lo, _mm_set1_epi64x(-1)) };
            (Self::from_m128(hi_out), Self::from_m128(lo_out))
        }
        #[cfg(not(any(miri, all(target_arch = "x86_64", target_feature = "sse4.1"))))]
        {
            let (hi, lo) = u64x2_ops::decrement_pair(
                (hi_in.parts()[0], hi_in.parts()[1]),
                (lo_in.parts()[0], lo_in.parts()[1]),
            );
            (
                Self::from_raw_bits_unmasked(raw_from_parts(hi)),
                Self::from_raw_bits_unmasked(raw_from_parts(lo)),
            )
        }
    }

    /// このBitboardを128bit整数とみなして1減算したBitboardを返す。
    #[inline]
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn decrement(&self) -> Self {
        #[cfg(all(target_arch = "x86_64", target_feature = "sse4.1", target_feature = "ssse3"))]
        {
            // SAFETY: SSE4.1 と SSSE3 が両方有効な場合にのみコンパイルされる。
            // 全操作はレジスタのみで完結し、任意のビットパターンを受け付ける。
            let t2 = unsafe { _mm_cmpeq_epi64(self.as_m128(), _mm_setzero_si128()) };
            let t2 = unsafe { _mm_alignr_epi8(t2, _mm_set1_epi64x(-1), 8) };
            let t1 = unsafe { _mm_add_epi64(self.as_m128(), t2) };
            Self::from_m128(t1)
        }
        #[cfg(all(
            target_arch = "x86_64",
            target_feature = "sse2",
            not(all(target_feature = "sse4.1", target_feature = "ssse3"))
        ))]
        {
            // SAFETY: SSE2 が有効な場合にのみコンパイルされる。このフォールバックは
            // レジスタのみの操作で 128 ビットデクリメントを実装する。
            let c = unsafe { _mm_set_epi64x(0, 1) };
            let mut t1 = unsafe { _mm_sub_epi64(self.as_m128(), c) };
            let t2 = unsafe { _mm_srli_epi64(t1, 63) };
            let t2 = unsafe { _mm_slli_si128(t2, 8) };
            t1 = unsafe { _mm_sub_epi64(t1, t2) };
            Self::from_m128(t1)
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
        {
            let raw = self.raw_bits().wrapping_sub(1);
            Self::from_raw_bits_unmasked(raw)
        }
    }

    /// 2bit以上あるかどうかを判定する。
    #[inline]
    #[must_use]
    pub const fn more_than_one(&self) -> bool {
        let parts = self.parts();
        if parts[0] & (parts[0].wrapping_sub(1)) != 0 {
            return true;
        }
        if parts[1] & (parts[1].wrapping_sub(1)) != 0 {
            return true;
        }
        parts[0] != 0 && parts[1] != 0
    }

    /// 空かどうか判定
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        let parts = self.parts();
        parts[0] == 0 && parts[1] == 0
    }

    /// 何かビットが立っているか判定
    #[inline]
    #[must_use]
    pub const fn any(&self) -> bool {
        let parts = self.parts();
        parts[0] != 0 || parts[1] != 0
    }

    /// イテレータを返す
    #[must_use]
    pub const fn iter(&self) -> BitIter {
        BitIter { bb: *self }
    }
}

#[inline]
fn blsr_u64(value: u64) -> u64 {
    #[cfg(all(target_arch = "x86_64", target_feature = "bmi1"))]
    unsafe {
        // SAFETY: BMI1 が有効な場合にのみコンパイルされる。`_blsr_u64` は
        // あらゆる `u64` を受け付け、ゼロを渡すと命令はゼロを返す。
        core::arch::x86_64::_blsr_u64(value)
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "bmi1")))]
    {
        value & (value - 1)
    }
}

// ANCHOR: bitboard_iter
impl IntoIterator for &Bitboard {
    type Item = Square;
    type IntoIter = BitIter;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Bitboard上の立っているビットを列挙するイテレータ
pub struct BitIter {
    bb: Bitboard,
}

impl Iterator for BitIter {
    type Item = Square;

    fn next(&mut self) -> Option<Self::Item> {
        self.bb.pop_lsb()
    }
}
// ANCHOR_END: bitboard_iter

impl fmt::Debug for Bitboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Bitboard(")?;
        for rank in (0..9).rev() {
            write!(f, "  ")?;
            for file in (0..9).rev() {
                let sq = Square::from_file_rank(File::new(file), Rank::new(rank));
                if self.test(sq) {
                    write!(f, "1")?;
                } else {
                    write!(f, "0")?;
                }
            }
            writeln!(f)?;
        }
        write!(f, ")")
    }
}

// ANCHOR: bitboard_bit_ops
impl BitAnd for Bitboard {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        {
            // SAFETY: SSE2 が有効な場合にのみコンパイルされ、
            // 初期化済みのレジスタ値のみを操作する。
            let value = unsafe { _mm_and_si128(self.as_m128(), rhs.as_m128()) };
            Self::from_m128(value)
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
        {
            let parts = u64x2_ops::and(
                (self.parts()[0], self.parts()[1]),
                (rhs.parts()[0], rhs.parts()[1]),
            );
            Self::from_raw_bits_unmasked(raw_from_parts(parts))
        }
    }
}

impl BitOr for Bitboard {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        {
            // SAFETY: SSE2 が有効な場合にのみコンパイルされ、
            // 初期化済みのレジスタ値のみを操作する。
            let value = unsafe { _mm_or_si128(self.as_m128(), rhs.as_m128()) };
            Self::from_m128(value)
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
        {
            let parts =
                u64x2_ops::or((self.parts()[0], self.parts()[1]), (rhs.parts()[0], rhs.parts()[1]));
            Self::from_raw_bits_unmasked(raw_from_parts(parts))
        }
    }
}

impl BitOrAssign for Bitboard {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl BitXor for Bitboard {
    type Output = Self;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        {
            // SAFETY: SSE2 が有効な場合にのみコンパイルされ、
            // 初期化済みのレジスタ値のみを操作する。
            let value = unsafe { _mm_xor_si128(self.as_m128(), rhs.as_m128()) };
            Self::from_m128(value)
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
        {
            let parts = u64x2_ops::xor(
                (self.parts()[0], self.parts()[1]),
                (rhs.parts()[0], rhs.parts()[1]),
            );
            Self::from_raw_bits_unmasked(raw_from_parts(parts))
        }
    }
}

impl BitXorAssign for Bitboard {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        *self = *self ^ rhs;
    }
}

impl BitAndAssign for Bitboard {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}

impl Not for Bitboard {
    type Output = Self;

    #[inline]
    fn not(self) -> Self::Output {
        // 81ビットマスクを適用して、使用しないビットをクリア
        let parts = self.parts();
        let masked = [(!parts[0]) & BOARD_MASK_LOW, (!parts[1]) & BOARD_MASK_HIGH];
        Self { p: masked }
    }
}
// ANCHOR_END: bitboard_bit_ops

impl fmt::Display for Bitboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(test)]
mod tests;
