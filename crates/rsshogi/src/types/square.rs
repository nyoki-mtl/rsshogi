//! マス（Square）の定義
//!
//! 将棋盤の 81 マスを表現する。
//!
//! # 座標系
//!
//! ```text
//!        9   8   7   6   5   4   3   2   1
//!      +---+---+---+---+---+---+---+---+---+
//!   一 | 0 | 9 |18 |27 |36 |45 |54 |63 |72 | a (RANK_1)
//!      +---+---+---+---+---+---+---+---+---+
//!   二 | 1 |10 |19 |28 |37 |46 |55 |64 |73 | b (RANK_2)
//!      +---+---+---+---+---+---+---+---+---+
//!   三 | 2 |11 |20 |29 |38 |47 |56 |65 |74 | c (RANK_3)
//!      +---+---+---+---+---+---+---+---+---+
//!      ...
//! ```
//!
//! - 内部値は 0-80（81マス）
//! - `SQ_NONE` (81) は無効なマスを表す
//! - 筋（File）は右から左へ 1-9
//! - 段（Rank）は上から下へ 一-九
//!
//! # USI 表記
//!
//! USI プロトコルでは `1a`-`9i` の形式で表記する。
//!
//! ```
//! use rsshogi::types::{Square, File, Rank};
//!
//! let sq = Square::from_file_rank(File::FILE_7, Rank::RANK_7);
//! assert_eq!(sq.to_string(), "7g");
//! ```

use std::convert::TryFrom;
use std::fmt;
use std::ops::{BitOr, Index};
use std::str::FromStr;

use super::Color;
use super::file::File;
use super::rank::Rank;
use super::rank::is_promotable_rank as is_promotable_rank_fn;

// ビルド時に生成されたテーブルを読み込み
include!(concat!(env!("OUT_DIR"), "/square_tables.rs"));

// ANCHOR: square_struct
/// マス表現（0..81 + `SQ_NONE`）
///
/// 将棋盤の各マスを 0-80 の整数で表す。
/// `SQ_NONE` (81) は無効なマスを表す特別な値である。
///
/// # 座標変換
///
/// ```
/// use rsshogi::types::{Square, File, Rank};
///
/// // 筋と段からマスを作成
/// let sq = Square::from_file_rank(File::FILE_5, Rank::RANK_5);
///
/// // マスから筋と段を取得
/// assert_eq!(sq.file(), File::FILE_5);
/// assert_eq!(sq.rank(), Rank::RANK_5);
/// ```
///
/// # 定数
///
/// よく使うマスは定数として定義されている。
///
/// ```
/// use rsshogi::types::{SQ_77, SQ_51, Square};
///
/// // 7七（平手初期局面の先手の歩の前）
/// let sq = SQ_77;
///
/// // 5一（先手玉の初期位置）
/// let king_sq = SQ_51;
/// ```
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
#[repr(transparent)]
pub struct Square(i8);

const _: () = {
    assert!(core::mem::size_of::<Square>() == 1);
    assert!(core::mem::align_of::<Square>() == 1);
};
// ANCHOR_END: square_struct

// ANCHOR: square_direction_constants
// 定数定義
pub const SQ_LD: i8 = 10;
pub const SQ_LU: i8 = 8;
pub const SQ_RD: i8 = -8;
pub const SQ_RU: i8 = -10;
pub const SQ_L: i8 = 9;
pub const SQ_R: i8 = -9;
pub const SQ_U: i8 = -1;
pub const SQ_D: i8 = 1;
// ANCHOR_END: square_direction_constants
pub const SQ_ZERO: Square = Square::ZERO;
pub const SQ_NONE: Square = Square::NONE;

/// `Square` で引ける盤上 81 マス用テーブル。
///
/// `Square` の storage representation は変更せず、テーブル側に「盤上の
/// `Square` だけを受け付ける」境界を閉じ込めるための薄い wrapper である。
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SquareTable<T, const N: usize>([T; N]);

impl<T, const N: usize> SquareTable<T, N> {
    /// 配列から `SquareTable` を作る。
    #[inline]
    #[must_use]
    pub const fn new(entries: [T; N]) -> Self {
        Self(entries)
    }

    /// テーブルの要素数。
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        N
    }

    /// テーブルが空かどうか。
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        N == 0
    }

    /// 生配列 slice として参照する。
    #[inline]
    #[must_use]
    pub const fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// iterator を返す。
    #[inline]
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.0.iter()
    }
}

