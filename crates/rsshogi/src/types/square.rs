use super::{File, Rank};
use core::{
    fmt,
    ops::{Index, IndexMut},
    str::FromStr,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Square(i8);

const fn raw_to_file() -> [i8; 256] {
    let mut values = [0; 256];
    let mut index = 0;
    while index < values.len() {
        values[index] = (index as u8 as i8) / 9;
        index += 1;
    }
    values
}

const fn raw_to_rank() -> [i8; 256] {
    let mut values = [0; 256];
    let mut index = 0;
    while index < values.len() {
        values[index] = (index as u8 as i8) % 9;
        index += 1;
    }
    values
}

static RAW_TO_FILE: [i8; 256] = raw_to_file();
static RAW_TO_RANK: [i8; 256] = raw_to_rank();

impl Square {
    pub const COUNT: usize = 81;
    pub const COUNT_WITH_NONE: usize = 82;

    pub fn iter() -> impl Iterator<Item = Self> {
        (0..Self::COUNT).map(Self::from_index)
    }
    #[must_use]
    pub const fn new(raw: i8) -> Self {
        Self(raw)
    }
    #[must_use]
    pub const fn raw(self) -> i8 {
        self.0
    }
    #[must_use]
    pub const fn to_index(self) -> usize {
        self.0 as usize
    }
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 >= 0 && self.0 < Self::COUNT as i8
    }
    #[must_use]
    pub const fn is_on_board(self) -> bool {
        self.is_valid()
    }
    #[must_use]
    pub const fn from_index(index: usize) -> Self {
        Self(index as i8)
    }
    #[must_use]
    pub const fn to_board_index(self) -> usize {
        self.0 as usize
    }
    #[must_use]
    pub const fn to_index_with_none(self) -> usize {
        self.0 as usize
    }
    #[must_use]
    pub const fn is_none(self) -> bool {
        self.0 == 81
    }
    #[must_use]
    pub const fn file(self) -> File {
        File::new(RAW_TO_FILE[self.0 as u8 as usize])
    }
    #[must_use]
    pub const fn rank(self) -> Rank {
        Rank::new(RAW_TO_RANK[self.0 as u8 as usize])
    }
    #[must_use]
    pub const fn from_file_rank(file: File, rank: Rank) -> Self {
        Self(file.raw() * 9 + rank.raw())
    }
    #[must_use]
    pub const fn distance(self, other: Self) -> i8 {
        let df = self.file().raw() - other.file().raw();
        let dr = self.rank().raw() - other.rank().raw();
        let af = if df < 0 { -df } else { df };
        let ar = if dr < 0 { -dr } else { dr };
        if af > ar { af } else { ar }
    }
    #[must_use]
    pub const fn flip(self) -> Self {
        flip(self)
    }
    #[must_use]
    pub fn from_usi(value: &str) -> Option<Self> {
        value.parse().ok()
    }

    #[must_use]
    pub const fn mirror_file(self) -> Self {
        Self::from_file_rank(File::new(8 - self.file().raw()), self.rank())
    }
    #[must_use]
    pub fn to_usi(self) -> String {
        self.to_string()
    }
}
impl FromStr for Square {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "none" {
            return Ok(Self::NONE);
        }
        let b = value.as_bytes();
        if b.len() != 2 || !(b'1'..=b'9').contains(&b[0]) || !(b'a'..=b'i').contains(&b[1]) {
            return Err("invalid USI square");
        }
        Ok(Self::from_file_rank(File::new((b[0] - b'1') as i8), Rank::new((b[1] - b'a') as i8)))
    }
}
impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.file(), self.rank())
    }
}

