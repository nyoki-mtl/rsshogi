//! 持ち駒の型定義
//!
//! [`Hand`] は各プレイヤーの持ち駒を `u32` のビットフィールドで管理する。
//! 7 種の駒（歩、香、桂、銀、角、飛、金）の枚数を 1 ワードにパックし、
//! 加減算やビット演算で高速に操作できる。
//!
//! [`HandPiece`] は持ち駒の駒種インデックス（0～6）を表す。
//!
//! # ビットレイアウト
//!
//! ```text
//! bit  0.. 4: 歩 (5bit, 最大 18 枚)
//! bit  8..10: 香 (3bit, 最大 4 枚)
//! bit 12..14: 桂 (3bit, 最大 4 枚)
//! bit 16..18: 銀 (3bit, 最大 4 枚)
//! bit 20..21: 角 (2bit, 最大 2 枚)
//! bit 24..25: 飛 (2bit, 最大 2 枚)
//! bit 28..30: 金 (3bit, 最大 4 枚)
//! ```

use super::PieceType;

/// 持ち駒の駒種
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct HandPiece(i8);

const _: () = {
    assert!(core::mem::size_of::<HandPiece>() == 1);
    assert!(core::mem::align_of::<HandPiece>() == 1);
};

/// 全駒種の有効ビットを結合したマスク。持ち駒の存在判定やビット操作に使用。
pub const HAND_BIT_MASK: u32 =
    0x1f | (0x7 << 8) | (0x7 << 12) | (0x7 << 16) | (0x3 << 20) | (0x3 << 24) | (0x7 << 28);
/// 各駒種フィールドの上位 1 ビット（繰り下がり検出用）。
/// 持ち駒の優劣比較で使用する。
pub const HAND_BORROW_MASK: u32 = (HAND_BIT_MASK << 1) & !HAND_BIT_MASK;
impl HandPiece {
    // 持ち駒定数（歩から金まで）
    pub const PAWN: Self = Self(0);
    pub const LANCE: Self = Self(1);
    pub const KNIGHT: Self = Self(2);
    pub const SILVER: Self = Self(3);
    pub const BISHOP: Self = Self(4);
    pub const ROOK: Self = Self(5);
    pub const GOLD: Self = Self(6);
    pub const COUNT: usize = 7;

    /// 内部値から `HandPiece` を生成する。
    ///
    /// 値の妥当性は呼び出し側が保証する（互換性とテスト用途を想定）。
    #[inline]
    #[must_use]
    pub const fn new(raw: i8) -> Self {
        Self(raw)
    }

    /// `PieceType` から変換
    #[must_use]
    pub const fn from_piece_type(pt: PieceType) -> Option<Self> {
        match pt {
            PieceType::PAWN => Some(Self::PAWN),
            PieceType::LANCE => Some(Self::LANCE),
            PieceType::KNIGHT => Some(Self::KNIGHT),
            PieceType::SILVER => Some(Self::SILVER),
            PieceType::BISHOP => Some(Self::BISHOP),
            PieceType::ROOK => Some(Self::ROOK),
            PieceType::GOLD => Some(Self::GOLD),
            _ => None,
        }
    }

    /// `PieceType` へ変換
    #[must_use]
    pub const fn to_piece_type(self) -> PieceType {
        match self {
            Self::PAWN => PieceType::PAWN,
            Self::LANCE => PieceType::LANCE,
            Self::KNIGHT => PieceType::KNIGHT,
            Self::SILVER => PieceType::SILVER,
            Self::BISHOP => PieceType::BISHOP,
            Self::ROOK => PieceType::ROOK,
            Self::GOLD => PieceType::GOLD,
            _ => PieceType::NONE,
        }
    }

