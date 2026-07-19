//! Zobrist ハッシュキー。
//!
//! [`ZobristKey`] は局面の同一性判定に使用するハッシュ値。駒の配置、持ち駒、
//! 手番の各要素にランダムなビット列を事前に割り当て、XOR で合成することで
//! 高速にハッシュを計算する。
//!
//! 指し手の適用時は差分更新のみで済むため、全駒を走査せずにハッシュを維持できる。
//!
//! YaneuraOu と互換のハッシュ値を生成する。デフォルトは 64bit、`hash-128`
//! feature を有効にすると 128bit になる。

use crate::types::{Color, Piece, PieceType, Square};

// ANCHOR: zobrist_key_struct
/// デフォルトで 64 bit、`hash-128` feature で 128 bit のハッシュキー。
#[cfg(not(feature = "hash-128"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ZobristKey(u64);
// ANCHOR_END: zobrist_key_struct

#[cfg(not(feature = "hash-128"))]
impl ZobristKey {
    #[must_use]
    pub const fn new(low: u64, _high: u64) -> Self {
        Self(low)
    }

    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn low_u64(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn high_u64(self) -> u64 {
        0
    }

    // ANCHOR: zobrist_key_ops
    /// ハッシュ更新用の XOR 演算。
    pub fn xor(&mut self, other: Self) {
        self.0 ^= other.0;
    }

    /// material key 更新用の加算。
    pub fn add(&mut self, other: Self) {
        self.0 = self.0.wrapping_add(other.0);
    }

    /// material key 更新用の減算。
    pub fn sub(&mut self, other: Self) {
        self.0 = self.0.wrapping_sub(other.0);
    }
    // ANCHOR_END: zobrist_key_ops

    #[must_use]
    pub const fn mul_u64(self, rhs: u64) -> Self {
        Self(self.0.wrapping_mul(rhs))
    }
}

#[cfg(feature = "hash-128")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ZobristKey {
    low: u64,
    high: u64,
}

#[cfg(feature = "hash-128")]
impl ZobristKey {
    #[must_use]
    pub const fn new(low: u64, high: u64) -> Self {
        Self { low, high }
    }

    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self { low: value, high: 0 }
    }

    #[must_use]
    pub const fn low_u64(self) -> u64 {
        self.low
    }

    #[must_use]
    pub const fn high_u64(self) -> u64 {
        self.high
    }

    /// ハッシュ更新用の XOR 演算。
    pub fn xor(&mut self, other: Self) {
        self.low ^= other.low;
        self.high ^= other.high;
    }

    /// material key 更新用の加算。
    pub fn add(&mut self, other: Self) {
        self.low = self.low.wrapping_add(other.low);
        self.high = self.high.wrapping_add(other.high);
    }

    /// material key 更新用の減算。
    pub fn sub(&mut self, other: Self) {
        self.low = self.low.wrapping_sub(other.low);
        self.high = self.high.wrapping_sub(other.high);
    }

    #[must_use]
    pub const fn mul_u64(self, rhs: u64) -> Self {
        Self { low: self.low.wrapping_mul(rhs), high: self.high.wrapping_mul(rhs) }
    }
}

impl From<u64> for ZobristKey {
    fn from(value: u64) -> Self {
        Self::from_u64(value)
    }
}

impl std::ops::BitXor for ZobristKey {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        #[cfg(not(feature = "hash-128"))]
        {
            Self(self.0 ^ rhs.0)
        }
        #[cfg(feature = "hash-128")]
        {
            Self { low: self.low ^ rhs.low, high: self.high ^ rhs.high }
        }
    }
}

impl std::ops::BitXorAssign for ZobristKey {
    fn bitxor_assign(&mut self, rhs: Self) {
        #[cfg(not(feature = "hash-128"))]
        {
            self.0 ^= rhs.0;
        }
        #[cfg(feature = "hash-128")]
        {
            self.low ^= rhs.low;
            self.high ^= rhs.high;
        }
    }
}