impl<T, const N: usize> Index<usize> for SquareTable<T, N> {
    type Output = T;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<T> Index<Square> for SquareTable<T, { Square::COUNT }> {
    type Output = T;

    #[inline]
    fn index(&self, sq: Square) -> &Self::Output {
        debug_assert!(sq.is_on_board(), "SquareTable requires on-board square: {sq:?}");
        // SAFETY: 呼び出し側が無効な Square 値を構築できるため、このラッパーは
        // debug assertion の背後に境界外アクセスを閉じ込め、81 要素の盤面
        // テーブルにのみ実装する。盤上の Square は 0..Square::COUNT にマップされる。
        unsafe { self.0.get_unchecked(sq.to_index()) }
    }
}

/// 盤面を左右反転させた升目を返す。
#[inline]
#[must_use]
pub const fn mirror_file(sq: Square) -> Square {
    let file = sq.raw() / 9;
    let rank = sq.raw() % 9;
    Square((8 - file) * 9 + rank)
}

/// 盤面を180度回転させた升目を返す。
#[inline]
#[must_use]
pub const fn flip(sq: Square) -> Square {
    Square((Square::COUNT as i8 - 1) - sq.raw())
}

/// 移動元/移動先が成りゾーンかを判定
#[inline]
#[must_use]
pub fn is_promotable_square(color: Color, from_or_to: Square) -> bool {
    if !from_or_to.is_on_board() {
        return false;
    }
    is_promotable_rank_fn(color, from_or_to.rank())
}

/// 移動元と移動先のどちらかが成りゾーンかを判定
#[inline]
#[must_use]
pub fn is_promotable_move(color: Color, from: Square, to: Square) -> bool {
    is_promotable_square(color, from) || is_promotable_square(color, to)
}
pub const SQ_99: Square = Square(80);
pub const SQ_98: Square = Square(79);
pub const SQ_97: Square = Square(78);
pub const SQ_96: Square = Square(77);
pub const SQ_95: Square = Square(76);
pub const SQ_94: Square = Square(75);
pub const SQ_93: Square = Square(74);
pub const SQ_92: Square = Square(73);
pub const SQ_91: Square = Square(72);
pub const SQ_89: Square = Square(71);
pub const SQ_88: Square = Square(70);
pub const SQ_87: Square = Square(69);
pub const SQ_86: Square = Square(68);
pub const SQ_85: Square = Square(67);
pub const SQ_84: Square = Square(66);
pub const SQ_83: Square = Square(65);
pub const SQ_82: Square = Square(64);
pub const SQ_81: Square = Square(63);
pub const SQ_79: Square = Square(62);
pub const SQ_78: Square = Square(61);
pub const SQ_77: Square = Square(60);
pub const SQ_76: Square = Square(59);
pub const SQ_75: Square = Square(58);
pub const SQ_74: Square = Square(57);
pub const SQ_73: Square = Square(56);
pub const SQ_72: Square = Square(55);
pub const SQ_71: Square = Square(54);
pub const SQ_69: Square = Square(53);
pub const SQ_68: Square = Square(52);
pub const SQ_67: Square = Square(51);
pub const SQ_66: Square = Square(50);
pub const SQ_65: Square = Square(49);
pub const SQ_64: Square = Square(48);
pub const SQ_63: Square = Square(47);
pub const SQ_62: Square = Square(46);
pub const SQ_61: Square = Square(45);
pub const SQ_59: Square = Square(44);
pub const SQ_58: Square = Square(43);
pub const SQ_57: Square = Square(42);
pub const SQ_56: Square = Square(41);
pub const SQ_55: Square = Square(40);
pub const SQ_54: Square = Square(39);
pub const SQ_53: Square = Square(38);
pub const SQ_52: Square = Square(37);
pub const SQ_51: Square = Square(36);
pub const SQ_49: Square = Square(35);
pub const SQ_48: Square = Square(34);
pub const SQ_47: Square = Square(33);
pub const SQ_46: Square = Square(32);
pub const SQ_45: Square = Square(31);
pub const SQ_44: Square = Square(30);
pub const SQ_43: Square = Square(29);
pub const SQ_42: Square = Square(28);
pub const SQ_41: Square = Square(27);
pub const SQ_39: Square = Square(26);
pub const SQ_38: Square = Square(25);
pub const SQ_37: Square = Square(24);
pub const SQ_36: Square = Square(23);
pub const SQ_35: Square = Square(22);
pub const SQ_34: Square = Square(21);
pub const SQ_33: Square = Square(20);
pub const SQ_32: Square = Square(19);
pub const SQ_31: Square = Square(18);
pub const SQ_29: Square = Square(17);
pub const SQ_28: Square = Square(16);
pub const SQ_27: Square = Square(15);
pub const SQ_26: Square = Square(14);
pub const SQ_25: Square = Square(13);
pub const SQ_24: Square = Square(12);
pub const SQ_23: Square = Square(11);
pub const SQ_22: Square = Square(10);
pub const SQ_21: Square = Square(9);
pub const SQ_19: Square = Square(8);
pub const SQ_18: Square = Square(7);
pub const SQ_17: Square = Square(6);
pub const SQ_16: Square = Square(5);
pub const SQ_15: Square = Square(4);
pub const SQ_14: Square = Square(3);
pub const SQ_13: Square = Square(2);
pub const SQ_12: Square = Square(1);
pub const SQ_11: Square = Square(0);
impl Square {
    // 関連定数
    /// 無効なマス
    pub const NONE: Self = Self(81);
    /// ゼロ値
    pub const ZERO: Self = Self(0);
    /// マスの総数
    pub const COUNT: usize = 81;
    /// マスの総数+1（`SQ_NONE` 含む）
    pub const COUNT_WITH_NONE: usize = 82;

