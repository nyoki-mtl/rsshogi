#![allow(clippy::missing_const_for_fn)]

use crate::types::Bitboard;
#[repr(align(32))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct U64x4 {
    p: [u64; 4],
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

    #[must_use]
    pub fn from_bitboards(b1: Bitboard, b2: Bitboard) -> Self {
        let [p0, p1] = b1.parts();
        let [p2, p3] = b2.parts();
        Self::new(p0, p1, p2, p3)
    }

    #[must_use]
    pub fn splat_bitboard(bb: Bitboard) -> Self {
        let [p0, p1] = bb.parts();
        Self::new(p0, p1, p0, p1)
    }

    #[must_use]
    pub fn and(self, other: Self) -> Self {
        Self::new(
            self.p[0] & other.p[0],
            self.p[1] & other.p[1],
            self.p[2] & other.p[2],
            self.p[3] & other.p[3],
        )
    }

    #[must_use]
    pub fn wrapping_add(self, other: Self) -> Self {
        Self::new(
            self.p[0].wrapping_add(other.p[0]),
            self.p[1].wrapping_add(other.p[1]),
            self.p[2].wrapping_add(other.p[2]),
            self.p[3].wrapping_add(other.p[3]),
        )
    }

    #[must_use]
    pub fn wrapping_sub(self, other: Self) -> Self {
        Self::new(
            self.p[0].wrapping_sub(other.p[0]),
            self.p[1].wrapping_sub(other.p[1]),
            self.p[2].wrapping_sub(other.p[2]),
            self.p[3].wrapping_sub(other.p[3]),
        )
    }

    #[must_use]
    pub fn shift_left(self, shift: u32) -> Self {
        Self::new(self.p[0] << shift, self.p[1] << shift, self.p[2] << shift, self.p[3] << shift)
    }

    #[must_use]
    pub fn shift_right(self, shift: u32) -> Self {
        Self::new(self.p[0] >> shift, self.p[1] >> shift, self.p[2] >> shift, self.p[3] >> shift)
    }

    #[must_use]
    pub fn or(self, other: Self) -> Self {
        Self::new(
            self.p[0] | other.p[0],
            self.p[1] | other.p[1],
            self.p[2] | other.p[2],
            self.p[3] | other.p[3],
        )
    }

    #[must_use]
    pub fn xor(self, other: Self) -> Self {
        Self::new(
            self.p[0] ^ other.p[0],
            self.p[1] ^ other.p[1],
            self.p[2] ^ other.p[2],
            self.p[3] ^ other.p[3],
        )
    }

    #[must_use]
    pub fn and_not(self, other: Self) -> Self {
        Self::new(
            self.p[0] & !other.p[0],
            self.p[1] & !other.p[1],
            self.p[2] & !other.p[2],
            self.p[3] & !other.p[3],
        )
    }

    #[must_use]
    pub fn byte_reverse(self) -> Self {
        Self::new(
            self.p[1].swap_bytes(),
            self.p[0].swap_bytes(),
            self.p[3].swap_bytes(),
            self.p[2].swap_bytes(),
        )
    }

    #[must_use]
    pub fn unpack(hi_in: Self, lo_in: Self) -> (Self, Self) {
        let hi_out = Self::new(lo_in.p[1], hi_in.p[1], lo_in.p[3], hi_in.p[3]);
        let lo_out = Self::new(lo_in.p[0], hi_in.p[0], lo_in.p[2], hi_in.p[2]);
        (hi_out, lo_out)
    }

    #[must_use]
    pub fn decrement_pair(hi_in: Self, lo_in: Self) -> (Self, Self) {
        let hi_out = Self::new(
            hi_in.p[0].wrapping_sub(u64::from(lo_in.p[0] == 0)),
            hi_in.p[1].wrapping_sub(u64::from(lo_in.p[1] == 0)),
            hi_in.p[2].wrapping_sub(u64::from(lo_in.p[2] == 0)),
            hi_in.p[3].wrapping_sub(u64::from(lo_in.p[3] == 0)),
        );
        let lo_out = Self::new(
            lo_in.p[0].wrapping_sub(1),
            lo_in.p[1].wrapping_sub(1),
            lo_in.p[2].wrapping_sub(1),
            lo_in.p[3].wrapping_sub(1),
        );
        (hi_out, lo_out)
    }

    #[must_use]
    pub fn merge(self) -> [u64; 2] {
        [self.p[0] | self.p[2], self.p[1] | self.p[3]]
    }
}