// ANCHOR: zobrist_table_struct
/// 局面ハッシュ用の Zobrist テーブル構造体。
#[derive(Debug)]
pub struct ZobristTable {
    /// 駒種とマスで添字付けした、盤上の駒のハッシュ値。
    board: [[ZobristKey; Square::COUNT_WITH_NONE]; Piece::COUNT],
    /// 手番と駒種で添字付けした、持ち駒のハッシュ値。
    /// `count == 1` の値を基準とし、手駒数に応じて加算する。
    hand: [[ZobristKey; PieceType::HAND_TABLE_SIZE]; Color::COUNT],
    /// 歩がない局面用のハッシュ値。
    no_pawns: ZobristKey,
    /// 手番のハッシュ値（BLACK = 0、WHITE = この値）。
    side: ZobristKey,
}
// ANCHOR_END: zobrist_table_struct

pub const HASH_KEY_BITS: u32 = if cfg!(feature = "hash-128") { 128 } else { 64 };

const BOARD_PIECES: usize = Piece::COUNT;
const BOARD_SQUARES: usize = Square::COUNT;
const HAND_PIECES: usize = PieceType::HAND_TABLE_SIZE;
const MATERIAL_SQUARE_INDEX: usize = 8;
const ZOBRIST_SEED: u64 = 20_151_225;
const PRNG_MULTIPLIER: u64 = 2_685_821_657_736_338_717;

const fn prng_next_u64(state: &mut u64) -> u64 {
    let mut s = *state;
    s ^= s >> 12;
    s ^= s << 25;
    s ^= s >> 27;
    *state = s;
    s.wrapping_mul(PRNG_MULTIPLIER)
}

// ANCHOR: zobrist_init
#[allow(clippy::large_stack_arrays)]
const fn generate_zobrist_table() -> ZobristTable {
    let mut rng_state = ZOBRIST_SEED;
    let mut board = [[ZobristKey::from_u64(0); Square::COUNT_WITH_NONE]; BOARD_PIECES];
    let mut hand = [[ZobristKey::from_u64(0); HAND_PIECES]; Color::COUNT];

    let side = next_key(&mut rng_state);
    let no_pawns = next_key(&mut rng_state);

    let mut piece_idx = 1;
    while piece_idx < BOARD_PIECES {
        let mut square_idx = 0;
        while square_idx < BOARD_SQUARES {
            board[piece_idx][square_idx] = next_key(&mut rng_state);
            square_idx += 1;
        }
        piece_idx += 1;
    }

    let mut color_idx = 0;
    while color_idx < Color::COUNT {
        let mut hand_piece_idx = 1;
        while hand_piece_idx < HAND_PIECES {
            hand[color_idx][hand_piece_idx] = next_key(&mut rng_state);
            hand_piece_idx += 1;
        }
        color_idx += 1;
    }

    ZobristTable { board, hand, no_pawns, side }
}

const fn next_key(state: &mut u64) -> ZobristKey {
    let low = prng_next_u64(state);
    let high = if HASH_KEY_BITS >= 128 { prng_next_u64(state) } else { 0 };

    let remaining = if HASH_KEY_BITS >= 128 { 2 } else { 3 };
    let mut i = 0;
    while i < remaining {
        let _ = prng_next_u64(state);
        i += 1;
    }

    ZobristKey::new(low, high)
}

/// 固定シードから決定的に生成する、グローバルな Zobrist テーブル実体。
static ZOBRIST: ZobristTable = generate_zobrist_table();
// ANCHOR_END: zobrist_init

/// Zobrist ハッシュテーブルへの公開アクセサ。
///
/// 下流 crate から canonical な Zobrist テーブルを参照したい場合は、
/// module-level static ではなく [`Zobrist::instance()`] を使用する。
pub struct Zobrist;

