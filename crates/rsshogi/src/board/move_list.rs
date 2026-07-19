//! `MoveList` - 固定長の合法手リスト
//!
//! # Safety
//! - このモジュール内の `unsafe` は、ホットパス最適化のために
//!   `MaybeUninit` バッファへ直接書き込む用途でのみ使用する。
//! - `FixedVec` は `Copy` 型のみを保持し、`len` で有効範囲を管理する。
//! - 公開 safe API は容量超過を panic として扱い、内部 hot path のみ
//!   unchecked push を使う。

#![allow(clippy::inline_always)]

use crate::types::{Move, Move32};
use std::mem::MaybeUninit;
use std::ptr;

/// 固定長の小さな可変配列（MoveList用の軽量実装）
#[derive(Debug)]
struct FixedVec<T: Copy, const N: usize> {
    data: [MaybeUninit<T>; N],
    len: usize,
}

impl<T: Copy, const N: usize> FixedVec<T, N> {
    pub const fn new() -> Self {
        let data: [MaybeUninit<T>; N] = [const { MaybeUninit::uninit() }; N];
        Self { data, len: 0 }
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn clear(&mut self) {
        self.len = 0;
    }

    #[inline]
    pub fn push(&mut self, value: T) {
        assert!(self.len < N, "MoveList capacity exceeded");
        // SAFETY: len < N を満たす前提でのみ書き込む。
        unsafe {
            self.data.get_unchecked_mut(self.len).as_mut_ptr().write(value);
        }
        self.len += 1;
    }

    #[inline]
    pub unsafe fn push_unchecked(&mut self, value: T) {
        debug_assert!(self.len < N);
        // SAFETY: 呼び出し側が len < N を保証する。
        unsafe { self.data.get_unchecked_mut(self.len).as_mut_ptr().write(value) };
        self.len += 1;
    }

    #[inline]
    pub fn remove(&mut self, index: usize) -> T {
        debug_assert!(index < self.len);
        // SAFETY: index は有効範囲内。Copy なので read/shift は安全。
        unsafe {
            let value = self.data.get_unchecked(index).assume_init_read();
            let old_len = self.len;
            self.len -= 1;
            let src = self.data.as_ptr().add(index + 1).cast::<T>();
            let dst = self.data.as_mut_ptr().add(index).cast::<T>();
            ptr::copy(src, dst, old_len - index - 1);
            value
        }
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    #[inline]
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: len までの範囲は初期化済みとして扱う。
        unsafe { std::slice::from_raw_parts(self.data.as_ptr().cast::<T>(), self.len) }
    }
}

impl<T: Copy, const N: usize> Clone for FixedVec<T, N> {
    fn clone(&self) -> Self {
        let mut out = Self::new();
        out.len = self.len;
        // SAFETY: len までの範囲は初期化済みであり、Copy なので複製可能。
        unsafe {
            ptr::copy_nonoverlapping(
                self.data.as_ptr().cast::<T>(),
                out.data.as_mut_ptr().cast::<T>(),
                self.len,
            );
        }
        out
    }
}

impl<T: Copy, const N: usize> std::ops::Index<usize> for FixedVec<T, N> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < self.len);
        // SAFETY: index < len の範囲は初期化済み。
        unsafe { &*self.data.get_unchecked(index).as_ptr() }
    }
}

impl<T: Copy, const N: usize> std::ops::IndexMut<usize> for FixedVec<T, N> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        debug_assert!(index < self.len);
        // SAFETY: index < len の範囲は初期化済み。
        unsafe { &mut *self.data.get_unchecked_mut(index).as_mut_ptr() }
    }
}

/// 最大合法手数（将棋の理論上限）
pub const MAX_MOVES: usize = 600;

/// 固定長の合法手リスト
#[derive(Debug, Clone)]
pub struct MoveList {
    moves: FixedVec<Move, MAX_MOVES>,
}