    /// 有効な値かどうか判定
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.raw() >= 0 && self.raw() <= Self::GOLD.raw()
    }

    /// 内部値を取得する（主にテスト用）
    #[inline]
    #[must_use]
    pub const fn raw(self) -> i8 {
        self.0
    }

    /// 全ての持ち駒種（PAWN, LANCE, KNIGHT, SILVER, BISHOP, ROOK, GOLD）を順番にイテレート
    ///
    /// # Examples
    ///
    /// ```
    /// use rsshogi::types::HandPiece;
    ///
    /// let pieces: Vec<_> = HandPiece::iter().collect();
    /// assert_eq!(pieces.len(), 7);
    /// ```
    #[inline]
    #[must_use]
    pub fn iter() -> impl DoubleEndedIterator<Item = Self> + ExactSizeIterator {
        (0..Self::COUNT).map(|i| Self::new(i as i8))
    }
}

/// 持ち駒
/// ビットフィールド構成:
/// - 歩: bit0-4 (5bit, 最大31枚)
/// - 香: bit8-10 (3bit, 最大7枚)
/// - 桂: bit12-14 (3bit, 最大7枚)
/// - 銀: bit16-18 (3bit, 最大7枚)
/// - 角: bit20-21 (2bit, 最大3枚)
/// - 飛: bit24-25 (2bit, 最大3枚)
/// - 金: bit28-30 (3bit, 最大7枚)
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct Hand(u32);

const _: () = {
    assert!(core::mem::size_of::<Hand>() == 4);
    assert!(core::mem::align_of::<Hand>() == 4);
};

impl Hand {
    pub const ZERO: Self = Self(0);

    /// 生のビット表現から `Hand` を生成する（互換性と低レベル用途）。
    #[inline]
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// 内部のビット表現を取得する（低レベル用途とテスト用）。
    #[inline]
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    // ビット配置定数（開始ビット位置）
    const PIECE_BITS: [u32; 7] = [
        0,  // 歩: bit 0から
        8,  // 香: bit 8から
        12, // 桂: bit 12から
        16, // 銀: bit 16から
        20, // 角: bit 20から
        24, // 飛: bit 24から
        28, // 金: bit 28から
    ];

    // ビットマスク（各駒種のビット幅）
    const PIECE_BIT_MASK: [u32; 7] = [
        0x1f, // 歩: 5bit
        0x07, // 香: 3bit
        0x07, // 桂: 3bit
        0x07, // 銀: 3bit
        0x03, // 角: 2bit
        0x03, // 飛: 2bit
        0x07, // 金: 3bit
    ];

    // 単体マスク（1枚分の値）
    const PIECE_BIT_ONE: [u32; 7] = [
        1 << 0,  // 歩
        1 << 8,  // 香
        1 << 12, // 桂
        1 << 16, // 銀
        1 << 20, // 角
        1 << 24, // 飛
        1 << 28, // 金
    ];

    // 優劣判定用マスク

    /// 指定した駒種の枚数を取得
    #[inline]
    #[must_use]
    pub const fn count_of(hand: Self, hp: HandPiece) -> u32 {
        if let Some(idx) = Self::index_for(hp) {
            let shift = Self::PIECE_BITS[idx];
            let mask = Self::PIECE_BIT_MASK[idx];
            (hand.0 >> shift) & mask
        } else {
            0
        }
    }

    /// 指定した駒種が存在するか判定
    #[inline]
    #[must_use]
    pub const fn has(hand: Self, hp: HandPiece) -> bool {
        Self::count_of(hand, hp) > 0
    }

    /// 指定した駒種を1枚追加
    #[inline]
    #[must_use]
    pub const fn add_one(hand: Self, hp: HandPiece) -> Self {
        if let Some(idx) = Self::index_for(hp) {
            Self(hand.0 + Self::PIECE_BIT_ONE[idx])
        } else {
            hand
        }
    }

    /// 指定した駒種を1枚減らす。
    #[inline]
    #[must_use]
    pub const fn sub_one(hand: Self, hp: HandPiece) -> Self {
        if let Some(idx) = Self::index_for(hp) {
            Self(hand.0 - Self::PIECE_BIT_ONE[idx])
        } else {
            hand
        }
    }

