//! 駒種と駒の型定義
//!
//! # 概要
//!
//! - [`PieceType`] - 駒種（手番の区別なし）。歩(`PAWN`)～玉(`KING`) の 8 種と
//!   成駒 6 種（と金、成香、成桂、成銀、馬、龍）を定数として定義する。
//! - [`Piece`] - 駒種 + 手番の組み合わせ。`Piece::from_parts(Color::BLACK, PieceType::PAWN)`
//!   のように生成する。
//!
//! # 成りと成り戻し
//!
//! ```
//! use rsshogi::types::PieceType;
//!
//! let pawn = PieceType::PAWN;
//! assert!(pawn.is_promotable());
//!
//! let promoted = pawn.promote();
//! assert_eq!(promoted, PieceType::PRO_PAWN);
//! assert!(promoted.is_promoted());
//!
//! let demoted = promoted.demote();
//! assert_eq!(demoted, PieceType::PAWN);
//! ```

use super::Color;
use std::convert::TryFrom;
use std::fmt;
use std::str::FromStr;

// ANCHOR: piece_type_struct
/// 駒種を表す型（先手/後手の区別なし）
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct PieceType(i8);

const _: () = {
    assert!(core::mem::size_of::<PieceType>() == 1);
    assert!(core::mem::align_of::<PieceType>() == 1);
};
// ANCHOR_END: piece_type_struct

impl PieceType {
    // ANCHOR: piece_type_constants
    // 関連定数
    // 非成駒定数
    /// 無効な駒種
    pub const NONE: Self = Self(0);
    /// 歩
    pub const PAWN: Self = Self(1);
    /// 香車
    pub const LANCE: Self = Self(2);
    /// 桂馬
    pub const KNIGHT: Self = Self(3);
    /// 銀
    pub const SILVER: Self = Self(4);
    /// 角
    pub const BISHOP: Self = Self(5);
    /// 飛車
    pub const ROOK: Self = Self(6);
    /// 金
    pub const GOLD: Self = Self(7);
    /// 玉
    pub const KING: Self = Self(8);

    // 成駒定数
    /// と金（成歩）
    pub const PRO_PAWN: Self = Self(9);
    /// 成香
    pub const PRO_LANCE: Self = Self(10);
    /// 成桂
    pub const PRO_KNIGHT: Self = Self(11);
    /// 成銀
    pub const PRO_SILVER: Self = Self(12);
    /// 馬（成角）
    pub const HORSE: Self = Self(13);
    /// 龍（成飛）
    pub const DRAGON: Self = Self(14);
    /// 金相当の駒（GOLD_LIKE）
    pub const GOLD_LIKE: Self = Self(15);

    // メタ定数
    /// 駒種の総数
    pub const COUNT: usize = 16;
    /// 成り変換用オフセット
    pub const PIECE_TYPE_PROMOTE: i8 = 8;
    /// 持ち駒の起点（PAWN）
    pub const PIECE_HAND_ZERO: Self = Self(1);
    /// 持ち駒種類数
    pub const HAND_TABLE_SIZE: usize = 8;
    // ANCHOR_END: piece_type_constants

    /// const fn コンストラクタ
    #[inline]
    #[must_use]
    pub const fn new(value: i8) -> Self {
        Self(value)
    }

    // ANCHOR: piece_type_promote
    /// 成れる駒かどうかを判定
    #[must_use]
    pub const fn is_promotable(self) -> bool {
        self.0 >= Self::PAWN.0 && self.0 <= Self::ROOK.0
    }

    /// 成り駒かどうかを判定
    #[must_use]
    pub const fn is_promoted(self) -> bool {
        self.0 >= Self::PRO_PAWN.0
    }

    /// これ以上成れない駒かどうかを判定
    #[must_use]
    pub const fn is_unpromotable(self) -> bool {
        self.0 >= Self::GOLD.0
    }

    /// 成り駒に変換
    #[must_use]
    pub const fn promote(self) -> Self {
        if self.is_promotable() { Self(self.0 + Self::PIECE_TYPE_PROMOTE) } else { self }
    }