impl ZobristTable {
    /// 手番のハッシュを返す。
    #[inline]
    #[must_use]
    pub const fn side(&self) -> ZobristKey {
        self.side
    }

    /// no-pawns のハッシュを返す。
    #[inline]
    #[must_use]
    pub const fn no_pawns(&self) -> ZobristKey {
        self.no_pawns
    }

    /// 盤上（piece index, square index）のハッシュを返す。
    ///
    /// `piece_idx` は `0..Piece::COUNT`、`square_idx` は `0..Square::COUNT_WITH_NONE`。
    #[inline]
    #[must_use]
    pub fn board_at_index(&self, piece_idx: usize, square_idx: usize) -> ZobristKey {
        self.board[piece_idx][square_idx]
    }

    /// 手駒（color index, piece_type index）のハッシュを返す。
    ///
    /// `color_idx` は `0..Color::COUNT`、`piece_type_idx` は `0..PieceType::HAND_TABLE_SIZE`。
    #[inline]
    #[must_use]
    pub fn hand_at_index(&self, color_idx: usize, piece_type_idx: usize) -> ZobristKey {
        self.hand[color_idx][piece_type_idx]
    }
}

impl Zobrist {
    /// 決定的に生成された共有 Zobrist テーブルを返す。
    ///
    /// 下流 crate が `board::zobrist` の公開 surface を再公開したい場合も、
    /// この accessor を安定した入口として利用できる。
    ///
    /// # Examples
    ///
    /// ```
    /// use rsshogi::board::zobrist::Zobrist;
    ///
    /// let table = Zobrist::instance();
    /// assert_eq!(table.side(), Zobrist::side());
    /// ```
    #[must_use]
    pub fn instance() -> &'static ZobristTable {
        &ZOBRIST
    }

    /// マス上の駒のハッシュを返す。
    #[must_use]
    pub fn psq(sq: Square, piece: Piece) -> ZobristKey {
        let table = Self::instance();
        let piece_idx = piece.to_index();
        // `COUNT_WITH_NONE` 相当のテーブル参照は `SQ_NONE` を含めて index 化する。
        let square_idx = sq.to_index_with_none();
        table.board[piece_idx][square_idx]
    }

    /// マスに依存しない駒のハッシュ（material key）を返す。
    #[must_use]
    pub fn material(piece: Piece) -> ZobristKey {
        let table = Self::instance();
        let piece_idx = piece.to_index();
        table.board[piece_idx][MATERIAL_SQUARE_INDEX]
    }

    /// 持ち駒のハッシュを基本値と枚数の積として返す。
    #[must_use]
    pub fn hand(color: Color, piece_type: PieceType, count: usize) -> ZobristKey {
        let table = Self::instance();
        let piece_idx = piece_type.to_index();
        if piece_idx == 0 || piece_idx >= PieceType::HAND_TABLE_SIZE || count == 0 {
            return ZobristKey::default();
        }
        let base = table.hand[color.to_index()][piece_idx];
        base.mul_u64(count as u64)
    }

    /// 手番のハッシュを返す。
    #[must_use]
    pub fn side() -> ZobristKey {
        Self::instance().side()
    }

    /// 歩がない局面のハッシュを返す。
    #[must_use]
    pub fn no_pawns() -> ZobristKey {
        Self::instance().no_pawns()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_zobrist_table_structure() {
        assert_ne!(ZOBRIST.side(), ZobristKey::default(), "Side hash should not be zero");
        assert_ne!(ZOBRIST.no_pawns(), ZobristKey::default(), "No-pawns hash should not be zero");

        let mut non_zero_count = 0;
        for piece in 0..Piece::COUNT {
            for square in 0..Square::COUNT {
                if ZOBRIST.board_at_index(piece, square) != ZobristKey::default() {
                    non_zero_count += 1;
                }
            }
        }
        assert!(non_zero_count > 0, "Board hashes should not all be zero");

        non_zero_count = 0;
        for color in 0..Color::COUNT {
            for piece in 1..PieceType::HAND_TABLE_SIZE {
                if ZOBRIST.hand_at_index(color, piece) != ZobristKey::default() {
                    non_zero_count += 1;
                }
            }
        }
        assert!(non_zero_count > 0, "Hand hashes should not all be zero");
    }

    #[test]
    fn test_zobrist_table_uniqueness() {
        let mut hashes = HashSet::new();

        hashes.insert(ZOBRIST.side());
        hashes.insert(ZOBRIST.no_pawns());

        for piece in 0..Piece::COUNT {
            for square in 0..Square::COUNT {
                let hash = ZOBRIST.board_at_index(piece, square);
                if hash != ZobristKey::default() {
                    assert!(
                        hashes.insert(hash),
                        "Duplicate hash found in board table: low={:#016x} high={:#016x}",
                        hash.low_u64(),
                        hash.high_u64()
                    );
                }
            }
        }

        for color in 0..Color::COUNT {
            for piece in 1..PieceType::HAND_TABLE_SIZE {
                let hash = ZOBRIST.hand_at_index(color, piece);
                if hash != ZobristKey::default() {
                    assert!(
                        hashes.insert(hash),
                        "Duplicate hash found in hand table: low={:#016x} high={:#016x}",
                        hash.low_u64(),
                        hash.high_u64()
                    );
                }
            }
        }

        let unique_count = hashes.len();
        assert!(unique_count > 100, "Too few unique hashes: {unique_count}");
    }

    #[test]
    fn test_zobrist_table_size_verification() {
        assert_eq!(ZOBRIST.board.len(), Piece::COUNT);
        assert_eq!(ZOBRIST.board[0].len(), Square::COUNT_WITH_NONE);

        assert_eq!(ZOBRIST.hand.len(), Color::COUNT);
        assert_eq!(ZOBRIST.hand[0].len(), PieceType::HAND_TABLE_SIZE);

        assert_ne!(ZOBRIST.side(), ZobristKey::default());
    }

    #[test]
    fn test_zobrist_no_piece_is_zero() {
        for square in 0..Square::COUNT_WITH_NONE {
            assert_eq!(
                ZOBRIST.board_at_index(0, square),
                ZobristKey::default(),
                "NO_PIECE hash at square {square} should be 0"
            );
        }
    }

    #[test]
    fn test_zobrist_zero_count_is_zero() {
        for color in 0..Color::COUNT {
            assert_eq!(
                ZOBRIST.hand_at_index(color, 0),
                ZobristKey::default(),
                "Hash for NO_PIECE_TYPE in hand should be 0 for color {color}"
            );
        }
    }

    #[test]
    fn test_zobrist_reproducibility() {
        assert_eq!(ZOBRIST.board_at_index(1, 0), ZOBRIST.board_at_index(1, 0));
        assert_ne!(ZOBRIST.side(), ZobristKey::default());
    }

    #[test]
    fn test_zobrist_distribution() {
        let mut bit_count = [0u32; 64];

        for piece in 1..Piece::COUNT {
            for square in 0..Square::COUNT {
                let hash = ZOBRIST.board_at_index(piece, square).low_u64();
                for (bit, count) in bit_count.iter_mut().enumerate() {
                    if (hash >> bit) & 1 == 1 {
                        *count += 1;
                    }
                }
            }
        }

        let piece_count = u32::try_from(Piece::COUNT).expect("piece count fits in u32");
        let square_count = u32::try_from(Square::COUNT).expect("square count fits in u32");
        let total: u32 = (piece_count - 1) * square_count;
        for (bit, count) in bit_count.iter().enumerate() {
            let ratio = f64::from(*count) / f64::from(total);
            assert!((0.3..0.7).contains(&ratio), "Bit {bit} has unusual distribution: {ratio:.2}");
        }
    }
}