    /// h1がh2以上の持ち駒を持っているか判定
    /// `HAND_BORROW_MASK` を使った高速判定
    #[must_use]
    pub const fn is_equal_or_superior(h1: Self, h2: Self) -> bool {
        (h1.0.wrapping_sub(h2.0) & HAND_BORROW_MASK) == 0
    }

    /// ビットフィールドにオーバーフロー（borrow）が発生しているか判定
    ///
    /// # Examples
    ///
    /// ```
    /// use rsshogi::types::{Hand, HandPiece};
    ///
    /// let mut hand = Hand::ZERO;
    /// for _ in 0..31 {
    ///     hand = Hand::add_one(hand, HandPiece::PAWN);
    /// }
    /// assert!(!hand.has_overflow());
    ///
    /// hand = Hand::add_one(hand, HandPiece::PAWN);
    /// assert!(hand.has_overflow());
    /// ```
    #[must_use]
    #[inline]
    pub const fn has_overflow(self) -> bool {
        (self.0 & HAND_BORROW_MASK) != 0
    }

    /// 指定した駒種を `n` 枚追加し、ビットフィールド幅を超えなければ新しい `Hand` を返す。
    ///
    /// フィールド幅（歩5bit、香3bit...）を超える場合は `None` を返す。
    ///
    /// # Examples
    ///
    /// ```
    /// use rsshogi::types::{Hand, HandPiece};
    ///
    /// let hand = Hand::ZERO.checked_add(HandPiece::PAWN, 18).unwrap();
    /// assert_eq!(hand.count(HandPiece::PAWN), 18);
    ///
    /// assert!(hand.checked_add(HandPiece::PAWN, 32).is_none());
    /// ```
    #[must_use]
    #[inline]
    pub fn checked_add(self, hp: HandPiece, n: u32) -> Option<Self> {
        let hand = if let Some(idx) = Self::index_for(hp) {
            let shift = Self::PIECE_BITS[idx];
            let mask = Self::PIECE_BIT_MASK[idx];
            let current = (self.0 >> shift) & mask;
            let next = current.checked_add(n)?;
            if next > mask {
                return None;
            }
            let cleared = self.0 & !(mask << shift);
            Self(cleared | (next << shift))
        } else {
            self
        };

        if hand.has_overflow() { None } else { Some(hand) }
    }

    // 簡便なメソッド追加
    /// 指定した駒種の枚数を取得
    #[inline]
    #[must_use]
    pub const fn count(&self, hp: HandPiece) -> u32 {
        Self::count_of(*self, hp)
    }

    /// 指定した駒種を指定枚数追加
    #[inline]
    pub fn add(&mut self, hp: HandPiece, n: u32) {
        if let Some(idx) = Self::index_for(hp) {
            self.0 += Self::PIECE_BIT_ONE[idx] * n;
        }
    }

    /// 指定した駒種を指定枚数減らす
    #[inline]
    pub fn sub(&mut self, hp: HandPiece, n: u32) {
        if let Some(idx) = Self::index_for(hp) {
            self.0 -= Self::PIECE_BIT_ONE[idx] * n;
        }
    }

    /// 歩の枚数を取得
    #[inline]
    #[must_use]
    pub const fn pawn_count(&self) -> u32 {
        self.count(HandPiece::PAWN)
    }

    /// 持ち駒種に対応するインデックス
    const fn index_for(hp: HandPiece) -> Option<usize> {
        match hp {
            HandPiece::PAWN => Some(0),
            HandPiece::LANCE => Some(1),
            HandPiece::KNIGHT => Some(2),
            HandPiece::SILVER => Some(3),
            HandPiece::BISHOP => Some(4),
            HandPiece::ROOK => Some(5),
            HandPiece::GOLD => Some(6),
            _ => None,
        }
    }
}