    /// 成り駒を元の駒に戻す（持ち駒変換用）
    #[must_use]
    pub const fn demote(self) -> Self {
        match self.0 {
            9..=14 => Self(self.0 - Self::PIECE_TYPE_PROMOTE), // 成駒を元に戻す
            _ => self,                                         // それ以外はそのまま
        }
    }
    // ANCHOR_END: piece_type_promote

    /// 有効な値かどうかを判定
    #[must_use]
    pub fn is_valid(self) -> bool {
        usize::try_from(self.0).map(|idx| idx < Self::COUNT).unwrap_or(false)
    }

    /// 内部値を取得する（主にテスト用）
    #[inline]
    #[must_use]
    pub const fn raw(self) -> i8 {
        self.0
    }

    /// `PieceType` をインデックス値に変換
    #[inline]
    #[must_use]
    #[allow(clippy::cast_sign_loss)]
    pub const fn to_index(self) -> usize {
        self.0 as usize
    }

    /// 小駒（香、桂、銀、金、と金、成香、成桂、成銀）かどうかを返す。
    ///
    /// Zobrist ハッシュの hand key 更新に使用される。
    #[inline]
    #[must_use]
    pub const fn is_minor(self) -> bool {
        // LANCE=2, KNIGHT=3, SILVER=4, GOLD=7, PRO_PAWN=9, PRO_LANCE=10, PRO_KNIGHT=11, PRO_SILVER=12
        const MINOR_MASK: u32 = (1 << 2)
            | (1 << 3)
            | (1 << 4)
            | (1 << 7)
            | (1 << 9)
            | (1 << 10)
            | (1 << 11)
            | (1 << 12);
        let idx = self.0 as u32;
        idx < 32 && (MINOR_MASK >> idx) & 1 != 0
    }

    /// 盤上に存在しうる駒種を順に返すイテレータを返す。
    ///
    /// `NONE` から `DRAGON` まで（`GOLD_LIKE` は除く）。
    ///
    /// # Examples
    ///
    /// ```
    /// use rsshogi::types::PieceType;
    ///
    /// let piece_types: Vec<_> = PieceType::iter().collect();
    /// assert_eq!(piece_types.len(), 15);
    /// assert_eq!(piece_types[0], PieceType::NONE);
    /// assert_eq!(piece_types[1], PieceType::PAWN);
    /// ```
    #[inline]
    pub fn iter() -> impl DoubleEndedIterator<Item = Self> + ExactSizeIterator {
        (0..15).map(Self::new)
    }

    /// 非成駒（歩〜玉）を順に返すイテレータを返す。
    #[inline]
    #[must_use]
    pub fn raw_pieces() -> impl DoubleEndedIterator<Item = Self> + ExactSizeIterator {
        [
            Self::PAWN,
            Self::LANCE,
            Self::KNIGHT,
            Self::SILVER,
            Self::BISHOP,
            Self::ROOK,
            Self::GOLD,
            Self::KING,
        ]
        .into_iter()
    }

    /// 持ち駒となりうる駒種（歩〜金）を順に返すイテレータを返す。
    #[inline]
    #[must_use]
    pub fn hand_pieces() -> impl DoubleEndedIterator<Item = Self> + ExactSizeIterator {
        [Self::PAWN, Self::LANCE, Self::KNIGHT, Self::SILVER, Self::BISHOP, Self::ROOK, Self::GOLD]
            .into_iter()
    }
}

impl Default for PieceType {
    fn default() -> Self {
        Self::NONE
    }
}

impl fmt::Display for PieceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            Self::PAWN => "P",
            Self::LANCE => "L",
            Self::KNIGHT => "N",
            Self::SILVER => "S",
            Self::BISHOP => "B",
            Self::ROOK => "R",
            Self::GOLD => "G",
            Self::KING => "K",
            Self::PRO_PAWN => "+P",
            Self::PRO_LANCE => "+L",
            Self::PRO_KNIGHT => "+N",
            Self::PRO_SILVER => "+S",
            Self::HORSE => "+B",
            Self::DRAGON => "+R",
            _ => "?",
        };
        write!(f, "{s}")
    }
}