impl MoveList {
    /// 新しい空の`MoveList`を作成
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self { moves: FixedVec::new() }
    }

    /// リストをクリア
    #[inline]
    pub fn clear(&mut self) {
        self.moves.clear();
    }

    /// 手を追加
    #[inline]
    pub fn push(&mut self, m: Move) {
        self.moves.push(m);
    }

    #[inline]
    pub(crate) unsafe fn push_unchecked(&mut self, m: Move) {
        // SAFETY: 呼び出し側がリストに空きスロットを持つこと（`len < MAX_MOVES`）を保証する。
        unsafe { self.moves.push_unchecked(m) };
    }

    /// 手数を取得
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.moves.len()
    }

    /// 空かどうか判定
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }

    /// イテレータを取得
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Move> {
        self.moves.iter()
    }

    /// スライスを取得
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[Move] {
        self.moves.as_slice()
    }

    /// 条件を満たす手だけを残す
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(Move) -> bool,
    {
        let mut i = 0;
        while i < self.moves.len() {
            let m = self.moves[i];
            if f(m) {
                i += 1;
            } else {
                self.moves.remove(i);
            }
        }
    }

    /// 条件を満たす手だけを残す（順序は保持しない）
    #[inline]
    pub fn retain_unordered<F>(&mut self, mut f: F)
    where
        F: FnMut(Move) -> bool,
    {
        let mut i = 0;
        while i < self.moves.len {
            // SAFETY: i < len を満たすループ条件でのみ参照する。
            let m = unsafe { self.moves.data.get_unchecked(i).assume_init_read() };
            if !f(m) {
                let last = self.moves.len - 1;
                if i != last {
                    // SAFETY: last は有効範囲内。Copy 型なので read/write で問題ない。
                    unsafe {
                        let tail = self.moves.data.get_unchecked(last).assume_init_read();
                        self.moves.data.get_unchecked_mut(i).as_mut_ptr().write(tail);
                    }
                }
                self.moves.len = last;
                continue;
            }
            i += 1;
        }
    }
}

impl Default for MoveList {
    fn default() -> Self {
        Self::new()
    }
}

/// Move32（32bit）の固定長リスト
#[derive(Debug, Clone)]
pub struct Move32List {
    moves: FixedVec<Move32, MAX_MOVES>,
}