    // 方角定数（将棋の座標系）
    // file * 9 + rank なので:
    // 上(U): rank-1 = -1
    // 下(D): rank+1 = +1
    // 右(R): file-1 = -9
    // 左(L): file+1 = +9
    // 注: file+1は9筋側（先手視点で左）に進む。

    /// 新しい`Square`
    #[inline]
    #[must_use]
    pub const fn new(value: i8) -> Self {
        Self(value)
    }

    /// 有効なマスかどうかを判定
    #[must_use]
    pub const fn is_valid(self) -> bool {
        0 <= self.0 && self.0 <= 81 // 0..81 + SQ_NONE(81)
    }

    /// 盤上のマス（0..=80）かどうか
    ///
    /// `SQ_NONE` は盤上ではない。
    #[inline]
    #[must_use]
    pub const fn is_on_board(self) -> bool {
        0 <= self.0 && self.0 < Self::COUNT as i8
    }

    /// `SQ_NONE` かどうか
    #[inline]
    #[must_use]
    pub const fn is_none(self) -> bool {
        self.0 == Self::NONE.0
    }

    /// マスから筋を取得
    #[inline]
    #[must_use]
    pub fn file(self) -> File {
        debug_assert!(self.is_on_board(), "Square::file() called for non-board square: {self:?}");
        SQUARE_TO_FILE[self.to_index()]
    }

    /// マスから段を取得
    #[inline]
    #[must_use]
    pub fn rank(self) -> Rank {
        debug_assert!(self.is_on_board(), "Square::rank() called for non-board square: {self:?}");
        SQUARE_TO_RANK[self.to_index()]
    }

    // ANCHOR: square_from_file_rank
    /// 筋と段からマスを生成
    #[inline]
    #[must_use]
    pub const fn from_file_rank(file: File, rank: Rank) -> Self {
        if file.is_valid() && rank.is_valid() {
            Self(file.raw() * 9 + rank.raw())
        } else {
            Self::NONE
        }
    }
    // ANCHOR_END: square_from_file_rank

    /// 内部値を取得する（主にテスト用）
    #[inline]
    #[must_use]
    pub const fn raw(self) -> i8 {
        self.0
    }

    /// `USI`形式から`Square`を生成
    #[must_use]
    pub fn from_usi(s: &str) -> Option<Self> {
        s.parse().ok()
    }

    /// インデックス値を取得
    #[inline]
    #[must_use]
    pub fn to_index(self) -> usize {
        // 盤上専用（0..=80）。
        debug_assert!(self.is_on_board(), "Square::to_index() requires on-board square: {self:?}");
        self.0 as usize
    }

    /// インデックス値を取得（`SQ_NONE` を含む: 0..=81）
    ///
    /// `Square::COUNT_WITH_NONE` 長のテーブルを引く用途などに使う。
    #[must_use]
    pub fn to_index_with_none(self) -> usize {
        debug_assert!(
            self.is_valid(),
            "Square::to_index_with_none() requires valid square: {self:?}"
        );
        debug_assert!(self.0 >= 0, "square index should be non-negative");
        self.0 as usize
    }

    /// 盤面を左右反転させた升目を返す。
    #[inline]
    #[must_use]
    pub const fn mirror_file(self) -> Self {
        mirror_file(self)
    }

