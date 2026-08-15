//! Zobrist ハッシュキー。
//!
//! [`ZobristKey`] は局面の同一性判定に使用するハッシュ値。駒の配置、持ち駒、
//! 手番の各要素にランダムなビット列を事前に割り当て、XOR で合成することで
//! 高速にハッシュを計算する。
//!
//! 指し手の適用時は差分更新のみで済むため、全駒を走査せずにハッシュを維持できる。
//!
//! # キーの代数
//!
//! [`ZobristKey`] が持つ演算は XOR だけである（`BitXor` / `BitXorAssign`）。
//! 持ち駒は枚数ごとに独立したキーを引くため（[`Zobrist::hand`]）、
//! 加算・減算・乗算を必要とする要素が存在しない。
//! この縮退により差分更新は do と undo で同一演算になり、符号の取り違えが起こらない。
//!
//! # 永続データとの互換性
//!
//! テーブルは固定シードの Xorshift64\* から決定的に生成する。
//! `side` も通常の乱数定数であり、キーの特定 bit に手番を埋め込まない。
//! 生成方法と合成規則は保存済みの rsshogi book key の一部なので、変更するときは
//! versioned format として扱う必要がある。
//!
//! # ビット幅
//!
//! デフォルトは 64bit、`hash-128` feature を有効にすると 128bit になる。
//! 128bit 構成の [`ZobristKey`] は独立した 64bit XOR レーン 2 本にすぎない。

use crate::types::{Color, Hand, HandPiece, Piece, PieceType, Square};

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
}

impl From<u64> for ZobristKey {
    #[inline]
    fn from(value: u64) -> Self {
        Self::from_u64(value)
    }
}

// ANCHOR: zobrist_key_ops
impl std::ops::BitXor for ZobristKey {
    type Output = Self;