impl Move32List {
    /// 新しい空の`Move32List`を作成
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self { moves: FixedVec::new() }
    }

    /// リストをクリア
    #[inline]
    pub fn clear(&mut self) {
        self.moves.clear();
    }

    /// 手を追加
    #[inline]
    pub fn push(&mut self, m: Move32) {
        self.moves.push(m);
    }

    #[inline]
    pub(crate) unsafe fn push_unchecked(&mut self, m: Move32) {
        // SAFETY: 呼び出し側がリストに空きスロットを持つこと（`len < MAX_MOVES`）を保証する。
        unsafe { self.moves.push_unchecked(m) };
    }

    #[inline(always)]
    pub(crate) unsafe fn push_drop1_unchecked(&mut self, d0: u32, to_raw: u32) {
        let len = self.moves.len;
        debug_assert!(len < MAX_MOVES);
        // SAFETY: 呼び出し側が空きスロットを少なくとも 1 つ持ち、drop/to のビットパターンが既にエンコード済みであることを保証する。
        unsafe {
            self.moves
                .data
                .get_unchecked_mut(len)
                .as_mut_ptr()
                .write(Move32::from_raw(d0 | to_raw));
        }
        self.moves.len = len + 1;
    }

    #[inline(always)]
    pub(crate) unsafe fn push_drop2_unchecked(&mut self, drops: [u32; 2], to_raw: u32) {
        let [d0, d1] = drops;
        let len = self.moves.len;
        debug_assert!(len + 2 <= MAX_MOVES);
        // SAFETY: 呼び出し側が空きスロットを少なくとも 2 つ持ち、drop/to のビットパターンが既にエンコード済みであることを保証する。
        unsafe {
            self.moves
                .data
                .get_unchecked_mut(len)
                .as_mut_ptr()
                .write(Move32::from_raw(d0 | to_raw));
            self.moves
                .data
                .get_unchecked_mut(len + 1)
                .as_mut_ptr()
                .write(Move32::from_raw(d1 | to_raw));
        }
        self.moves.len = len + 2;
    }

    #[inline(always)]
    pub(crate) unsafe fn push_drop3_unchecked(&mut self, drops: [u32; 3], to_raw: u32) {
        let [d0, d1, d2] = drops;
        let len = self.moves.len;
        debug_assert!(len + 3 <= MAX_MOVES);
        // SAFETY: 呼び出し側が空きスロットを少なくとも 3 つ持ち、drop/to のビットパターンが既にエンコード済みであることを保証する。
        unsafe {
            self.moves
                .data
                .get_unchecked_mut(len)
                .as_mut_ptr()
                .write(Move32::from_raw(d0 | to_raw));
            self.moves
                .data
                .get_unchecked_mut(len + 1)
                .as_mut_ptr()
                .write(Move32::from_raw(d1 | to_raw));
            self.moves
                .data
                .get_unchecked_mut(len + 2)
                .as_mut_ptr()
                .write(Move32::from_raw(d2 | to_raw));
        }
        self.moves.len = len + 3;
    }

    #[inline(always)]
    pub(crate) unsafe fn push_drop4_unchecked(&mut self, drops: [u32; 4], to_raw: u32) {
        let [d0, d1, d2, d3] = drops;
        let len = self.moves.len;
        debug_assert!(len + 4 <= MAX_MOVES);
        // SAFETY: 呼び出し側が空きスロットを少なくとも 4 つ持ち、drop/to のビットパターンが既にエンコード済みであることを保証する。
        unsafe {
            self.moves
                .data
                .get_unchecked_mut(len)
                .as_mut_ptr()
                .write(Move32::from_raw(d0 | to_raw));
            self.moves
                .data
                .get_unchecked_mut(len + 1)
                .as_mut_ptr()
                .write(Move32::from_raw(d1 | to_raw));
            self.moves
                .data
                .get_unchecked_mut(len + 2)
                .as_mut_ptr()
                .write(Move32::from_raw(d2 | to_raw));
            self.moves
                .data
                .get_unchecked_mut(len + 3)
                .as_mut_ptr()
                .write(Move32::from_raw(d3 | to_raw));
        }
        self.moves.len = len + 4;
    }

    #[inline(always)]
    pub(crate) unsafe fn push_drop5_unchecked(&mut self, drops: [u32; 5], to_raw: u32) {
        let [d0, d1, d2, d3, d4] = drops;
        let len = self.moves.len;
        debug_assert!(len + 5 <= MAX_MOVES);
        // SAFETY: 呼び出し側が空きスロットを少なくとも 5 つ持ち、drop/to のビットパターンが既にエンコード済みであることを保証する。
        unsafe {
            self.moves
                .data
                .get_unchecked_mut(len)
                .as_mut_ptr()
                .write(Move32::from_raw(d0 | to_raw));
            self.moves
                .data
                .get_unchecked_mut(len + 1)
                .as_mut_ptr()
                .write(Move32::from_raw(d1 | to_raw));
            self.moves
                .data
                .get_unchecked_mut(len + 2)
                .as_mut_ptr()
                .write(Move32::from_raw(d2 | to_raw));
            self.moves
                .data
                .get_unchecked_mut(len + 3)
                .as_mut_ptr()
                .write(Move32::from_raw(d3 | to_raw));
            self.moves
                .data
                .get_unchecked_mut(len + 4)
                .as_mut_ptr()
                .write(Move32::from_raw(d4 | to_raw));
        }
        self.moves.len = len + 5;
    }

    #[inline(always)]
    pub(crate) unsafe fn push_drop6_unchecked(&mut self, drops: [u32; 6], to_raw: u32) {
        let [d0, d1, d2, d3, d4, d5] = drops;
        let len = self.moves.len;
        debug_assert!(len + 6 <= MAX_MOVES);
        // SAFETY: 呼び出し側が空きスロットを少なくとも 6 つ持ち、drop/to のビットパターンが既にエンコード済みであることを保証する。
        unsafe {
            self.moves
                .data
                .get_unchecked_mut(len)
                .as_mut_ptr()
                .write(Move32::from_raw(d0 | to_raw));
            self.moves
                .data
                .get_unchecked_mut(len + 1)
                .as_mut_ptr()
                .write(Move32::from_raw(d1 | to_raw));
            self.moves
                .data
                .get_unchecked_mut(len + 2)
                .as_mut_ptr()
                .write(Move32::from_raw(d2 | to_raw));
            self.moves
                .data
                .get_unchecked_mut(len + 3)
                .as_mut_ptr()
                .write(Move32::from_raw(d3 | to_raw));
            self.moves
                .data
                .get_unchecked_mut(len + 4)
                .as_mut_ptr()
                .write(Move32::from_raw(d4 | to_raw));
            self.moves
                .data
                .get_unchecked_mut(len + 5)
                .as_mut_ptr()
                .write(Move32::from_raw(d5 | to_raw));
        }
        self.moves.len = len + 6;
    }

    /// 手数を取得
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.moves.len()
    }

    /// 空かどうか判定
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }

    /// イテレータを取得
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Move32> {
        self.moves.iter()
    }

    /// スライスを取得
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[Move32] {
        self.moves.as_slice()
    }

    /// 条件を満たす手だけを残す（順序は保持しない）
    #[inline]
    pub fn retain_unordered<F>(&mut self, mut f: F)
    where
        F: FnMut(Move32) -> bool,
    {
        let mut i = 0;
        while i < self.moves.len {
            // SAFETY: i < len を満たすループ条件でのみ参照する。
            let m = unsafe { self.moves.data.get_unchecked(i).assume_init_read() };
            if !f(m) {
                let last = self.moves.len - 1;
                if i != last {
                    // SAFETY: last は有効範囲内。Copy 型なので read/write で問題ない。
                    unsafe {
                        let tail = self.moves.data.get_unchecked(last).assume_init_read();
                        self.moves.data.get_unchecked_mut(i).as_mut_ptr().write(tail);
                    }
                }
                self.moves.len = last;
                continue;
            }
            i += 1;
        }
    }
}