impl FromStr for PieceType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "P" => Ok(Self::PAWN),
            "L" => Ok(Self::LANCE),
            "N" => Ok(Self::KNIGHT),
            "S" => Ok(Self::SILVER),
            "B" => Ok(Self::BISHOP),
            "R" => Ok(Self::ROOK),
            "G" => Ok(Self::GOLD),
            "K" => Ok(Self::KING),
            "+P" => Ok(Self::PRO_PAWN),
            "+L" => Ok(Self::PRO_LANCE),
            "+N" => Ok(Self::PRO_KNIGHT),
            "+S" => Ok(Self::PRO_SILVER),
            "+B" => Ok(Self::HORSE),
            "+R" => Ok(Self::DRAGON),
            _ => Err(format!("invalid piece type: {s}")),
        }
    }
}

// ANCHOR: piece_struct
/// 駒を表す型（先手/後手の区別あり）
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Hash)]
pub struct Piece(i8);

const _: () = {
    assert!(core::mem::size_of::<Piece>() == 1);
    assert!(core::mem::align_of::<Piece>() == 1);
};
// ANCHOR_END: piece_struct

impl Piece {
    // ANCHOR: piece_constants
    // 関連定数
    // 特殊値
    /// 無効な駒
    pub const NONE: Self = Self(0);

    // 先手駒定数
    /// 先手の歩
    pub const B_PAWN: Self = Self(1);
    /// 先手の香
    pub const B_LANCE: Self = Self(2);
    /// 先手の桂
    pub const B_KNIGHT: Self = Self(3);
    /// 先手の銀
    pub const B_SILVER: Self = Self(4);
    /// 先手の角
    pub const B_BISHOP: Self = Self(5);
    /// 先手の飛車
    pub const B_ROOK: Self = Self(6);
    /// 先手の金
    pub const B_GOLD: Self = Self(7);
    /// 先手の玉
    pub const B_KING: Self = Self(8);
    /// 先手のと金
    pub const B_PRO_PAWN: Self = Self(9);
    /// 先手の成香
    pub const B_PRO_LANCE: Self = Self(10);
    /// 先手の成桂
    pub const B_PRO_KNIGHT: Self = Self(11);
    /// 先手の成銀
    pub const B_PRO_SILVER: Self = Self(12);
    /// 先手の馬
    pub const B_HORSE: Self = Self(13);
    /// 先手の龍
    pub const B_DRAGON: Self = Self(14);
    /// 先手の金相当駒（GOLD_LIKE）
    pub const B_GOLD_LIKE: Self = Self(15);

    // 後手駒定数
    /// 後手の歩
    pub const W_PAWN: Self = Self(17);
    /// 後手の香
    pub const W_LANCE: Self = Self(18);
    /// 後手の桂
    pub const W_KNIGHT: Self = Self(19);
    /// 後手の銀
    pub const W_SILVER: Self = Self(20);
    /// 後手の角
    pub const W_BISHOP: Self = Self(21);
    /// 後手の飛車
    pub const W_ROOK: Self = Self(22);
    /// 後手の金
    pub const W_GOLD: Self = Self(23);
    /// 後手の玉
    pub const W_KING: Self = Self(24);
    /// 後手のと金
    pub const W_PRO_PAWN: Self = Self(25);
    /// 後手の成香
    pub const W_PRO_LANCE: Self = Self(26);
    /// 後手の成桂
    pub const W_PRO_KNIGHT: Self = Self(27);
    /// 後手の成銀
    pub const W_PRO_SILVER: Self = Self(28);
    /// 後手の馬
    pub const W_HORSE: Self = Self(29);
    /// 後手の龍
    pub const W_DRAGON: Self = Self(30);
    /// 後手の金相当駒（GOLD_LIKE）
    pub const W_GOLD_LIKE: Self = Self(31);

    // メタ定数
    /// 駒の総数
    pub const COUNT: usize = 32;
    /// 成りフラグ
    pub const PROMOTE_OFFSET: i8 = 8;
    /// 後手フラグ
    pub const COLOR_BIT: i8 = 16;
    /// 持ち駒種類数
    pub const HAND_TABLE_SIZE: usize = 8;
    /// 非成駒の終端
    pub const RAW_COUNT: usize = 8;
    // ANCHOR_END: piece_constants