    #[inline]
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
    #[inline]
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
// ANCHOR_END: zobrist_key_ops

// ANCHOR: zobrist_table_struct
/// 局面ハッシュ用の Zobrist テーブル構造体。
#[derive(Debug)]
pub struct ZobristTable {
    /// 駒種とマスで添字付けした、盤上の駒のハッシュ値。
    board: [[ZobristKey; Square::COUNT_WITH_NONE]; Piece::COUNT],
    /// 手番と「駒種 + 枚数」の flat slot で添字付けした、持ち駒のハッシュ値。
    ///
    /// slot は駒種ごとに `0..=表現可能な最大枚数` を連続で並べた ragged layout
    /// （歩 32・香桂銀金 各 8・角飛 各 4 で計 [`ZobristTable::HAND_SLOT_COUNT`]）。
    /// 枚数 0 の slot は常にゼロで、空の持ち駒がキーに寄与しないことを保証する。
    hand: [[ZobristKey; HAND_SLOT_COUNT]; Color::COUNT],
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

/// `PieceType` index から持ち駒の枚数マスクへの写像。
///
/// `Hand` のビットフィールド幅から導出するため、`Hand` のレイアウトを変えると
/// テーブル形状が自動追随する。持ち駒にならない駒種と `NONE` は 0。
///
/// テーブルの定義域を「物理的な最大枚数」ではなく「`Hand` が表現できる枚数」に取るのは、
/// SFEN パーサが物理上限を超える持ち駒数を受理し、その検出を
/// [`Position::validate`](crate::board::Position::validate) に委ねる設計だからである。
/// 全再計算は局面構築時（`validate` より前）に走るため、
/// 表現可能な全枚数に対してキーが定義されている必要がある。
const HAND_COUNT_MASK: [u32; PieceType::COUNT] = hand_count_mask_table();

/// `PieceType` index から持ち駒 slot の先頭位置への写像。
///
/// 各駒種は `HAND_OFFSET[pt] + (count & HAND_COUNT_MASK[pt])` の位置を占める。
/// 持ち駒にならない駒種と `NONE` は offset 0 / mask 0 のため slot 0（歩 0 枚 = ゼロキー）に落ちる。
const HAND_OFFSET: [u16; PieceType::COUNT] = hand_offset_table();

/// 持ち駒テーブルの flat slot 数（各駒種の `0..=表現可能な最大枚数` を連結した長さ）。
const HAND_SLOT_COUNT: usize = hand_slot_count();

const fn hand_count_mask_table() -> [u32; PieceType::COUNT] {
    let mut table = [0; PieceType::COUNT];
    let mut idx = 1;
    while idx < HAND_PIECES {
        // 持ち駒テーブルの範囲 1..HAND_PIECES は全て HandPiece に対応する。
        if let Some(hp) = HandPiece::from_piece_type(PieceType::new(idx as i8)) {
            table[idx] = Hand::count_mask(hp);
        }
        idx += 1;
    }
    table
}

const fn hand_offset_table() -> [u16; PieceType::COUNT] {
    let masks = hand_count_mask_table();
    let mut table = [0; PieceType::COUNT];
    let mut idx = 1;
    let mut acc = 0;
    while idx < HAND_PIECES {
        table[idx] = acc;
        acc += masks[idx] as u16 + 1;
        idx += 1;
    }
    table
}

const fn hand_slot_count() -> usize {
    let masks = hand_count_mask_table();
    let mut idx = 1;
    let mut acc = 0;
    while idx < HAND_PIECES {
        acc += masks[idx] as usize + 1;
        idx += 1;
    }
    acc
}

const _: () = {
    // 歩 32 + 香桂銀金 8 × 4 + 角飛 4 × 2 = 72 slot。
    assert!(HAND_SLOT_COUNT == 72);
    // 持ち駒にならない駒種は slot 0 に落ち、ゼロキーを返す。
    let mut idx = HAND_PIECES;
    while idx < PieceType::COUNT {
        assert!(HAND_OFFSET[idx] == 0 && HAND_COUNT_MASK[idx] == 0);
        idx += 1;
    }
    assert!(HAND_OFFSET[0] == 0 && HAND_COUNT_MASK[0] == 0);
};

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
    let mut hand = [[ZobristKey::from_u64(0); HAND_SLOT_COUNT]; Color::COUNT];

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

    // 枚数 0 の slot は draw せずゼロのまま残す。空の持ち駒がキーに寄与しないことで、
    // 全再計算が枚数 0 の駒種を読み飛ばせる。
    let mut color_idx = 0;
    while color_idx < Color::COUNT {
        let mut hand_piece_idx = 1;
        while hand_piece_idx < HAND_PIECES {
            let offset = HAND_OFFSET[hand_piece_idx] as usize;
            let max = HAND_COUNT_MASK[hand_piece_idx] as usize;
            let mut count = 1;
            while count <= max {
                hand[color_idx][offset + count] = next_key(&mut rng_state);
                count += 1;
            }
            hand_piece_idx += 1;
        }
        color_idx += 1;
    }

    ZobristTable { board, hand, no_pawns, side }
}

/// PRNG から次のキーを 1 つ取り出す。
///
/// ビット幅によらず PRNG をちょうど 4 回消費し、`low` は常に 1 draw 目とする。
/// これにより 64bit ビルドのキーは 128bit ビルドの low limb と一致し、
/// `hash-128` feature の on/off でキーの下位 64bit が食い違わない。
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

/// `PieceType` index と枚数から持ち駒テーブルの flat slot index を求める。
///
/// `count` はフィールド幅でマスクするため、全ての `u32` に対して定義される。
/// マスクの折り返しは `Hand::add` / `Hand::sub` がフィールド幅を超えたときの
/// 挙動と一致するため、差分更新と全再計算は表現可能な全状態で整合する。
#[inline]
fn hand_slot(piece_idx: usize, count: u32) -> usize {
    HAND_OFFSET[piece_idx] as usize + (count & HAND_COUNT_MASK[piece_idx]) as usize
}

/// Zobrist ハッシュテーブルへの公開アクセサ。
///
/// 下流 crate から canonical な Zobrist テーブルを参照したい場合は、
/// module-level static ではなく [`Zobrist::instance()`] を使用する。
pub struct Zobrist;

impl ZobristTable {
    /// 持ち駒テーブルの flat slot 数。
    pub const HAND_SLOT_COUNT: usize = HAND_SLOT_COUNT;

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