pub const SQ_ZERO: Square = Square::new(0);
pub const SQ_NONE: Square = Square::new(81);
impl Square {
    pub const NONE: Self = SQ_NONE;
}
pub const SQ_D: i8 = 1;
pub const SQ_U: i8 = -1;
pub const SQ_R: i8 = -9;
pub const SQ_L: i8 = 9;
pub const SQ_RU: i8 = -10;
pub const SQ_RD: i8 = -8;
pub const SQ_LU: i8 = 8;
pub const SQ_LD: i8 = 10;
macro_rules! squares { ($($name:ident = $raw:expr),* $(,)?) => { $(pub const $name: Square = Square::new($raw);)* }; }
squares!(
    SQ_11 = 0,
    SQ_12 = 1,
    SQ_13 = 2,
    SQ_14 = 3,
    SQ_15 = 4,
    SQ_16 = 5,
    SQ_17 = 6,
    SQ_18 = 7,
    SQ_19 = 8,
    SQ_21 = 9,
    SQ_22 = 10,
    SQ_23 = 11,
    SQ_24 = 12,
    SQ_25 = 13,
    SQ_26 = 14,
    SQ_27 = 15,
    SQ_28 = 16,
    SQ_29 = 17,
    SQ_31 = 18,
    SQ_32 = 19,
    SQ_33 = 20,
    SQ_34 = 21,
    SQ_35 = 22,
    SQ_36 = 23,
    SQ_37 = 24,
    SQ_38 = 25,
    SQ_39 = 26,
    SQ_41 = 27,
    SQ_42 = 28,
    SQ_43 = 29,
    SQ_44 = 30,
    SQ_45 = 31,
    SQ_46 = 32,
    SQ_47 = 33,
    SQ_48 = 34,
    SQ_49 = 35,
    SQ_51 = 36,
    SQ_52 = 37,
    SQ_53 = 38,
    SQ_54 = 39,
    SQ_55 = 40,
    SQ_56 = 41,
    SQ_57 = 42,
    SQ_58 = 43,
    SQ_59 = 44,
    SQ_61 = 45,
    SQ_62 = 46,
    SQ_63 = 47,
    SQ_64 = 48,
    SQ_65 = 49,
    SQ_66 = 50,
    SQ_67 = 51,
    SQ_68 = 52,
    SQ_69 = 53,
    SQ_71 = 54,
    SQ_72 = 55,
    SQ_73 = 56,
    SQ_74 = 57,
    SQ_75 = 58,
    SQ_76 = 59,
    SQ_77 = 60,
    SQ_78 = 61,
    SQ_79 = 62,
    SQ_81 = 63,
    SQ_82 = 64,
    SQ_83 = 65,
    SQ_84 = 66,
    SQ_85 = 67,
    SQ_86 = 68,
    SQ_87 = 69,
    SQ_88 = 70,
    SQ_89 = 71,
    SQ_91 = 72,
    SQ_92 = 73,
    SQ_93 = 74,
    SQ_94 = 75,
    SQ_95 = 76,
    SQ_96 = 77,
    SQ_97 = 78,
    SQ_98 = 79,
    SQ_99 = 80
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SquareTable<T, const N: usize>([T; N]);
impl<T, const N: usize> SquareTable<T, N> {
    pub const fn new(values: [T; N]) -> Self {
        Self(values)
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        N
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        N == 0
    }
    #[must_use]
    pub const fn as_slice(&self) -> &[T] {
        &self.0
    }
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.0.iter()
    }
}
impl<T, const N: usize> Index<usize> for SquareTable<T, N> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}
impl<T, const N: usize> Index<Square> for SquareTable<T, N> {
    type Output = T;
    fn index(&self, index: Square) -> &Self::Output {
        &self.0[index.to_index()]
    }
}
impl<T, const N: usize> IndexMut<usize> for SquareTable<T, N> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}
impl<T, const N: usize> IntoIterator for SquareTable<T, N> {
    type Item = T;
    type IntoIter = core::array::IntoIter<T, N>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
impl<'a, T, const N: usize> IntoIterator for &'a SquareTable<T, N> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}
#[must_use]
pub const fn flip(square: Square) -> Square {
    Square::new(80 - square.raw())
}
#[must_use]
pub const fn is_promotable_move(color: super::Color, from: Square, to: Square) -> bool {
    match color {
        super::Color::BLACK => from.rank().raw() <= 2 || to.rank().raw() <= 2,
        super::Color::WHITE => from.rank().raw() >= 6 || to.rank().raw() >= 6,
    }
}
#[must_use]
pub const fn is_promotable_square(color: super::Color, square: Square) -> bool {
    match color {
        super::Color::BLACK => square.rank().raw() <= 2,
        super::Color::WHITE => square.rank().raw() >= 6,
    }
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use super::{SQ_11, SQ_91, SQ_99, Square};

    #[test]
    fn usi_and_file_mirror_are_fallible_and_symmetric() {
        assert_eq!(Square::from_usi("1a"), Some(SQ_11));
        assert_eq!(Square::from_usi("9a"), Some(SQ_91));
        assert_eq!(Square::from_usi("0a"), None);
        assert_eq!(SQ_11.mirror_file(), SQ_91);
        assert_eq!(SQ_11.mirror_file().mirror_file(), SQ_11);
    }

    #[test]
    fn iterator_visits_all_valid_squares_in_raw_order() {
        let squares: Vec<_> = Square::iter().collect();
        assert_eq!(squares.len(), Square::COUNT);
        assert_eq!(squares.first(), Some(&SQ_11));
        assert_eq!(squares.last(), Some(&SQ_99));
        assert!(squares.iter().enumerate().all(|(raw, square)| square.raw() == raw as i8));
    }

    #[test]
    fn from_str_accepts_none_and_reports_python_compatible_errors() {
        assert_eq!(Square::from_str("none"), Ok(Square::NONE));
        assert_eq!(Square::from_str("1j"), Err("invalid USI square"));
    }

    #[test]
    fn coordinate_lookups_match_signed_arithmetic_for_every_raw_value() {
        for raw in i8::MIN..=i8::MAX {
            let square = Square::new(raw);
            assert_eq!(square.file().raw(), raw / 9, "raw file {raw}");
            assert_eq!(square.rank().raw(), raw % 9, "raw rank {raw}");
        }
    }

    #[test]
    fn all_squares_preserve_file_major_mapping() {
        for square in Square::iter() {
            assert_eq!(square.raw(), square.file().raw() * 9 + square.rank().raw());
            assert_eq!(Square::from_file_rank(square.file(), square.rank()), square);
        }
    }
}