    /// const fn コンストラクタ
    #[inline]
    #[must_use]
    pub const fn new(value: i8) -> Self {
        Self(value)
    }

    // ANCHOR: piece_accessors
    /// 手番と駒種から駒を作成
    #[must_use]
    pub const fn from_parts(color: Color, piece_type: PieceType) -> Self {
        let base = piece_type.0;
        if color.raw() == Color::WHITE.raw() { Self(base | Self::COLOR_BIT) } else { Self(base) }
    }

    /// 駒の手番を取得
    #[must_use]
    pub const fn color(self) -> Color {
        if (self.0 & Self::COLOR_BIT) != 0 { Color::WHITE } else { Color::BLACK }
    }

    /// 駒種を取得
    #[must_use]
    pub const fn piece_type(self) -> PieceType {
        PieceType::new(self.0 & 15)
    }
    /// 空（駒なし）かどうかを判定
    #[inline]
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    // ANCHOR_END: piece_accessors

    /// 成り駒に変換
    #[must_use]
    pub const fn promote(self) -> Self {
        let pt = self.piece_type();
        if pt.is_promotable() { Self(self.0 + Self::PROMOTE_OFFSET) } else { self }
    }

    /// 成り駒かどうかを判定
    #[must_use]
    pub const fn is_promoted(self) -> bool {
        self.piece_type().is_promoted()
    }

    /// これ以上成れない駒かどうかを判定
    #[must_use]
    pub const fn is_unpromotable(self) -> bool {
        self.piece_type().is_unpromotable()
    }

    /// 成りを外した駒種を取得（先後なし）
    #[must_use]
    pub const fn base_piece_type(self) -> PieceType {
        PieceType::new(self.0 & 7)
    }

    /// 成りを外した駒を取得（先後維持）
    #[must_use]
    pub const fn unpromoted_piece(self) -> Self {
        Self(self.0 & !Self::PROMOTE_OFFSET)
    }

    /// 遠方駒（香、角、飛、馬、龍のような飛び駒）かどうかを判定
    #[must_use]
    pub const fn is_long_range(self) -> bool {
        matches!(
            self.piece_type(),
            PieceType::LANCE
                | PieceType::BISHOP
                | PieceType::ROOK
                | PieceType::HORSE
                | PieceType::DRAGON
        )
    }

    /// 有効な値かどうかを判定
    #[must_use]
    pub fn is_valid(self) -> bool {
        if self.0 == Self::NONE.0 {
            return true;
        }
        let pt = self.piece_type();
        pt.is_valid()
            && pt.raw() != 0
            && self.0 >= 0
            && usize::try_from(self.0).map(|idx| idx < Self::COUNT).unwrap_or(false)
    }

    /// 内部値を取得する（主にテスト用）
    #[inline]
    #[must_use]
    pub const fn raw(self) -> i8 {
        self.0
    }

    /// インデックス値に変換
    #[inline]
    #[must_use]
    #[allow(clippy::cast_sign_loss)]
    pub const fn to_index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for Piece {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == Self::NONE {
            return write!(f, " ");
        }

        let piece_text = self.piece_type().to_string();
        let display = if self.color() == Color::WHITE {
            // 後手は小文字
            piece_text.to_lowercase()
        } else {
            // 先手は大文字
            piece_text
        };
        write!(f, "{display}")
    }
}

impl FromStr for Piece {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == " " {
            return Ok(Self::NONE);
        }

        // 成駒の場合、最初の'+'を確認
        let (is_promoted, base_str) =
            s.strip_prefix('+').map_or((false, s), |stripped| (true, stripped));

        // 大文字小文字で先手後手を判定
        let is_white = base_str.chars().next().is_some_and(char::is_lowercase);
        let color = if is_white { Color::WHITE } else { Color::BLACK };

        // 大文字に変換してPieceTypeとしてパース
        let upper = base_str.to_uppercase();
        let pt_str = if is_promoted { format!("+{upper}") } else { upper };
        let piece_type = PieceType::from_str(&pt_str)?;