impl Default for Move32List {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Move;
    use crate::types::square::{SQ_26, SQ_27, SQ_76, SQ_77};

    #[test]
    fn test_move_list_new() {
        let list = MoveList::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_move_list_push() {
        let mut list = MoveList::new();
        let m = Move::normal(SQ_77, SQ_76);

        list.push(m);
        assert_eq!(list.len(), 1);
        assert!(!list.is_empty());
    }

    #[test]
    fn test_move_list_clear() {
        let mut list = MoveList::new();
        let m = Move::normal(SQ_77, SQ_76);

        list.push(m);
        assert_eq!(list.len(), 1);

        list.clear();
        assert_eq!(list.len(), 0);
        assert!(list.is_empty());
    }

    #[test]
    fn test_move_list_iter() {
        let mut list = MoveList::new();
        let m1 = Move::normal(SQ_77, SQ_76);
        let m2 = Move::normal(SQ_27, SQ_26);

        list.push(m1);
        list.push(m2);

        let moves: Vec<_> = list.iter().copied().collect();
        assert_eq!(moves.len(), 2);
        assert_eq!(moves[0], m1);
        assert_eq!(moves[1], m2);
    }

    #[test]
    #[should_panic(expected = "MoveList capacity exceeded")]
    fn test_move_list_push_over_capacity_panics() {
        let mut list = MoveList::new();
        let m = Move::normal(SQ_77, SQ_76);

        for _ in 0..=MAX_MOVES {
            list.push(m);
        }
    }

    #[test]
    #[should_panic(expected = "MoveList capacity exceeded")]
    fn test_move32_list_push_over_capacity_panics() {
        let mut list = Move32List::new();
        let m = Move32::from_raw(0);

        for _ in 0..=MAX_MOVES {
            list.push(m);
        }
    }
}
