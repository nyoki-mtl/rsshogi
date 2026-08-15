use super::Color;
use core::{fmt, str::FromStr};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PieceType(i8);
impl PieceType {
    pub const NONE: Self = Self(0);
    pub const PAWN: Self = Self(1);
    pub const LANCE: Self = Self(2);
    pub const KNIGHT: Self = Self(3);
    pub const SILVER: Self = Self(4);
    pub const BISHOP: Self = Self(5);
    pub const ROOK: Self = Self(6);
    pub const GOLD: Self = Self(7);
    pub const KING: Self = Self(8);
    pub const PRO_PAWN: Self = Self(9);
    pub const PRO_LANCE: Self = Self(10);
    pub const PRO_KNIGHT: Self = Self(11);
    pub const PRO_SILVER: Self = Self(12);
    pub const HORSE: Self = Self(13);
    pub const DRAGON: Self = Self(14);
    pub const GOLD_LIKE: Self = Self(15);
    pub const PIECE_TYPE_PROMOTE: i8 = 8;
    pub const COUNT: usize = 16;
    pub const PIECE_HAND_ZERO: Self = Self::PAWN;
    pub const HAND_TABLE_SIZE: usize = 8;
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
    pub const fn is_promoted(self) -> bool {
        matches!(self.0, 9..=14)
    }
    pub const fn is_promotable(self) -> bool {
        matches!(self.0, 1..=6)
    }
    pub const fn is_unpromotable(self) -> bool {
        !self.is_promotable()
    }
    pub const fn promote(self) -> Self {
        Self::new(self.raw() + if self.is_promotable() { 8 } else { 0 })
    }
    pub const fn demote(self) -> Self {
        Self::new(self.raw() - if self.is_promoted() { 8 } else { 0 })
    }
    pub const fn is_hand_piece(self) -> bool {
        matches!(self.0, 1..=7)
    }
    pub const fn is_minor(self) -> bool {
        matches!(self.0, 2..=4 | 7 | 9..=12)
    }
    pub fn iter() -> impl Iterator<Item = Self> {
        (0..Self::GOLD_LIKE.raw()).map(Self::new)
    }
    pub fn raw_pieces() -> impl DoubleEndedIterator<Item = Self> + ExactSizeIterator {
        (Self::PAWN.raw()..=Self::KING.raw()).map(Self::new)
    }
    pub fn hand_pieces() -> impl DoubleEndedIterator<Item = Self> + ExactSizeIterator {
        (1..Self::HAND_TABLE_SIZE as i8).map(Self::new)
    }
}
impl TryFrom<i8> for PieceType {
    type Error = &'static str;
    fn try_from(v: i8) -> Result<Self, Self::Error> {
        if Self::new(v).is_valid() { Ok(Self::new(v)) } else { Err("invalid piece type") }
    }
}
impl fmt::Display for PieceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.0 {
            1 => "P",
            2 => "L",
            3 => "N",
            4 => "S",
            5 => "B",
            6 => "R",
            7 => "G",
            8 => "K",
            9 => "+P",
            10 => "+L",
            11 => "+N",
            12 => "+S",
            13 => "+B",
            14 => "+R",
            _ => "",
        })
    }
}
impl FromStr for PieceType {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let plus = s.starts_with('+');
        match s.trim_start_matches('+').to_ascii_uppercase().as_str() {
            "P" => Ok(if plus { Self::PRO_PAWN } else { Self::PAWN }),
            "L" => Ok(if plus { Self::PRO_LANCE } else { Self::LANCE }),
            "N" => Ok(if plus { Self::PRO_KNIGHT } else { Self::KNIGHT }),
            "S" => Ok(if plus { Self::PRO_SILVER } else { Self::SILVER }),
            "B" => Ok(if plus { Self::HORSE } else { Self::BISHOP }),
            "R" => Ok(if plus { Self::DRAGON } else { Self::ROOK }),
            "G" => Ok(Self::GOLD),
            "K" => Ok(Self::KING),
            _ => Err("invalid piece type"),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Piece(i8);
impl Piece {
    pub const NONE: Self = Self(0);
    pub const COLOR_BIT: i8 = 16;
    pub const PROMOTE_OFFSET: i8 = 8;
    pub const COUNT: usize = 32;
    pub const HAND_TABLE_SIZE: usize = 8;
    pub const RAW_COUNT: usize = 8;
}
macro_rules! pieces{($($n:ident=$v:expr),*)=>{$(pub const $n:Piece=Piece($v);)*};}
pieces!(
    B_PAWN = 1,
    B_LANCE = 2,
    B_KNIGHT = 3,
    B_SILVER = 4,
    B_BISHOP = 5,
    B_ROOK = 6,
    B_GOLD = 7,
    B_KING = 8,
    B_PRO_PAWN = 9,
    B_PRO_LANCE = 10,
    B_PRO_KNIGHT = 11,
    B_PRO_SILVER = 12,
    B_HORSE = 13,
    B_DRAGON = 14,
    B_GOLD_LIKE = 15,
    W_PAWN = 17,
    W_LANCE = 18,
    W_KNIGHT = 19,
    W_SILVER = 20,
    W_BISHOP = 21,
    W_ROOK = 22,
    W_GOLD = 23,
    W_KING = 24,
    W_PRO_PAWN = 25,
    W_PRO_LANCE = 26,
    W_PRO_KNIGHT = 27,
    W_PRO_SILVER = 28,
    W_HORSE = 29,
    W_DRAGON = 30,
    W_GOLD_LIKE = 31
);
impl Piece {
    pub const B_PAWN: Self = B_PAWN;
    pub const B_LANCE: Self = B_LANCE;
    pub const B_KNIGHT: Self = B_KNIGHT;
    pub const B_SILVER: Self = B_SILVER;
    pub const B_BISHOP: Self = B_BISHOP;
    pub const B_ROOK: Self = B_ROOK;
    pub const B_GOLD: Self = B_GOLD;
    pub const B_KING: Self = B_KING;
    pub const B_PRO_PAWN: Self = B_PRO_PAWN;
    pub const B_PRO_LANCE: Self = B_PRO_LANCE;
    pub const B_PRO_KNIGHT: Self = B_PRO_KNIGHT;
    pub const B_PRO_SILVER: Self = B_PRO_SILVER;
    pub const B_HORSE: Self = B_HORSE;
    pub const B_DRAGON: Self = B_DRAGON;
    pub const B_GOLD_LIKE: Self = B_GOLD_LIKE;
    pub const W_PAWN: Self = W_PAWN;
    pub const W_LANCE: Self = W_LANCE;
    pub const W_KNIGHT: Self = W_KNIGHT;
    pub const W_SILVER: Self = W_SILVER;
    pub const W_BISHOP: Self = W_BISHOP;
    pub const W_ROOK: Self = W_ROOK;
    pub const W_GOLD: Self = W_GOLD;
    pub const W_KING: Self = W_KING;
    pub const W_PRO_PAWN: Self = W_PRO_PAWN;
    pub const W_PRO_LANCE: Self = W_PRO_LANCE;
    pub const W_PRO_KNIGHT: Self = W_PRO_KNIGHT;
    pub const W_PRO_SILVER: Self = W_PRO_SILVER;
    pub const W_HORSE: Self = W_HORSE;
    pub const W_DRAGON: Self = W_DRAGON;
    pub const W_GOLD_LIKE: Self = W_GOLD_LIKE;
    pub const fn new(v: i8) -> Self {
        Self(v)
    }
    pub const fn raw(self) -> i8 {
        self.0
    }
    pub const fn to_index(self) -> usize {
        self.0 as usize
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub const fn color(self) -> Color {
        Color::new((self.0 as u8 >> 4) & 1)
    }
    pub const fn piece_type(self) -> PieceType {
        PieceType::new(self.0 & 15)
    }
    pub const fn from_parts(c: Color, p: PieceType) -> Self {
        Self(c.raw() * 16 + p.raw())
    }
    pub const fn promote(self) -> Self {
        Self::from_parts(self.color(), self.piece_type().promote())
    }
    pub const fn demote(self) -> Self {
        Self::from_parts(self.color(), self.piece_type().demote())
    }
    pub const fn base_piece_type(self) -> PieceType {
        self.piece_type().demote()
    }
    pub const fn unpromoted_piece(self) -> Self {
        self.demote()
    }
    pub const fn is_promoted(self) -> bool {
        self.piece_type().is_promoted()
    }
    pub const fn is_unpromotable(self) -> bool {
        self.piece_type().is_unpromotable()
    }
    pub const fn is_long_range(self) -> bool {
        matches!(self.piece_type().raw(), 2 | 5 | 6 | 13 | 14)
    }
    pub const fn is_valid(self) -> bool {
        self.0 >= 0 && self.0 < Self::COUNT as i8 && self.0 != Self::COLOR_BIT
    }
}
impl TryFrom<i8> for Piece {
    type Error = &'static str;
    fn try_from(v: i8) -> Result<Self, Self::Error> {
        if Self(v).is_valid() { Ok(Self(v)) } else { Err("invalid piece") }
    }
}
impl FromStr for Piece {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = s.as_bytes();
        let symbol_index = match bytes {
            [b'+', _] => 1,
            [_] => 0,
            _ => return Err("invalid piece"),
        };
        let piece_type = PieceType::from_str(s)?;
        let symbol = bytes[symbol_index] as char;
        let color = if symbol.is_ascii_lowercase() { Color::WHITE } else { Color::BLACK };
        Ok(Self::from_parts(color, piece_type))
    }
}

impl fmt::Display for Piece {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.piece_type().to_string();
        if self.is_empty() {
            Ok(())
        } else if self.color() == Color::WHITE {
            f.write_str(&s.to_ascii_lowercase())
        } else {
            f.write_str(&s)
        }
    }
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use super::{Piece, PieceType};

    #[test]
    fn piece_type_raw_and_minor_contracts() {
        let minor = [2, 3, 4, 7, 9, 10, 11, 12];
        for raw in 0..PieceType::COUNT as i8 {
            assert_eq!(PieceType::new(raw).is_minor(), minor.contains(&raw));
        }
        assert_eq!(PieceType::new(-1).raw(), -1);
        assert!(!PieceType::new(-1).is_valid());
    }

    #[test]
    fn piece_helpers_preserve_color_and_base_type() {
        assert_eq!(Piece::W_HORSE.base_piece_type(), PieceType::BISHOP);
        assert_eq!(Piece::W_HORSE.unpromoted_piece(), Piece::W_BISHOP);
        assert!(Piece::B_ROOK.is_long_range());
        assert!(!Piece::new(16).is_valid());
    }

    #[test]
    fn piece_from_str_preserves_color_and_promotion() {
        assert_eq!(Piece::from_str("P"), Ok(Piece::B_PAWN));
        assert_eq!(Piece::from_str("p"), Ok(Piece::W_PAWN));
        assert_eq!(Piece::from_str("+B"), Ok(Piece::B_HORSE));
        assert_eq!(Piece::from_str("+r"), Ok(Piece::W_DRAGON));
        assert!(Piece::from_str("").is_err());
        assert!(Piece::from_str("PP").is_err());
        assert!(Piece::from_str("++P").is_err());
    }

    #[test]
    fn public_piece_type_iterator_excludes_internal_gold_like() {
        let types: Vec<_> = PieceType::iter().collect();
        assert_eq!(types.len(), 15);
        assert_eq!(types.first(), Some(&PieceType::NONE));
        assert_eq!(types.last(), Some(&PieceType::DRAGON));
        assert!(!types.contains(&PieceType::GOLD_LIKE));
        assert_eq!(
            PieceType::raw_pieces().collect::<Vec<_>>(),
            vec![
                PieceType::PAWN,
                PieceType::LANCE,
                PieceType::KNIGHT,
                PieceType::SILVER,
                PieceType::BISHOP,
                PieceType::ROOK,
                PieceType::GOLD,
                PieceType::KING,
            ]
        );
        assert_eq!(
            PieceType::hand_pieces().collect::<Vec<_>>(),
            vec![
                PieceType::PAWN,
                PieceType::LANCE,
                PieceType::KNIGHT,
                PieceType::SILVER,
                PieceType::BISHOP,
                PieceType::ROOK,
                PieceType::GOLD,
            ]
        );
    }
}