        Ok(Self::from_parts(color, piece_type))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piece_type_constants() {
        // 非成駒
        assert_eq!(PieceType::NONE.raw(), 0);
        assert_eq!(PieceType::PAWN.raw(), 1);
        assert_eq!(PieceType::LANCE.raw(), 2);
        assert_eq!(PieceType::KNIGHT.raw(), 3);
        assert_eq!(PieceType::SILVER.raw(), 4);
        assert_eq!(PieceType::BISHOP.raw(), 5);
        assert_eq!(PieceType::ROOK.raw(), 6);
        assert_eq!(PieceType::GOLD.raw(), 7);
        assert_eq!(PieceType::KING.raw(), 8);

        // 成駒
        assert_eq!(PieceType::PRO_PAWN.raw(), 9);
        assert_eq!(PieceType::PRO_LANCE.raw(), 10);
        assert_eq!(PieceType::PRO_KNIGHT.raw(), 11);
        assert_eq!(PieceType::PRO_SILVER.raw(), 12);
        assert_eq!(PieceType::HORSE.raw(), 13);
        assert_eq!(PieceType::DRAGON.raw(), 14);

        // その他定数
        assert_eq!(PieceType::PIECE_TYPE_PROMOTE, 8);
        assert_eq!(PieceType::COUNT, 16);
        assert_eq!(PieceType::PIECE_HAND_ZERO.raw(), 1);
        assert_eq!(PieceType::HAND_TABLE_SIZE, 8);
    }

    #[test]
    fn test_piece_type_promote() {
        // 成れる駒
        assert_eq!(PieceType::PAWN.promote(), PieceType::PRO_PAWN);
        assert_eq!(PieceType::LANCE.promote(), PieceType::PRO_LANCE);
        assert_eq!(PieceType::KNIGHT.promote(), PieceType::PRO_KNIGHT);
        assert_eq!(PieceType::SILVER.promote(), PieceType::PRO_SILVER);
        assert_eq!(PieceType::BISHOP.promote(), PieceType::HORSE);
        assert_eq!(PieceType::ROOK.promote(), PieceType::DRAGON);

        // 成れない駒
        assert_eq!(PieceType::GOLD.promote(), PieceType::GOLD);
        assert_eq!(PieceType::KING.promote(), PieceType::KING);

        // すでに成っている駒
        assert_eq!(PieceType::PRO_PAWN.promote(), PieceType::PRO_PAWN);
        assert_eq!(PieceType::HORSE.promote(), PieceType::HORSE);
    }

    #[test]
    fn test_piece_type_is_promotable() {
        assert!(PieceType::PAWN.is_promotable());
        assert!(PieceType::LANCE.is_promotable());
        assert!(PieceType::KNIGHT.is_promotable());
        assert!(PieceType::SILVER.is_promotable());
        assert!(PieceType::BISHOP.is_promotable());
        assert!(PieceType::ROOK.is_promotable());

        assert!(!PieceType::GOLD.is_promotable());
        assert!(!PieceType::KING.is_promotable());
        assert!(!PieceType::PRO_PAWN.is_promotable());
        assert!(!PieceType::HORSE.is_promotable());
    }

    #[test]
    fn test_piece_type_is_ok() {
        // 有効な値
        for i in 0..16 {
            assert!(PieceType(i).is_valid());
        }

        // 無効な値
        assert!(!PieceType(-1).is_valid());
        assert!(!PieceType(16).is_valid());
        assert!(!PieceType(100).is_valid());
    }

    #[test]
    fn test_piece_type_display_to_string() {
        let cases = [
            (PieceType::PAWN, "P"),
            (PieceType::LANCE, "L"),
            (PieceType::KNIGHT, "N"),
            (PieceType::SILVER, "S"),
            (PieceType::BISHOP, "B"),
            (PieceType::ROOK, "R"),
            (PieceType::GOLD, "G"),
            (PieceType::KING, "K"),
            (PieceType::PRO_PAWN, "+P"),
            (PieceType::PRO_LANCE, "+L"),
            (PieceType::PRO_KNIGHT, "+N"),
            (PieceType::PRO_SILVER, "+S"),
            (PieceType::HORSE, "+B"),
            (PieceType::DRAGON, "+R"),
        ];

        for (piece_type, expected) in cases {
            assert_eq!(piece_type.to_string(), expected);
        }
    }

