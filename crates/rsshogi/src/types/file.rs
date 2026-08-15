use core::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct File(i8);

impl File {
    pub const FILE_1: Self = Self(0);
    pub const FILE_2: Self = Self(1);
    pub const FILE_3: Self = Self(2);
    pub const FILE_4: Self = Self(3);
    pub const FILE_5: Self = Self(4);
    pub const FILE_6: Self = Self(5);
    pub const FILE_7: Self = Self(6);
    pub const FILE_8: Self = Self(7);
    pub const FILE_9: Self = Self(8);
    pub const ZERO: Self = Self::FILE_1;
    pub const COUNT: usize = 9;
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
    pub const fn from_usi(value: char) -> Option<Self> {
        match value {
            '1'..='9' => Some(Self::new(value as i8 - '1' as i8)),
            _ => None,
        }
    }
    pub const fn to_usi(self) -> char {
        (b'1' + self.0 as u8) as char
    }
    pub fn iter() -> impl DoubleEndedIterator<Item = Self> + ExactSizeIterator {
        (0..Self::COUNT as i8).map(Self::new)
    }
}
impl TryFrom<i8> for File {
    type Error = &'static str;
    fn try_from(raw: i8) -> Result<Self, Self::Error> {
        let value = Self(raw);
        if value.is_valid() { Ok(value) } else { Err("invalid file") }
    }
}
impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0 + 1)
    }
}