impl Default for Hand {
    fn default() -> Self {
        Self::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hand_piece_constants() {
        assert_eq!(HandPiece::PAWN.raw(), 0);
        assert_eq!(HandPiece::LANCE.raw(), 1);
        assert_eq!(HandPiece::KNIGHT.raw(), 2);
        assert_eq!(HandPiece::SILVER.raw(), 3);
        assert_eq!(HandPiece::BISHOP.raw(), 4);
        assert_eq!(HandPiece::ROOK.raw(), 5);
        assert_eq!(HandPiece::GOLD.raw(), 6);
        assert_eq!(HandPiece::COUNT, 7);
    }

    #[test]
    fn test_hand_piece_conversion() {
        // from_piece_type -> to_piece_type の往復変換確認
        let test_cases = [
            (PieceType::PAWN, HandPiece::PAWN),
            (PieceType::LANCE, HandPiece::LANCE),
            (PieceType::KNIGHT, HandPiece::KNIGHT),
            (PieceType::SILVER, HandPiece::SILVER),
            (PieceType::BISHOP, HandPiece::BISHOP),
            (PieceType::ROOK, HandPiece::ROOK),
            (PieceType::GOLD, HandPiece::GOLD),
        ];

        for (pt, hp) in test_cases {
            assert_eq!(HandPiece::from_piece_type(pt), Some(hp));
            assert_eq!(hp.to_piece_type(), pt);
        }

        // 不正な駒種
        assert_eq!(HandPiece::from_piece_type(PieceType::KING), None);
        assert_eq!(HandPiece::from_piece_type(PieceType::PRO_PAWN), None);
        assert_eq!(HandPiece::from_piece_type(PieceType::NONE), None);
    }

    #[test]
    fn test_hand_piece_is_ok() {
        // 有効な値
        for i in 0..7 {
            assert!(HandPiece::new(i).is_valid());
        }

        // 無効な値
        assert!(!HandPiece::new(-1).is_valid());
        assert!(!HandPiece::new(7).is_valid());
        assert!(!HandPiece::new(100).is_valid());
    }

    #[test]
    fn test_hand_bit_packing() {
        // 空の持ち駒
        assert_eq!(Hand::ZERO.bits(), 0);

        // 各駒種を1枚ずつ追加
        let mut hand = Hand::ZERO;

        // 歩を1枚追加
        hand = Hand::add_one(hand, HandPiece::PAWN);
        assert_eq!(Hand::count_of(hand, HandPiece::PAWN), 1);
        assert_eq!(hand.bits(), 1);

        // 香を1枚追加
        hand = Hand::add_one(hand, HandPiece::LANCE);
        assert_eq!(Hand::count_of(hand, HandPiece::LANCE), 1);
        assert_eq!(hand.bits(), 1 | (1 << 8));

        // 桂を1枚追加
        hand = Hand::add_one(hand, HandPiece::KNIGHT);
        assert_eq!(Hand::count_of(hand, HandPiece::KNIGHT), 1);
        assert_eq!(hand.bits(), 1 | (1 << 8) | (1 << 12));
    }

    #[test]
    fn test_hand_add_sub() {
        let mut hand = Hand::ZERO;

        // 歩を5枚追加
        for _ in 0..5 {
            hand = Hand::add_one(hand, HandPiece::PAWN);
        }
        assert_eq!(Hand::count_of(hand, HandPiece::PAWN), 5);

        // 歩を2枚減らす
        hand.sub(HandPiece::PAWN, 2);
        assert_eq!(Hand::count_of(hand, HandPiece::PAWN), 3);

        // 角を2枚追加
        hand = Hand::add_one(hand, HandPiece::BISHOP);
        hand = Hand::add_one(hand, HandPiece::BISHOP);
        assert_eq!(Hand::count_of(hand, HandPiece::BISHOP), 2);

        // 飛車を1枚追加
        hand = Hand::add_one(hand, HandPiece::ROOK);
        assert_eq!(Hand::count_of(hand, HandPiece::ROOK), 1);
    }

    #[test]
    fn test_hand_exists() {
        let mut hand = Hand::ZERO;

        // 最初は何も持っていない
        assert!(!Hand::has(hand, HandPiece::PAWN));
        assert!(!Hand::has(hand, HandPiece::GOLD));

        // 金を1枚追加
        hand = Hand::add_one(hand, HandPiece::GOLD);
        assert!(Hand::has(hand, HandPiece::GOLD));
        assert!(!Hand::has(hand, HandPiece::PAWN));
    }

    #[test]
    fn test_hand_is_equal_or_superior() {
        let h1 = Hand::ZERO;
        let mut h2 = Hand::ZERO;

        // 同じ持ち駒
        assert!(Hand::is_equal_or_superior(h1, h2));

        // h1の方が少ない
        h2 = Hand::add_one(h2, HandPiece::PAWN);
        assert!(!Hand::is_equal_or_superior(h1, h2));

        // h1の方が多い
        let mut h3 = Hand::add_one(h1, HandPiece::PAWN);
        h3 = Hand::add_one(h3, HandPiece::PAWN);
        assert!(Hand::is_equal_or_superior(h3, h2));

        // 複数種類の駒での比較
        let mut h4 = Hand::ZERO;
        let mut h5 = Hand::ZERO;

        // h4: 歩3, 金1
        for _ in 0..3 {
            h4 = Hand::add_one(h4, HandPiece::PAWN);
        }
        h4 = Hand::add_one(h4, HandPiece::GOLD);

        // h5: 歩2, 金1
        for _ in 0..2 {
            h5 = Hand::add_one(h5, HandPiece::PAWN);
        }
        h5 = Hand::add_one(h5, HandPiece::GOLD);

        assert!(Hand::is_equal_or_superior(h4, h5));
        assert!(!Hand::is_equal_or_superior(h5, h4));
    }

    #[test]
    fn test_hand_overflow_boundary() {
        let mut hand = Hand::ZERO;

        // 歩を最大枚数（18枚）近くまで追加
        for _ in 0..18 {
            hand = Hand::add_one(hand, HandPiece::PAWN);
        }
        assert_eq!(Hand::count_of(hand, HandPiece::PAWN), 18);

        // 金を最大枚数（4枚）追加
        for _ in 0..4 {
            hand = Hand::add_one(hand, HandPiece::GOLD);
        }
        assert_eq!(Hand::count_of(hand, HandPiece::GOLD), 4);

        // 他の駒種が影響を受けていないことを確認
        assert_eq!(Hand::count_of(hand, HandPiece::PAWN), 18);
        assert_eq!(Hand::count_of(hand, HandPiece::LANCE), 0);
    }

    #[test]
    fn test_hand_has_overflow() {
        let mut hand = Hand::ZERO;
        for _ in 0..31 {
            hand = Hand::add_one(hand, HandPiece::PAWN);
        }
        assert!(!hand.has_overflow());

        hand = Hand::add_one(hand, HandPiece::PAWN);
        assert!(hand.has_overflow());
    }

    #[test]
    fn test_hand_checked_add() {
        let hand =
            Hand::ZERO.checked_add(HandPiece::PAWN, 18).expect("pawn count within field width");
        assert_eq!(hand.count(HandPiece::PAWN), 18);
        assert!(!hand.has_overflow());

        let hand =
            Hand::ZERO.checked_add(HandPiece::GOLD, 7).expect("gold count within field width");
        assert_eq!(hand.count(HandPiece::GOLD), 7);
        assert!(hand.checked_add(HandPiece::GOLD, 1).is_none());

        assert!(Hand::ZERO.checked_add(HandPiece::PAWN, 32).is_none());
    }
}