    #[test]
    fn test_piece_type_display_round_trip() {
        let cases = [
            PieceType::PAWN,
            PieceType::LANCE,
            PieceType::KNIGHT,
            PieceType::SILVER,
            PieceType::BISHOP,
            PieceType::ROOK,
            PieceType::GOLD,
            PieceType::KING,
            PieceType::PRO_PAWN,
            PieceType::PRO_LANCE,
            PieceType::PRO_KNIGHT,
            PieceType::PRO_SILVER,
            PieceType::HORSE,
            PieceType::DRAGON,
        ];

        for pt in cases {
            let s = pt.to_string();
            let parsed = PieceType::from_str(&s).unwrap();
            assert_eq!(pt, parsed);
        }
    }

    #[test]
    fn test_piece_type_from_str_invalid() {
        for invalid in ["X", "", "++P"] {
            assert!(PieceType::from_str(invalid).is_err());
        }
    }

    #[test]
    fn test_piece_constants_black() {
        assert_eq!(Piece::B_PAWN.raw(), 1);
        assert_eq!(Piece::B_LANCE.raw(), 2);
        assert_eq!(Piece::B_KNIGHT.raw(), 3);
        assert_eq!(Piece::B_SILVER.raw(), 4);
        assert_eq!(Piece::B_BISHOP.raw(), 5);
        assert_eq!(Piece::B_ROOK.raw(), 6);
        assert_eq!(Piece::B_GOLD.raw(), 7);
        assert_eq!(Piece::B_KING.raw(), 8);
        assert_eq!(Piece::B_PRO_PAWN.raw(), 9);
        assert_eq!(Piece::B_PRO_LANCE.raw(), 10);
        assert_eq!(Piece::B_PRO_KNIGHT.raw(), 11);
        assert_eq!(Piece::B_PRO_SILVER.raw(), 12);
        assert_eq!(Piece::B_HORSE.raw(), 13);
        assert_eq!(Piece::B_DRAGON.raw(), 14);
    }

    #[test]
    fn test_piece_constants_white() {
        assert_eq!(Piece::W_PAWN.raw(), 17);
        assert_eq!(Piece::W_LANCE.raw(), 18);
        assert_eq!(Piece::W_KNIGHT.raw(), 19);
        assert_eq!(Piece::W_SILVER.raw(), 20);
        assert_eq!(Piece::W_BISHOP.raw(), 21);
        assert_eq!(Piece::W_ROOK.raw(), 22);
        assert_eq!(Piece::W_GOLD.raw(), 23);
        assert_eq!(Piece::W_KING.raw(), 24);
        assert_eq!(Piece::W_PRO_PAWN.raw(), 25);
        assert_eq!(Piece::W_PRO_LANCE.raw(), 26);
        assert_eq!(Piece::W_PRO_KNIGHT.raw(), 27);
        assert_eq!(Piece::W_PRO_SILVER.raw(), 28);
        assert_eq!(Piece::W_HORSE.raw(), 29);
        assert_eq!(Piece::W_DRAGON.raw(), 30);
    }

    #[test]
    fn test_piece_constant_flags() {
        assert_eq!(Piece::NONE.raw(), 0);
        assert_eq!(Piece::COLOR_BIT, 16);
        assert_eq!(Piece::PROMOTE_OFFSET, 8);
        assert_eq!(Piece::COUNT, 32);
    }

