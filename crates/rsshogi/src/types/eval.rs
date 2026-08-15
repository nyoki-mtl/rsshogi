use core::{
    fmt,
    ops::{Add, AddAssign, Neg, Sub, SubAssign},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Eval {
    Cp(i16),
    Special(i16),
}

impl Eval {
    pub const CP_MIN: i16 = -32_000;
    pub const CP_MAX: i16 = 32_000;
    pub const ZERO: Self = Self::Cp(0);

    pub const fn from_raw(raw: i16) -> Self {
        if raw >= Self::CP_MIN && raw <= Self::CP_MAX { Self::Cp(raw) } else { Self::Special(raw) }
    }

    pub const fn from_cp(value: i32) -> Option<Self> {
        if value >= Self::CP_MIN as i32 && value <= Self::CP_MAX as i32 {
            Some(Self::Cp(value as i16))
        } else {
            None
        }
    }

    pub const fn from_i32(value: i32) -> Self {
        let clamped = if value > i16::MAX as i32 {
            i16::MAX
        } else if value < i16::MIN as i32 {
            i16::MIN
        } else {
            value as i16
        };
        Self::from_raw(clamped)
    }

    pub const fn raw(self) -> i16 {
        match self {
            Self::Cp(value) | Self::Special(value) => value,
        }
    }

    pub const fn to_i32(self) -> i32 {
        self.raw() as i32
    }

    pub const fn is_special(self) -> bool {
        matches!(self, Self::Special(_))
    }
}

impl Default for Eval {
    fn default() -> Self {
        Self::ZERO
    }
}
impl PartialOrd for Eval {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Eval {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.raw().cmp(&other.raw())
    }
}
impl From<i16> for Eval {
    fn from(value: i16) -> Self {
        Self::from_raw(value)
    }
}
impl From<Eval> for i16 {
    fn from(value: Eval) -> Self {
        value.raw()
    }
}
impl Add for Eval {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::from_i32(self.to_i32().saturating_add(rhs.to_i32()))
    }
}
impl Sub for Eval {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::from_i32(self.to_i32().saturating_sub(rhs.to_i32()))
    }
}
impl Neg for Eval {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self::from_i32(self.to_i32().saturating_neg())
    }
}
impl AddAssign for Eval {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}
impl SubAssign for Eval {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}
impl fmt::Display for Eval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw())
    }
}

#[cfg(test)]
mod tests {
    use super::Eval;

    #[test]
    fn cp_range_classification_is_inclusive() {
        assert_eq!(Eval::from_raw(Eval::CP_MIN), Eval::Cp(Eval::CP_MIN));
        assert_eq!(Eval::from_raw(Eval::CP_MAX), Eval::Cp(Eval::CP_MAX));
        assert!(Eval::from_raw(Eval::CP_MAX + 1).is_special());
        assert_eq!(Eval::from_cp(32_001), None);
        assert_eq!(Eval::from_i32(i32::MAX), Eval::Special(i16::MAX));
    }

    #[test]
    fn ordering_follows_the_numeric_value_across_variants() {
        assert!(Eval::Special(i16::MIN) < Eval::Cp(Eval::CP_MIN));
        assert!(Eval::Cp(Eval::CP_MAX) < Eval::Special(i16::MAX));
    }
}
