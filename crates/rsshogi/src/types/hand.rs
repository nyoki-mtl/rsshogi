use super::PieceType;
pub const HAND_BIT_MASK: u32 =
    0x1f | (0x7 << 8) | (0x7 << 12) | (0x7 << 16) | (0x3 << 20) | (0x3 << 24) | (0x7 << 28);
pub const HAND_BORROW_MASK: u32 =
    (1 << 5) | (1 << 11) | (1 << 15) | (1 << 19) | (1 << 22) | (1 << 26) | (1 << 31);
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct HandPiece(i8);
impl HandPiece {
    pub const PAWN: Self = Self(0);
    pub const LANCE: Self = Self(1);
    pub const KNIGHT: Self = Self(2);
    pub const SILVER: Self = Self(3);
    pub const BISHOP: Self = Self(4);
    pub const ROOK: Self = Self(5);
    pub const GOLD: Self = Self(6);
    pub const COUNT: usize = 7;
    pub const fn new(raw: i8) -> Self {
        Self(raw)
    }
    pub const fn raw(self) -> i8 {
        self.0
    }
    pub const fn to_index(self) -> usize {
        self.0 as usize
    }
    pub const fn is_valid(self) -> bool {
        self.0 >= 0 && self.0 < Self::COUNT as i8
    }
    pub const fn to_piece_type(self) -> PieceType {
        PieceType::new(self.0 + 1)
    }
    pub const fn from_piece_type(pt: PieceType) -> Option<Self> {
        match pt.raw() {
            1..=7 => Some(Self(pt.raw() - 1)),
            _ => None,
        }
    }
    pub fn iter() -> impl Iterator<Item = Self> {
        [Self::PAWN, Self::LANCE, Self::KNIGHT, Self::SILVER, Self::BISHOP, Self::ROOK, Self::GOLD]
            .into_iter()
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Hand(u32);
impl Hand {
    pub const ZERO: Self = Self(0);
    pub const fn raw(self) -> u32 {
        self.0
    }

    pub const fn bits(self) -> u32 {
        self.0
    }
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
    const fn shift(p: HandPiece) -> u32 {
        [0, 8, 12, 16, 20, 24, 28][p.to_index()]
    }
    const fn max(p: HandPiece) -> u32 {
        [31, 7, 7, 7, 3, 3, 7][p.to_index()]
    }
    pub const fn count_mask(p: HandPiece) -> u32 {
        Self::max(p)
    }
    pub const fn count(self, p: HandPiece) -> u32 {
        (self.0 >> Self::shift(p)) & Self::max(p)
    }
    pub const fn has(self, piece: PieceType) -> bool {
        match HandPiece::from_piece_type(piece) {
            Some(hand_piece) => self.count(hand_piece) != 0,
            None => false,
        }
    }
    pub const fn has_overflow(self) -> bool {
        self.0 & HAND_BORROW_MASK != 0
    }
    pub const fn count_of(hand: Self, p: HandPiece) -> u32 {
        hand.count(p)
    }
    pub fn add(&mut self, p: HandPiece, n: u32) {
        *self = self.checked_add(p, n).expect("hand count overflow")
    }
    pub fn sub(&mut self, p: HandPiece, n: u32) {
        *self = self.checked_sub(p, n).expect("hand count underflow")
    }
    pub const fn add_one(hand: Self, p: HandPiece) -> Self {
        match hand.checked_add(p, 1) {
            Some(v) => v,
            None => hand,
        }
    }
    pub const fn sub_one(hand: Self, p: HandPiece) -> Self {
        match hand.checked_sub(p, 1) {
            Some(v) => v,
            None => hand,
        }
    }
    pub const fn checked_add(self, p: HandPiece, n: u32) -> Option<Self> {
        if n > Self::max(p) - self.count(p) {
            None
        } else {
            Some(Self(self.0 + (n << Self::shift(p))))
        }
    }
    pub const fn checked_sub(self, p: HandPiece, n: u32) -> Option<Self> {
        if n > self.count(p) { None } else { Some(Self(self.0 - (n << Self::shift(p)))) }
    }
    pub const fn is_equal_or_superior(a: Self, b: Self) -> bool {
        a.count(HandPiece::PAWN) >= b.count(HandPiece::PAWN)
            && a.count(HandPiece::LANCE) >= b.count(HandPiece::LANCE)
            && a.count(HandPiece::KNIGHT) >= b.count(HandPiece::KNIGHT)
            && a.count(HandPiece::SILVER) >= b.count(HandPiece::SILVER)
            && a.count(HandPiece::BISHOP) >= b.count(HandPiece::BISHOP)
            && a.count(HandPiece::ROOK) >= b.count(HandPiece::ROOK)
            && a.count(HandPiece::GOLD) >= b.count(HandPiece::GOLD)
    }
    pub const fn pawn_count(self) -> u32 {
        self.count(HandPiece::PAWN)
    }
    pub const fn lance_count(self) -> u32 {
        self.count(HandPiece::LANCE)
    }
    pub const fn knight_count(self) -> u32 {
        self.count(HandPiece::KNIGHT)
    }
    pub const fn silver_count(self) -> u32 {
        self.count(HandPiece::SILVER)
    }
    pub const fn bishop_count(self) -> u32 {
        self.count(HandPiece::BISHOP)
    }
    pub const fn rook_count(self) -> u32 {
        self.count(HandPiece::ROOK)
    }
    pub const fn gold_count(self) -> u32 {
        self.count(HandPiece::GOLD)
    }
}

#[cfg(test)]
mod tests {
    use super::{Hand, HandPiece, PieceType};

    #[test]
    fn bits_exposes_the_packed_raw_value() {
        let hand = Hand::ZERO.checked_add(HandPiece::PAWN, 3).unwrap();
        assert_eq!(hand.bits(), hand.raw());
        assert_eq!(Hand::from_raw(hand.bits()), hand);
    }

    #[test]
    fn raw_piece_and_overflow_contracts() {
        assert!(HandPiece::new(6).is_valid());
        assert!(!HandPiece::new(7).is_valid());
        let hand = Hand::from_bits(1);
        assert!(hand.has(super::PieceType::PAWN));
        assert!(!hand.has_overflow());
        assert!(Hand::from_bits(super::HAND_BORROW_MASK).has_overflow());
    }

    #[test]
    fn promoted_pieces_are_not_hand_pieces() {
        let hand = Hand::ZERO.checked_add(HandPiece::PAWN, 1).unwrap();
        assert_eq!(HandPiece::from_piece_type(PieceType::PRO_PAWN), None);
        assert!(hand.has(PieceType::PAWN));
        assert!(!hand.has(PieceType::PRO_PAWN));
    }

    #[test]
    #[should_panic(expected = "hand count overflow")]
    fn add_panics_instead_of_corrupting_an_adjacent_field() {
        let mut hand = Hand::ZERO;
        hand.add(HandPiece::ROOK, 4);
    }

    #[test]
    #[should_panic(expected = "hand count underflow")]
    fn sub_panics_instead_of_borrowing_from_an_adjacent_field() {
        let mut hand = Hand::ZERO;
        hand.sub(HandPiece::PAWN, 1);
    }
}
