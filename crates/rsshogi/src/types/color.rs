use core::{fmt, ops::Not, str::FromStr};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Color {
    #[default]
    Black = 0,
    White = 1,
}
impl Color {
    pub const BLACK: Self = Self::Black;
    pub const WHITE: Self = Self::White;
    pub const ZERO: Self = Self::BLACK;
    pub const COUNT: usize = 2;
    #[must_use]
    pub const fn new(raw: u8) -> Self {
        if raw == 0 { Self::BLACK } else { Self::WHITE }
    }
    #[must_use]
    pub const fn from_i8(raw: i8) -> Option<Self> {
        match raw {
            0 => Some(Self::BLACK),
            1 => Some(Self::WHITE),
            _ => None,
        }
    }
    #[must_use]
    pub const fn raw(self) -> i8 {
        self as i8
    }
    #[must_use]
    pub const fn to_index(self) -> usize {
        self as usize
    }
    #[must_use]
    pub const fn flip(self) -> Self {
        match self {
            Self::BLACK => Self::WHITE,
            Self::WHITE => Self::BLACK,
        }
    }
    pub fn iter() -> impl DoubleEndedIterator<Item = Self> + ExactSizeIterator {
        [Self::BLACK, Self::WHITE].into_iter()
    }
}
impl TryFrom<u8> for Color {
    type Error = &'static str;
    fn try_from(raw: u8) -> Result<Self, Self::Error> {
        match raw {
            0 => Ok(Self::BLACK),
            1 => Ok(Self::WHITE),
            _ => Err("invalid color"),
        }
    }
}
impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match *self {
            Self::BLACK => "Black",
            Self::WHITE => "White",
        })
    }
}
impl Not for Color {
    type Output = Self;
    fn not(self) -> Self::Output {
        self.flip()
    }
}
impl FromStr for Color {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "black" | "b" => Ok(Self::BLACK),
            "white" | "w" => Ok(Self::WHITE),
            _ => Err("invalid color"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Color;

    #[test]
    fn raw_and_text_contract() {
        assert_eq!(Color::from_i8(0), Some(Color::Black));
        assert_eq!(Color::from_i8(2), None);
        assert_eq!(Color::Black.raw(), 0);
        assert_eq!(Color::Black.to_string(), "Black");
        assert_eq!("w".parse(), Ok(Color::White));
        assert_eq!(Color::iter().rev().collect::<Vec<_>>(), vec![Color::White, Color::Black]);
        assert_eq!(!Color::Black, Color::White);
    }
}
