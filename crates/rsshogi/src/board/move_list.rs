use core::mem::MaybeUninit;

use crate::types::{Move, Move32};

/// The largest legal shogi move set fits comfortably below this bound.
const MOVE_LIST_CAPACITY: usize = 600;

macro_rules! fixed_move_list {
    ($name:ident, $move:ty) => {
        pub struct $name {
            moves: [MaybeUninit<$move>; MOVE_LIST_CAPACITY],
            len: usize,
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $name {
            #[must_use]
            pub const fn new() -> Self {
                Self { moves: [const { MaybeUninit::uninit() }; MOVE_LIST_CAPACITY], len: 0 }
            }

            pub fn clear(&mut self) {
                self.len = 0;
            }

            #[must_use]
            pub const fn len(&self) -> usize {
                self.len
            }

            #[must_use]
            pub const fn is_empty(&self) -> bool {
                self.len == 0
            }

            #[must_use]
            pub const fn capacity(&self) -> usize {
                MOVE_LIST_CAPACITY
            }

            pub fn push(&mut self, mv: $move) {
                assert!(self.len < MOVE_LIST_CAPACITY, "move list capacity exceeded");
                self.moves[self.len].write(mv);
                self.len += 1;
            }

            pub(crate) fn ensure_additional_capacity(&self, additional: usize) {
                assert!(additional <= MOVE_LIST_CAPACITY - self.len, "move list capacity exceeded");
            }

            /// # Safety
            ///
            /// The caller must ensure that the active prefix has spare capacity.
            pub(crate) unsafe fn push_unchecked(&mut self, mv: $move) {
                debug_assert!(self.len < MOVE_LIST_CAPACITY);
                self.moves[self.len].write(mv);
                self.len += 1;
            }

            /// Keeps the moves for which `f` returns true, preserving their relative order.
            pub fn retain<F>(&mut self, mut f: F)
            where
                F: FnMut(&$move) -> bool,
            {
                let mut write = 0;
                for read in 0..self.len {
                    // SAFETY: every element in the active prefix was initialized by `push`.
                    let mv = unsafe { self.moves[read].assume_init_read() };
                    if f(&mv) {
                        self.moves[write].write(mv);
                        write += 1;
                    }
                }
                self.len = write;
            }

            /// Keeps the moves for which `f` returns true without preserving their order.
            pub fn retain_unordered<F>(&mut self, mut f: F)
            where
                F: FnMut($move) -> bool,
            {
                let mut index = 0;
                while index < self.len {
                    // SAFETY: every element in the active prefix was initialized by `push`.
                    let mv = unsafe { self.moves[index].assume_init_read() };
                    if f(mv) {
                        self.moves[index].write(mv);
                        index += 1;
                    } else {
                        self.len -= 1;
                        if index < self.len {
                            // SAFETY: the last element remains in the active initialized prefix.
                            let last = unsafe { self.moves[self.len].assume_init_read() };
                            self.moves[index].write(last);
                        }
                    }
                }
            }

            #[must_use]
            pub fn as_slice(&self) -> &[$move] {
                // SAFETY: only the first `len` elements are exposed and every one is written by push.
                unsafe {
                    core::slice::from_raw_parts(self.moves.as_ptr().cast::<$move>(), self.len)
                }
            }

            #[must_use]
            pub fn as_mut_slice(&mut self) -> &mut [$move] {
                // SAFETY: `push` initializes every element in the exposed prefix.
                unsafe {
                    core::slice::from_raw_parts_mut(
                        self.moves.as_mut_ptr().cast::<$move>(),
                        self.len,
                    )
                }
            }

            pub fn iter(&self) -> core::slice::Iter<'_, $move> {
                self.as_slice().iter()
            }
        }

        impl Clone for $name {
            fn clone(&self) -> Self {
                let mut out = Self::new();
                for &mv in self.iter() {
                    out.push(mv);
                }
                out
            }
        }

        impl core::fmt::Debug for $name
        where
            $move: core::fmt::Debug,
        {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.as_slice().fmt(f)
            }
        }

        impl AsRef<[$move]> for $name {
            fn as_ref(&self) -> &[$move] {
                self.as_slice()
            }
        }

        impl core::ops::Deref for $name {
            type Target = [$move];

            fn deref(&self) -> &Self::Target {
                self.as_slice()
            }
        }

        impl<'a> IntoIterator for &'a $name {
            type Item = &'a $move;
            type IntoIter = core::slice::Iter<'a, $move>;

            fn into_iter(self) -> Self::IntoIter {
                self.iter()
            }
        }
    };
}

fixed_move_list!(MoveList, Move);
fixed_move_list!(Move32List, Move32);
