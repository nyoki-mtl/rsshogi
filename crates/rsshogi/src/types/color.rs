//! 手番（先手と後手）の型定義
//!
//! [`Color`] は将棋の手番を表す列挙型。
//! 先手 (`BLACK` = 0) と後手 (`WHITE` = 1) の 2 値を持つ。
//!
//! [`flip()`](Color::flip) や `!` 演算子で手番を反転できる。

use std::fmt;
use std::ops::Not;
use std::str::FromStr;

/// 手番を表す型（先手 Black = 0、後手 White = 1）
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum Color {
    /// 先手（Black）
    Black = 0,
    /// 後手（White）
    White = 1,
}

const _: () = {
    assert!(core::mem::size_of::<Color>() == 1);
    assert!(core::mem::align_of::<Color>() == 1);
};

impl Color {
    // 関連定数
    /// 先手（Black）
    pub const BLACK: Self = Self::Black;
    /// 後手（White）
    pub const WHITE: Self = Self::White;
    /// ゼロ値（BLACK と同じ）
    pub const ZERO: Self = Self::Black;
    /// 色の総数
    pub const COUNT: usize = 2;

    /// 整数値（0/1）から `Color` を生成する。
    ///
    /// 不正値は `None` を返す。
    #[inline]
    #[must_use]
    pub const fn from_i8(value: i8) -> Option<Self> {
        match value {
            0 => Some(Self::Black),
            1 => Some(Self::White),
            _ => None,
        }
    }

    /// 手番を反転する
    #[inline]
    #[must_use]
    pub const fn flip(self) -> Self {
        match self {
            Self::Black => Self::White,
            Self::White => Self::Black,
        }
    }

    /// 内部値を取得する（主にテスト用）
    #[inline]
    #[must_use]
    pub const fn raw(self) -> i8 {
        self as u8 as i8
    }

    /// `Color` をインデックスに変換
    #[inline]
    #[must_use]
    pub const fn to_index(self) -> usize {
        self as u8 as usize
    }

    /// 全ての `Color` を順に返すイテレータを返す。
    ///
    /// # Examples
    ///
    /// ```
    /// use rsshogi::types::Color;
    ///
    /// let colors: Vec<_> = Color::iter().collect();
    /// assert_eq!(colors, vec![Color::Black, Color::White]);
    /// ```
    #[inline]
    #[must_use]
    pub fn iter() -> impl DoubleEndedIterator<Item = Self> + ExactSizeIterator {
        [Self::Black, Self::White].into_iter()
    }
}

impl Not for Color {
    type Output = Self;

    #[inline]
    fn not(self) -> Self::Output {
        self.flip()
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Black => write!(f, "Black"),
            Self::White => write!(f, "White"),
        }
    }
}

impl FromStr for Color {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "black" => Ok(Self::Black),
            "white" => Ok(Self::White),
            _ => Err(format!("invalid color string: {s}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_constants() {
        // 定数値検証（BLACK=0, WHITE=1, COUNT=2）
        assert_eq!(Color::BLACK.raw(), 0);
        assert_eq!(Color::WHITE.raw(), 1);
        assert_eq!(Color::ZERO.raw(), 0);
        assert_eq!(Color::COUNT, 2);
    }

    #[test]
    fn test_from_i8() {
        // const fn 変換のテスト
        const BLACK_CONST: Option<Color> = Color::from_i8(0);
        const WHITE_CONST: Option<Color> = Color::from_i8(1);
        const INVALID_CONST: Option<Color> = Color::from_i8(2);

        assert_eq!(BLACK_CONST, Some(Color::BLACK));
        assert_eq!(WHITE_CONST, Some(Color::WHITE));
        assert_eq!(INVALID_CONST, None);
    }

    #[test]
    fn test_color_flip() {
        // 反転操作のround-trip検証（c.flip().flip() == c）
        assert_eq!(Color::BLACK.flip(), Color::WHITE);
        assert_eq!(Color::WHITE.flip(), Color::BLACK);
        assert_eq!(Color::BLACK.flip().flip(), Color::BLACK);
        assert_eq!(Color::WHITE.flip().flip(), Color::WHITE);
    }

    #[test]
    fn test_color_not_operator() {
        // Not演算子のテスト
        assert_eq!(!Color::BLACK, Color::WHITE);
        assert_eq!(!Color::WHITE, Color::BLACK);
        assert_eq!(!!Color::BLACK, Color::BLACK);
        assert_eq!(!!Color::WHITE, Color::WHITE);
    }

    #[test]
    fn test_color_display() {
        // Display トレイトのテスト
        assert_eq!(Color::BLACK.to_string(), "Black");
        assert_eq!(Color::WHITE.to_string(), "White");
    }

    #[test]
    fn test_color_from_str() {
        // FromStr トレイトのテスト
        assert_eq!(Color::from_str("Black").unwrap(), Color::BLACK);
        assert_eq!(Color::from_str("White").unwrap(), Color::WHITE);
        assert_eq!(Color::from_str("black").unwrap(), Color::BLACK);
        assert_eq!(Color::from_str("white").unwrap(), Color::WHITE);
        assert_eq!(Color::from_str("BLACK").unwrap(), Color::BLACK);
        assert_eq!(Color::from_str("WHITE").unwrap(), Color::WHITE);

        assert!(Color::from_str("invalid").is_err());
        assert!(Color::from_str("").is_err());
    }

    #[test]
    fn test_color_display_from_str_roundtrip() {
        // Display/FromStr のround-trip検証
        let black_str = Color::BLACK.to_string();
        assert_eq!(Color::from_str(&black_str).unwrap(), Color::BLACK);

        let white_str = Color::WHITE.to_string();
        assert_eq!(Color::from_str(&white_str).unwrap(), Color::WHITE);
    }

    #[test]
    fn test_color_copy_clone() {
        // Copy/Clone トレイトのテスト
        let c = Color::BLACK;
        let c2 = c; // Copy
        assert_eq!(c, c2);

        #[allow(clippy::clone_on_copy)]
        let c3 = c.clone(); // Clone
        assert_eq!(c, c3);
    }
}