    #[test]
    fn test_piece_make_and_decompose() {
        // make -> color/piece_type のラウンドトリップ
        for color in [Color::BLACK, Color::WHITE] {
            for pt in [
                PieceType::PAWN,
                PieceType::LANCE,
                PieceType::KNIGHT,
                PieceType::SILVER,
                PieceType::BISHOP,
                PieceType::ROOK,
                PieceType::GOLD,
                PieceType::KING,
                PieceType::PRO_PAWN,
                PieceType::HORSE,
            ] {
                let piece = Piece::from_parts(color, pt);
                assert_eq!(piece.color(), color);
                assert_eq!(piece.piece_type(), pt);
            }
        }

        // 定数との一致確認
        assert_eq!(Piece::from_parts(Color::BLACK, PieceType::PAWN), Piece::B_PAWN);
        assert_eq!(Piece::from_parts(Color::WHITE, PieceType::PAWN), Piece::W_PAWN);
        assert_eq!(Piece::from_parts(Color::BLACK, PieceType::KING), Piece::B_KING);
        assert_eq!(Piece::from_parts(Color::WHITE, PieceType::KING), Piece::W_KING);
    }

    #[test]
    fn test_piece_promote() {
        // 成れる駒
        assert_eq!(Piece::B_PAWN.promote(), Piece::B_PRO_PAWN);
        assert_eq!(Piece::W_PAWN.promote(), Piece::W_PRO_PAWN);
        assert_eq!(Piece::B_BISHOP.promote(), Piece::B_HORSE);
        assert_eq!(Piece::W_ROOK.promote(), Piece::W_DRAGON);

        // 成れない駒
        assert_eq!(Piece::B_GOLD.promote(), Piece::B_GOLD);
        assert_eq!(Piece::W_KING.promote(), Piece::W_KING);

        // すでに成っている駒
        assert_eq!(Piece::B_PRO_PAWN.promote(), Piece::B_PRO_PAWN);
        assert_eq!(Piece::W_HORSE.promote(), Piece::W_HORSE);
    }

    #[test]
    fn test_piece_is_ok() {
        // 有効な駒
        assert!(Piece::NONE.is_valid());
        assert!(Piece::B_PAWN.is_valid());
        assert!(Piece::W_KING.is_valid());
        assert!(Piece::B_DRAGON.is_valid());
        assert!(Piece::W_DRAGON.is_valid());

        // 無効な駒
        assert!(!Piece(-1).is_valid());
        assert!(!Piece(32).is_valid()); // PIECE_NBと等しい
        assert!(!Piece(33).is_valid());
        assert!(!Piece(100).is_valid());

        // piece_type部分が0の場合（NO_PIECE以外）
        assert!(!Piece(16).is_valid()); // COLOR_BIT のみで駒種なし
    }

    #[test]
    fn test_piece_display_to_string() {
        let cases = [
            (Piece::NONE, " "),
            (Piece::B_PAWN, "P"),
            (Piece::B_LANCE, "L"),
            (Piece::B_KNIGHT, "N"),
            (Piece::B_SILVER, "S"),
            (Piece::B_BISHOP, "B"),
            (Piece::B_ROOK, "R"),
            (Piece::B_GOLD, "G"),
            (Piece::B_KING, "K"),
            (Piece::B_PRO_PAWN, "+P"),
            (Piece::B_HORSE, "+B"),
            (Piece::B_DRAGON, "+R"),
            (Piece::W_PAWN, "p"),
            (Piece::W_LANCE, "l"),
            (Piece::W_KNIGHT, "n"),
            (Piece::W_SILVER, "s"),
            (Piece::W_BISHOP, "b"),
            (Piece::W_ROOK, "r"),
            (Piece::W_GOLD, "g"),
            (Piece::W_KING, "k"),
            (Piece::W_PRO_PAWN, "+p"),
            (Piece::W_HORSE, "+b"),
            (Piece::W_DRAGON, "+r"),
        ];

        for (piece, expected) in cases {
            assert_eq!(piece.to_string(), expected);
        }
    }

    #[test]
    fn test_piece_display_round_trip() {
        let cases = [
            Piece::NONE,
            Piece::B_PAWN,
            Piece::W_PAWN,
            Piece::B_KING,
            Piece::W_KING,
            Piece::B_DRAGON,
            Piece::W_DRAGON,
        ];

        for piece in cases {
            let s = piece.to_string();
            let parsed = Piece::from_str(&s).unwrap();
            assert_eq!(piece, parsed);
        }
    }

    #[test]
    fn test_piece_display_from_str_invalid() {
        for invalid in ["X", ""] {
            assert!(Piece::from_str(invalid).is_err());
        }
    }
}