    /// 盤面を180度回転させた升目を返す。
    #[inline]
    #[must_use]
    pub const fn flip(self) -> Self {
        flip(self)
    }

    /// 盤面インデックス（0-80）を取得
    #[inline]
    #[must_use]
    pub fn to_board_index(self) -> usize {
        debug_assert!(self.is_on_board(), "Square out of board: {self:?}");
        self.to_index()
    }

    /// インデックスから`Square`を生成
    #[inline]
    #[must_use]
    pub fn from_index(index: usize) -> Self {
        debug_assert!(index < Self::COUNT, "Square index out of range: {index}");
        let value = i8::try_from(index).expect("square index fits in i8");
        Self(value)
    }

    /// 盤上の全てのマス（81マス）を順に返すイテレータを返す。
    ///
    /// # Examples
    ///
    /// ```
    /// use rsshogi::types::Square;
    ///
    /// let squares: Vec<_> = Square::iter().collect();
    /// assert_eq!(squares.len(), 81);
    /// ```
    #[inline]
    #[must_use]
    pub fn iter() -> impl DoubleEndedIterator<Item = Self> + ExactSizeIterator {
        (0..Self::COUNT).map(|i| Self::new(i as i8))
    }
}

/// File | Rank でSquareを生成できるようにする
impl BitOr<Rank> for File {
    type Output = Square;

    fn bitor(self, rank: Rank) -> Square {
        Square::from_file_rank(self, rank)
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == Self::NONE {
            write!(f, "NONE")
        } else if self.is_valid() {
            // USI形式: 筋(1-9) + 段(a-i)
            let file_char = self.file().to_usi();
            let rank_char = self.rank().to_usi();
            write!(f, "{file_char}{rank_char}")
        } else {
            write!(f, "INVALID")
        }
    }
}

impl FromStr for Square {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "NONE" {
            return Ok(Self::NONE);
        }

        if s.len() != 2 {
            return Err(format!("invalid square string: {s}"));
        }

        let mut chars = s.chars();
        let file_char = chars.next().expect("validated length >= 2");
        let rank_char = chars.next().expect("validated length >= 2");

        let file = File::from_usi(file_char)
            .ok_or_else(|| format!("invalid file character: {file_char}"))?;
        let rank = Rank::from_usi(rank_char)
            .ok_or_else(|| format!("invalid rank character: {rank_char}"))?;

        Ok(file | rank)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_square_constants() {
        // 定数値検証
        assert_eq!(SQ_11.raw(), 0);
        assert_eq!(SQ_99.raw(), 80);
        assert_eq!(SQ_ZERO.raw(), 0);
        assert_eq!(SQ_NONE.raw(), 81);
        assert_eq!(Square::COUNT, 81);
        assert_eq!(Square::COUNT_WITH_NONE, 82);

        // 方角定数検証
        assert_eq!(SQ_U, -1);
        assert_eq!(SQ_D, 1);
        assert_eq!(SQ_R, -9);
        assert_eq!(SQ_L, 9);
        assert_eq!(SQ_RU, -10);
        assert_eq!(SQ_RD, -8);
        assert_eq!(SQ_LU, 8);
        assert_eq!(SQ_LD, 10);
    }

    #[test]
    fn test_square_is_ok() {
        // 境界値テスト
        assert!(!Square::new(-1).is_valid());
        assert!(Square::new(0).is_valid()); // SQ_11
        assert!(Square::new(40).is_valid()); // SQ_55
        assert!(Square::new(80).is_valid()); // SQ_99
        assert!(Square::new(81).is_valid()); // SQ_NONE
        assert!(!Square::new(82).is_valid());
    }

    #[test]
    fn test_square_specific_positions() {
        // 特定のマスの値を確認
        assert_eq!(SQ_55.raw(), 40); // 中央
        assert_eq!(SQ_51.raw(), 36); // 5一
        assert_eq!(SQ_59.raw(), 44); // 5九
        assert_eq!(SQ_15.raw(), 4); // 1五
        assert_eq!(SQ_95.raw(), 76); // 9五
    }

    #[test]
    fn test_square_coordinate_conversion() {
        // file()とrank()のテスト
        let sq = SQ_55; // 中央（file=4, rank=4）
        assert_eq!(sq.file(), File::FILE_5);
        assert_eq!(sq.rank(), Rank::RANK_5);

        let sq = SQ_11; // file=1, rank=1
        assert_eq!(sq.file(), File::FILE_1);
        assert_eq!(sq.rank(), Rank::RANK_1);

        let sq = SQ_99; // file=9, rank=9
        assert_eq!(sq.file(), File::FILE_9);
        assert_eq!(sq.rank(), Rank::RANK_9);
    }

