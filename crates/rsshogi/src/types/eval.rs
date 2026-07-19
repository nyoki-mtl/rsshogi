//! 評価値の型定義
//!
//! [`Eval`] は局面の評価値を `i16` で表現する。
//!
//! - `Cp(value)` - 通常の評価値（centipawn 単位、`[-32000, 32000]`）
//! - `Special(value)` - 詰みスコアなどの特殊値（上記範囲外の `i16` 値）
//!
//! centipawn (cp) は評価値の標準的な単位で、歩 1 枚の価値を
//! 概ね 100cp として相対的に表現する。正の値は先手有利、
//! 負の値は後手有利を示す。

/// 評価値（i16）の表現。
///
/// - 通常値: `[-32000, 32000]` を `Cp` として扱う。
/// - それ以外は予約領域として `Special` に分類する。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Eval {
    Cp(i16),
    Special(i16),
}

impl Eval {
    pub const CP_MIN: i16 = -32000;
    pub const CP_MAX: i16 = 32000;

    /// 生の i16 値から `Eval` を生成する。
    #[must_use]
    pub const fn from_raw(raw: i16) -> Self {
        if raw >= Self::CP_MIN && raw <= Self::CP_MAX { Self::Cp(raw) } else { Self::Special(raw) }
    }

    /// cp 値から `Eval` を生成する（範囲外は `None`）。
    #[must_use]
    pub const fn from_cp(value: i32) -> Option<Self> {
        if value < Self::CP_MIN as i32 || value > Self::CP_MAX as i32 {
            return None;
        }
        Some(Self::Cp(value as i16))
    }

    /// i32 から `Eval` を生成する（i16 にクランプ）。
    #[must_use]
    pub const fn from_i32(value: i32) -> Self {
        let clamped = if value < i16::MIN as i32 {
            i16::MIN
        } else if value > i16::MAX as i32 {
            i16::MAX
        } else {
            value as i16
        };
        Self::from_raw(clamped)
    }

    /// i16 の生値を返す。
    #[must_use]
    pub const fn raw(self) -> i16 {
        match self {
            Self::Cp(value) | Self::Special(value) => value,
        }
    }

    /// i32 として取得する。
    #[must_use]
    pub const fn to_i32(self) -> i32 {
        self.raw() as i32
    }

    /// 特殊値かどうか。
    #[must_use]
    pub const fn is_special(self) -> bool {
        matches!(self, Self::Special(_))
    }
}