    /// 手駒（color index, flat slot index）のハッシュを返す。
    ///
    /// `color_idx` は `0..Color::COUNT`、`slot_idx` は `0..Self::HAND_SLOT_COUNT`。
    /// slot は「駒種 + 枚数」を連結した ragged layout だが、**その並びは実装詳細**であり
    /// `Hand` のビットレイアウト変更に追随して変わる。駒種と枚数から引く場合は
    /// 安定した入口である [`Zobrist::hand`] を使う。
    #[inline]
    #[must_use]
    pub fn hand_at_index(&self, color_idx: usize, slot_idx: usize) -> ZobristKey {
        self.hand[color_idx][slot_idx]
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
    #[inline]
    #[must_use]
    pub fn psq(sq: Square, piece: Piece) -> ZobristKey {
        let table = Self::instance();
        let piece_idx = piece.to_index();
        // `COUNT_WITH_NONE` 相当のテーブル参照は `SQ_NONE` を含めて index 化する。
        let square_idx = sq.to_index_with_none();
        table.board[piece_idx][square_idx]
    }

    /// 持ち駒を `count` 枚保有している状態のハッシュを返す。
    ///
    /// `count == 0`、および持ち駒にならない駒種では常にゼロを返す。
    /// 枚数ごとに独立したキーを引くため、合成は XOR で行う。
    ///
    /// `count` は `Hand` のフィールド幅でマスクするため、`Hand::count` が返しうる
    /// 全ての値に対して定義される。物理的な最大枚数（歩 18 枚など）を超える枚数にも
    /// 一意のキーが割り当てられる。これは SFEN パーサが物理上限を超える持ち駒数を
    /// 受理し、その検出を [`Position::validate`](crate::board::Position::validate) に
    /// 委ねる設計に合わせたもので、局面構築時の全再計算が panic しないことを保証する。
    #[inline]
    #[must_use]
    pub fn hand(color: Color, piece_type: PieceType, count: u32) -> ZobristKey {
        let piece_idx = piece_type.to_index();
        Self::instance().hand[color.to_index()][hand_slot(piece_idx, count)]
    }

    /// 持ち駒の枚数が `from` から `to` へ変化したときのハッシュ差分を返す。
    ///
    /// `hand(color, piece_type, from) ^ hand(color, piece_type, to)` と同値で、
    /// 合成キーへはこの戻り値をそのまま XOR すればよい。
    #[inline]
    #[must_use]
    pub fn hand_delta(color: Color, piece_type: PieceType, from: u32, to: u32) -> ZobristKey {
        let piece_idx = piece_type.to_index();
        let row = &Self::instance().hand[color.to_index()];
        row[hand_slot(piece_idx, from)] ^ row[hand_slot(piece_idx, to)]
    }

    /// 手番のハッシュを返す。
    #[inline]
    #[must_use]
    pub fn side() -> ZobristKey {
        Self::instance().side()
    }

    /// 歩がない局面のハッシュを返す。
    #[inline]
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
            for slot in 0..HAND_SLOT_COUNT {
                if ZOBRIST.hand_at_index(color, slot) != ZobristKey::default() {
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
            for slot in 0..HAND_SLOT_COUNT {
                let hash = ZOBRIST.hand_at_index(color, slot);
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
        assert_eq!(ZOBRIST.hand[0].len(), HAND_SLOT_COUNT);

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

    /// 枚数 0 の slot がゼロであることを全駒種・全色で確認する。
    ///
    /// 空の持ち駒が合成キーに寄与しないという前提は、全再計算が枚数 0 の駒種を
    /// 読み飛ばせる根拠そのものであり、崩れると全再計算と差分更新が食い違う。
    /// accessor と generator が同じ誤りを犯しても検出できるよう、
    /// `hand_at_index` で slot を直接引く経路も併せて固定する。
    #[test]
    fn test_zobrist_zero_count_is_zero() {
        for color in [Color::BLACK, Color::WHITE] {
            for (piece_idx, &offset) in HAND_OFFSET.iter().enumerate().take(HAND_PIECES).skip(1) {
                let piece_type = PieceType::new(i8::try_from(piece_idx).expect("index fits in i8"));
                assert_eq!(
                    Zobrist::hand(color, piece_type, 0),
                    ZobristKey::default(),
                    "hand hash for count 0 should be zero: color={color:?} piece_type={piece_type:?}"
                );
                assert_eq!(
                    ZOBRIST.hand_at_index(color.to_index(), offset as usize),
                    ZobristKey::default(),
                    "slot at the head of each piece type should be zero: piece_type={piece_type:?}"
                );
            }
            assert_eq!(
                Zobrist::hand(color, PieceType::NONE, 0),
                ZobristKey::default(),
                "hand hash for NO_PIECE_TYPE should be zero"
            );
        }
    }

    /// 持ち駒キーが枚数ごとに一意であり、`hand_delta` が 2 枚のキーの XOR と一致することを確認する。
    #[test]
    fn test_zobrist_hand_keys_are_unique_and_delta_is_consistent() {
        let mut hashes = HashSet::new();

        for color in [Color::BLACK, Color::WHITE] {
            for (piece_idx, &max) in HAND_COUNT_MASK.iter().enumerate().take(HAND_PIECES).skip(1) {
                let piece_type = PieceType::new(i8::try_from(piece_idx).expect("index fits in i8"));

                for count in 1..=max {
                    let key = Zobrist::hand(color, piece_type, count);
                    assert_ne!(key, ZobristKey::default(), "non-zero count must have a key");
                    assert!(
                        hashes.insert(key),
                        "duplicate hand hash: color={color:?} piece_type={piece_type:?} count={count}"
                    );
                }

                for from in 0..=max {
                    for to in 0..=max {
                        assert_eq!(
                            Zobrist::hand_delta(color, piece_type, from, to),
                            Zobrist::hand(color, piece_type, from)
                                ^ Zobrist::hand(color, piece_type, to),
                            "hand_delta must equal the XOR of both counts"
                        );
                    }
                }
            }
        }
    }

    /// 表現可能な枚数を超える引数がマスクで折り返り、panic しないことを確認する。
    ///
    /// SFEN パーサは物理上限を超える持ち駒数を受理し、その検出は `Position::validate`
    /// に委ねられる。全再計算は検証より前に走るため、キー計算は表現可能な全枚数に対して
    /// 定義されていなければならない。
    #[test]
    fn test_zobrist_hand_count_wraps_at_field_width() {
        let color = Color::BLACK;
        // 歩は 5bit なので 32 枚は 0 枚と同じ slot に折り返る。
        assert_eq!(Zobrist::hand(color, PieceType::PAWN, 32), ZobristKey::default());
        assert_eq!(
            Zobrist::hand(color, PieceType::PAWN, 33),
            Zobrist::hand(color, PieceType::PAWN, 1)
        );
        // 物理上限の 18 枚を超えても一意のキーを持つ。
        assert_ne!(
            Zobrist::hand(color, PieceType::PAWN, 19),
            Zobrist::hand(color, PieceType::PAWN, 18)
        );
        // 持ち駒にならない駒種は常にゼロ。
        assert_eq!(Zobrist::hand(color, PieceType::KING, 1), ZobristKey::default());
        assert_eq!(Zobrist::hand(color, PieceType::HORSE, 3), ZobristKey::default());
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