    #[test]
    fn test_square_from_file_rank() {
        // from_file_rank()のテスト
        assert_eq!(Square::from_file_rank(File::FILE_5, Rank::RANK_5), SQ_55);
        assert_eq!(Square::from_file_rank(File::FILE_1, Rank::RANK_1), SQ_11);
        assert_eq!(Square::from_file_rank(File::FILE_9, Rank::RANK_9), SQ_99);

        // 無効な入力
        assert_eq!(Square::from_file_rank(File::new(-1), Rank::RANK_1), SQ_NONE);
        assert_eq!(Square::from_file_rank(File::FILE_1, Rank::new(-1)), SQ_NONE);
        assert_eq!(Square::from_file_rank(File::new(9), Rank::RANK_1), SQ_NONE);
    }

    #[test]
    fn test_square_bitor_operator() {
        // File | Rank 演算子のテスト
        assert_eq!(File::FILE_5 | Rank::RANK_5, SQ_55);
        assert_eq!(File::FILE_1 | Rank::RANK_1, SQ_11);
        assert_eq!(File::FILE_9 | Rank::RANK_9, SQ_99);

        // round-trip検証
        for i in 0..81 {
            let sq = Square::new(i);
            let reconstructed = sq.file() | sq.rank();
            assert_eq!(reconstructed, sq);
        }
    }

    #[test]
    fn test_square_display() {
        // USI形式の出力テスト
        assert_eq!(SQ_11.to_string(), "1a"); // 1一
        assert_eq!(SQ_55.to_string(), "5e"); // 5五
        assert_eq!(SQ_99.to_string(), "9i"); // 9九
        assert_eq!(SQ_77.to_string(), "7g"); // 7七
        assert_eq!(SQ_NONE.to_string(), "NONE");

        // 無効な値
        assert_eq!(Square::new(-1).to_string(), "INVALID");
        assert_eq!(Square::new(100).to_string(), "INVALID");
    }

    #[test]
    fn test_square_from_str() {
        // USI形式のパーステスト
        assert_eq!("1a".parse::<Square>().unwrap(), SQ_11);
        assert_eq!("5e".parse::<Square>().unwrap(), SQ_55);
        assert_eq!("9i".parse::<Square>().unwrap(), SQ_99);
        assert_eq!("7g".parse::<Square>().unwrap(), SQ_77);
        assert_eq!("NONE".parse::<Square>().unwrap(), SQ_NONE);

        // 不正な入力
        assert!("".parse::<Square>().is_err());
        assert!("0a".parse::<Square>().is_err());
        assert!("5j".parse::<Square>().is_err());
        assert!("aa".parse::<Square>().is_err());
        assert!("123".parse::<Square>().is_err());
    }

    #[test]
    fn test_square_string_roundtrip() {
        // Display/FromStr round-trip検証
        for i in 0..81 {
            let sq = Square::new(i);
            let s = sq.to_string();
            let parsed: Square = s.parse().unwrap();
            assert_eq!(parsed, sq);
        }

        // SQ_NONEも検証
        let sq = SQ_NONE;
        let s = sq.to_string();
        let parsed: Square = s.parse().unwrap();
        assert_eq!(parsed, sq);
    }

    #[test]
    fn test_square_inv_mirror_file_flip() {
        let sq = SQ_11;
        assert_eq!(sq.flip(), SQ_99);
        assert_eq!(sq.mirror_file(), SQ_91);
        assert_eq!(sq.flip(), SQ_99);

        let sq = SQ_55;
        assert_eq!(sq.flip(), SQ_55);
        assert_eq!(sq.mirror_file(), SQ_55);
        assert_eq!(sq.flip(), SQ_55);

        let sq = SQ_99;
        assert_eq!(sq.flip(), SQ_11);
        assert_eq!(sq.mirror_file(), SQ_19);
        assert_eq!(sq.flip(), SQ_11);
    }

    #[test]
    fn test_square_table_indexing() {
        let entries: [usize; Square::COUNT] = core::array::from_fn(|idx| idx);
        let table = SquareTable::new(entries);

        assert_eq!(table.len(), Square::COUNT);
        assert!(!table.is_empty());
        assert_eq!(table[SQ_11], 0);
        assert_eq!(table[SQ_55], SQ_55.to_index());
        assert_eq!(table[SQ_99], 80);
        assert_eq!(table[10], 10);
    }
}
