//! 局面履歴管理
//!
//! [`StateInfo`] は `apply_move32` 時に生成される差分情報を保持し、
//! `undo_move32` で局面を完全に復元するために使用する。
//!
//! # 保持する情報
//!
//! - 取られた駒（undo 時の復元用）
//! - Zobrist ハッシュキー（盤面と持ち駒）
//! - 千日手検出用カウンタ
//! - 王手駒のキャッシュ（[`StateInfo::checkers`]）
//! - ピン情報（[`StateInfo::pinners`], [`StateInfo::blockers_for_king`]）
//! - 王手候補マス（[`StateInfo::check_squares`]）
//!
//! [`StateStack`] は `StateInfo` のスタックで、局面ツリーの
//! 探索中に履歴を管理する。

#![allow(clippy::inline_always)]

use super::position::Ply;
#[cfg(test)]
use super::position::Position;
use crate::board::zobrist::{Zobrist, ZobristKey};
use crate::types::Bitboard;
use crate::types::{Color, Hand, MOVE_NONE, Move32, Piece, PieceType, RepetitionState};
use std::ops::{Deref, DerefMut, Index};

/// `StateInfo` 内でのスタック位置を表す型
pub type StateIndex = usize;

/// `check_squares` の圧縮配列サイズ。
///
/// Gold と同じ利きを持つ成小駒を 1 slot に統合し、直接王手に使わない
/// `NONE` / `KING` / `GOLD_LIKE` を省略する。
pub(crate) const CHECK_SQUARES_TABLE_SIZE: usize = 9;

pub(crate) const CHECK_SQ_PAWN: usize = 0;
pub(crate) const CHECK_SQ_LANCE: usize = 1;
pub(crate) const CHECK_SQ_KNIGHT: usize = 2;
pub(crate) const CHECK_SQ_SILVER: usize = 3;
pub(crate) const CHECK_SQ_BISHOP: usize = 4;
pub(crate) const CHECK_SQ_ROOK: usize = 5;
pub(crate) const CHECK_SQ_GOLD: usize = 6;
pub(crate) const CHECK_SQ_HORSE: usize = 7;
pub(crate) const CHECK_SQ_DRAGON: usize = 8;

const CHECK_SQ_INVALID: u8 = u8::MAX;

const CHECK_SQ_INDEX: [u8; PieceType::COUNT] = [
    CHECK_SQ_INVALID,      // NONE
    CHECK_SQ_PAWN as u8,   // PAWN
    CHECK_SQ_LANCE as u8,  // LANCE
    CHECK_SQ_KNIGHT as u8, // KNIGHT
    CHECK_SQ_SILVER as u8, // SILVER
    CHECK_SQ_BISHOP as u8, // BISHOP
    CHECK_SQ_ROOK as u8,   // ROOK
    CHECK_SQ_GOLD as u8,   // GOLD
    CHECK_SQ_INVALID,      // KING
    CHECK_SQ_GOLD as u8,   // PRO_PAWN
    CHECK_SQ_GOLD as u8,   // PRO_LANCE
    CHECK_SQ_GOLD as u8,   // PRO_KNIGHT
    CHECK_SQ_GOLD as u8,   // PRO_SILVER
    CHECK_SQ_HORSE as u8,  // HORSE
    CHECK_SQ_DRAGON as u8, // DRAGON
    CHECK_SQ_INVALID,      // GOLD_LIKE
];

/// 駒種別の王手候補マスを圧縮して保持する cache。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckSquares {
    inner: [Bitboard; CHECK_SQUARES_TABLE_SIZE],
}

impl CheckSquares {
    pub const EMPTY: Self = Self { inner: [Bitboard::EMPTY; CHECK_SQUARES_TABLE_SIZE] };

    #[inline]
    const fn index(piece_type: PieceType) -> Option<usize> {
        let idx = piece_type.to_index();
        if idx >= PieceType::COUNT {
            return None;
        }
        let compressed = CHECK_SQ_INDEX[idx];
        if compressed == CHECK_SQ_INVALID { None } else { Some(compressed as usize) }
    }

    #[inline]
    #[must_use]
    pub const fn get(&self, piece_type: PieceType) -> Bitboard {
        match Self::index(piece_type) {
            Some(idx) => self.inner[idx],
            None => Bitboard::EMPTY,
        }
    }

    #[inline]
    pub(crate) fn set(&mut self, idx: usize, value: Bitboard) {
        self.inner[idx] = value;
    }

    /// 圧縮済みの駒種別 check-square 配列から構築する。
    #[inline]
    #[must_use]
    pub const fn new(inner: [Bitboard; CHECK_SQUARES_TABLE_SIZE]) -> Self {
        Self { inner }
    }

    /// 圧縮表現を `PieceType::COUNT` 長の完全配列へ展開する。
    #[inline]
    #[must_use]
    pub fn to_full_array(self) -> [Bitboard; PieceType::COUNT] {
        std::array::from_fn(|idx| self.get(PieceType::new(i8::try_from(idx).unwrap_or(i8::MAX))))
    }

    /// 圧縮済みの内部配列への参照を返す。
    #[inline]
    #[must_use]
    pub const fn as_compressed(&self) -> &[Bitboard; CHECK_SQUARES_TABLE_SIZE] {
        &self.inner
    }
}

impl Default for CheckSquares {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TacticalCache {
    checkers: Bitboard,
    pinners: [Bitboard; Color::COUNT],
    blockers_for_king: [Bitboard; Color::COUNT],
    pub(crate) check_squares: CheckSquares,
}

impl TacticalCache {
    pub(crate) const EMPTY: Self = Self {
        checkers: Bitboard::EMPTY,
        pinners: [Bitboard::EMPTY; Color::COUNT],
        blockers_for_king: [Bitboard::EMPTY; Color::COUNT],
        check_squares: CheckSquares::EMPTY,
    };

    #[inline]
    #[must_use]
    pub(crate) const fn new(
        checkers: Bitboard,
        pinners: [Bitboard; Color::COUNT],
        blockers_for_king: [Bitboard; Color::COUNT],
        check_squares: CheckSquares,
    ) -> Self {
        Self { checkers, pinners, blockers_for_king, check_squares }
    }

    #[inline]
    #[must_use]
    pub(crate) const fn checkers(&self) -> Bitboard {
        self.checkers
    }

    #[inline]
    #[must_use]
    pub(crate) const fn pinners(&self, color: Color) -> Bitboard {
        self.pinners[color.to_index()]
    }

    #[inline]
    #[must_use]
    pub(crate) const fn blockers_for_king(&self, color: Color) -> Bitboard {
        self.blockers_for_king[color.to_index()]
    }

    #[inline]
    #[must_use]
    pub(crate) const fn check_square(&self, piece_type: PieceType) -> Bitboard {
        self.check_squares.get(piece_type)
    }

    #[inline]
    pub(crate) fn set_checkers(&mut self, checkers: Bitboard) {
        self.checkers = checkers;
    }

    #[inline]
    pub(crate) fn set_check_squares(&mut self, check_squares: CheckSquares) {
        self.check_squares = check_squares;
    }
}

impl Default for TacticalCache {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// 探索ホットパスで頻出する部分ハッシュと駒割り情報のキャッシュ。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartialKeys {
    /// 盤上の歩のみから構成される Zobrist キー。
    pub pawn: ZobristKey,

    /// 盤上の小駒のみから構成される Zobrist キー。
    pub minor: ZobristKey,

    /// 色別の「歩以外の盤上駒」から構成される Zobrist キー。
    pub non_pawn: [ZobristKey; Color::COUNT],

    /// 盤上駒の構成だけに依存する material Zobrist キー。
    pub material: ZobristKey,

    /// 先手視点・手駒込みの駒割り評価値。
    pub material_value: i32,
}

impl PartialKeys {
    #[must_use]
    pub const fn new(
        pawn: ZobristKey,
        minor: ZobristKey,
        non_pawn: [ZobristKey; Color::COUNT],
        material: ZobristKey,
        material_value: i32,
    ) -> Self {
        Self { pawn, minor, non_pawn, material, material_value }
    }
}

impl Default for PartialKeys {
    fn default() -> Self {
        Self {
            pawn: Zobrist::no_pawns(),
            minor: ZobristKey::default(),
            non_pawn: [ZobristKey::default(); Color::COUNT],
            material: ZobristKey::default(),
            material_value: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UndoPayload {
    captured: Piece,
    last_move: Move32,
    last_moved_piece_type: PieceType,
}

impl UndoPayload {
    pub(crate) const EMPTY: Self = Self {
        captured: Piece::NONE,
        last_move: MOVE_NONE,
        last_moved_piece_type: PieceType::NONE,
    };

    #[inline(always)]
    pub(crate) const fn new(
        captured: Piece,
        last_move: Move32,
        last_moved_piece_type: PieceType,
    ) -> Self {
        Self { captured, last_move, last_moved_piece_type }
    }

    #[inline(always)]
    pub(crate) const fn captured_piece(&self) -> Piece {
        self.captured
    }

    #[inline(always)]
    pub(crate) const fn last_move(&self) -> Move32 {
        self.last_move
    }

    #[inline(always)]
    pub(crate) const fn last_moved_piece_type(&self) -> PieceType {
        self.last_moved_piece_type
    }
}

impl Default for UndoPayload {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NullTacticalPrefix {
    pinners: [Bitboard; Color::COUNT],
    blockers_for_king: [Bitboard; Color::COUNT],
}

impl NullTacticalPrefix {
    pub(crate) const EMPTY: Self = Self {
        pinners: [Bitboard::EMPTY; Color::COUNT],
        blockers_for_king: [Bitboard::EMPTY; Color::COUNT],
    };
}

impl Default for NullTacticalPrefix {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[derive(Clone, Debug)]
pub struct StateHot {
    /// 盤面Zobrist（board_key）
    pub(crate) board_key: ZobristKey,

    /// 持ち駒Zobrist（hand_key）
    pub(crate) hand_key: ZobristKey,

    /// 盤上の歩のみから構成される Zobrist キー。
    pub(crate) pawn_key: ZobristKey,

    /// 盤上の小駒のみから構成される Zobrist キー。
    pub(crate) minor_piece_key: ZobristKey,

    /// 色別の「歩以外の盤上駒」から構成される Zobrist キー。
    pub(crate) non_pawn_key: [ZobristKey; Color::COUNT],

    /// 盤上駒の構成だけに依存する material Zobrist キー。
    pub(crate) material_key: ZobristKey,

    /// 先手視点・手駒込みの駒割り評価値。
    pub(crate) material_value: i32,

    /// 同一局面の登場回数（千日手検出用）
    pub(crate) repetition_counter: i32,

    /// 同一局面までの距離（千日手検出用）
    /// 4回目以降の繰り返しは負の値で表す。
    pub(crate) repetition_distance: i32,

    /// 同一局面の繰り返し回数 - 1
    pub(crate) repetition_times: i32,

    /// null move からの手数
    pub(crate) plies_from_null: Ply,

    /// 王手・ピン・王手候補マスの戦術キャッシュ。
    pub(crate) tactical: TacticalCache,

    /// 連続王手カウンタ（[先手, 後手]）
    /// 王手でない手を指すと0にリセットされる
    pub(crate) continuous_check: [u16; 2],

    /// 現局面の手番側の持ち駒
    pub(crate) hand: Hand,

    /// 千日手の状態（キャッシュ用）
    pub(crate) repetition_type: RepetitionState,
}

impl Default for StateHot {
    fn default() -> Self {
        let partial = PartialKeys::default();
        Self {
            board_key: ZobristKey::default(),
            hand_key: ZobristKey::default(),
            pawn_key: partial.pawn,
            minor_piece_key: partial.minor,
            non_pawn_key: partial.non_pawn,
            material_key: partial.material,
            material_value: partial.material_value,
            repetition_counter: 0,
            repetition_distance: 0,
            repetition_times: 0,
            plies_from_null: 0,
            tactical: TacticalCache::EMPTY,
            continuous_check: [0; Color::COUNT],
            hand: Hand::ZERO,
            repetition_type: RepetitionState::None,
        }
    }
}

impl StateHot {
    #[must_use]
    pub const fn partial_keys(&self) -> PartialKeys {
        PartialKeys::new(
            self.pawn_key,
            self.minor_piece_key,
            self.non_pawn_key,
            self.material_key,
            self.material_value,
        )
    }

    #[must_use]
    pub const fn check_square(&self, piece_type: PieceType) -> Bitboard {
        self.tactical.check_square(piece_type)
    }

    #[must_use]
    pub const fn blockers_for_king(&self, color: Color) -> Bitboard {
        self.tactical.blockers_for_king(color)
    }

    /// 盤面 Zobrist（board_key）を返す。
    #[must_use]
    pub const fn board_key(&self) -> ZobristKey {
        self.board_key
    }

    /// 手番側の持ち駒を返す。
    #[must_use]
    pub const fn hand(&self) -> Hand {
        self.hand
    }

    /// null move からの手数を返す。
    #[must_use]
    pub const fn plies_from_null(&self) -> Ply {
        self.plies_from_null
    }

    /// 同一局面の繰り返し回数 - 1 を返す。
    #[must_use]
    pub const fn repetition_times(&self) -> i32 {
        self.repetition_times
    }

    /// 千日手の状態（キャッシュ）を返す。
    #[must_use]
    pub const fn repetition_type(&self) -> RepetitionState {
        self.repetition_type
    }

    /// 連続王手カウンタ（[先手, 後手]）を返す。
    #[must_use]
    pub const fn continuous_checks(&self) -> [u16; Color::COUNT] {
        self.continuous_check
    }

    #[inline(always)]
    pub(crate) const fn tactical_cache(&self) -> &TacticalCache {
        &self.tactical
    }
}

// ANCHOR: state_info_struct
/// 局面の履歴情報を保持する構造体
///
/// `apply_move32` 時の差分情報を記録し、`undo_move32` で完全に復元できるようにする。
/// また、千日手検出や王手情報のキャッシュも管理する。
#[derive(Clone, Debug)]
pub struct StateInfo {
    /// undo 専用 cold payload。
    pub(crate) cold: StateColdPayload,

    /// 探索中に頻繁に読む current-state payload。
    pub(crate) hot: StateHot,
}
// ANCHOR_END: state_info_struct

impl Default for StateInfo {
    fn default() -> Self {
        Self { cold: StateColdPayload::EMPTY, hot: StateHot::default() }
    }
}

impl Deref for StateInfo {
    type Target = StateHot;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.hot
    }
}

impl DerefMut for StateInfo {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.hot
    }
}

impl StateInfo {
    /// 探索中に頻繁に読む hot payload を返す。
    #[must_use]
    #[inline(always)]
    pub const fn hot(&self) -> &StateHot {
        &self.hot
    }

    #[inline(always)]
    pub(crate) fn write_root_hot(
        &mut self,
        board_key: ZobristKey,
        hand_key: ZobristKey,
        hand: Hand,
    ) {
        self.hot.board_key = board_key;
        self.hot.hand_key = hand_key;
        self.hot.hand = hand;
    }

    /// 探索開始時のリセット（ゼロクリア）
    pub(crate) fn reset_sfen(&mut self) {
        *self = Self::default();
    }

    #[inline(always)]
    pub(crate) const fn tactical_cache(&self) -> &TacticalCache {
        &self.hot.tactical
    }

    #[inline(always)]
    pub(crate) fn replace_tactical_cache(&mut self, tactical: TacticalCache) {
        self.hot.tactical = tactical;
    }

    #[inline(always)]
    pub(crate) fn copy_tactical_cache_from(&mut self, tactical: &TacticalCache) {
        self.hot.tactical = *tactical;
    }

    #[inline(always)]
    pub(crate) const fn null_tactical_prefix(&self) -> NullTacticalPrefix {
        NullTacticalPrefix {
            pinners: self.hot.tactical.pinners,
            blockers_for_king: self.hot.tactical.blockers_for_king,
        }
    }

    #[inline(always)]
    pub(crate) fn copy_null_tactical_prefix_from(&mut self, prefix: NullTacticalPrefix) {
        self.hot.tactical.pinners = prefix.pinners;
        self.hot.tactical.blockers_for_king = prefix.blockers_for_king;
    }

    /// 直前の指し手を返す。
    #[must_use]
    pub const fn last_move(&self) -> Move32 {
        self.cold.undo.last_move()
    }

    /// 直前に動かした駒種を返す。
    #[must_use]
    pub const fn last_moved_piece_type(&self) -> PieceType {
        self.cold.undo.last_moved_piece_type()
    }

    /// 部分ハッシュ・駒割りキャッシュをまとめて返す。
    #[must_use]
    pub const fn partial_keys(&self) -> PartialKeys {
        PartialKeys::new(
            self.hot.pawn_key,
            self.hot.minor_piece_key,
            self.hot.non_pawn_key,
            self.hot.material_key,
            self.hot.material_value,
        )
    }

    pub(crate) fn set_partial_keys(&mut self, keys: PartialKeys) {
        self.hot.pawn_key = keys.pawn;
        self.hot.minor_piece_key = keys.minor;
        self.hot.non_pawn_key = keys.non_pawn;
        self.hot.material_key = keys.material;
        self.hot.material_value = keys.material_value;
    }

    #[inline(always)]
    pub(crate) const fn move_prefix(&self) -> MoveStatePrefix {
        MoveStatePrefix {
            plies_from_null: self.hot.plies_from_null,
            continuous_check: self.hot.continuous_check,
        }
    }

    #[inline(always)]
    pub(crate) const fn null_prefix(&self) -> NullStatePrefix {
        NullStatePrefix {
            partial_keys: self.partial_keys(),
            continuous_check: self.hot.continuous_check,
            tactical: self.null_tactical_prefix(),
            last_move: self.last_move(),
            last_moved_piece_type: self.last_moved_piece_type(),
        }
    }

    #[inline(always)]
    pub(crate) fn copy_move_prefix_from(&mut self, prefix: MoveStatePrefix) {
        self.hot.plies_from_null = prefix.plies_from_null;
        self.hot.continuous_check = prefix.continuous_check;
    }

    #[inline(always)]
    pub(crate) fn copy_null_prefix_from(&mut self, prefix: NullStatePrefix) {
        self.set_partial_keys(prefix.partial_keys);
        self.hot.continuous_check = prefix.continuous_check;
        self.copy_null_tactical_prefix_from(prefix.tactical);
        self.cold.undo.last_move = prefix.last_move;
        self.cold.undo.last_moved_piece_type = prefix.last_moved_piece_type;
    }

    #[inline(always)]
    pub(crate) fn reset_move_cold_fields(&mut self) {
        self.cold.undo.captured = Piece::NONE;
    }

    #[inline(always)]
    pub(crate) fn reset_null_cold_fields(&mut self) {
        self.hot.plies_from_null = 0;
        self.cold.undo.captured = Piece::NONE;
        self.hot.repetition_counter = 0;
        self.hot.repetition_distance = 0;
        self.hot.repetition_times = 0;
        self.hot.repetition_type = RepetitionState::None;
    }

    #[inline(always)]
    pub(crate) fn write_null_tactical_after_turn_flip(&mut self, check_squares: CheckSquares) {
        self.hot.tactical.set_checkers(Bitboard::EMPTY);
        self.hot.tactical.set_check_squares(check_squares);
    }

    #[inline(always)]
    pub(crate) fn write_move_state(&mut self, state: MoveStateWriteBack) {
        self.cold = state.cold;
        self.hot.plies_from_null = state.hot.plies_from_null;
        self.hot.board_key = state.hot.board_key;
        self.hot.hand_key = state.hot.hand_key;
        self.set_partial_keys(state.hot.partial_keys);
        self.hot.hand = state.hot.hand;
        self.hot.continuous_check = state.hot.continuous_check;
        self.replace_tactical_cache(state.hot.tactical);
        self.hot.repetition_counter = state.hot.repetition_counter;
        self.hot.repetition_distance = state.hot.repetition_distance;
        self.hot.repetition_times = state.hot.repetition_times;
        self.hot.repetition_type = state.hot.repetition_type;
    }
}

#[derive(Clone, Copy)]
pub(crate) struct MoveStatePrefix {
    plies_from_null: Ply,
    continuous_check: [u16; Color::COUNT],
}

#[derive(Clone, Copy)]
pub(crate) struct NullStatePrefix {
    partial_keys: PartialKeys,
    continuous_check: [u16; Color::COUNT],
    tactical: NullTacticalPrefix,
    last_move: Move32,
    last_moved_piece_type: PieceType,
}

#[derive(Clone, Copy)]
pub(crate) struct StateHotComputed {
    pub(crate) plies_from_null: Ply,
    pub(crate) board_key: ZobristKey,
    pub(crate) hand_key: ZobristKey,
    pub(crate) partial_keys: PartialKeys,
    pub(crate) hand: Hand,
    pub(crate) continuous_check: [u16; Color::COUNT],
    pub(crate) tactical: TacticalCache,
    pub(crate) repetition_counter: i32,
    pub(crate) repetition_distance: i32,
    pub(crate) repetition_times: i32,
    pub(crate) repetition_type: RepetitionState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StateColdPayload {
    pub(crate) undo: UndoPayload,
}

impl StateColdPayload {
    pub(crate) const EMPTY: Self = Self { undo: UndoPayload::EMPTY };

    #[inline(always)]
    pub(crate) const fn captured_piece(&self) -> Piece {
        self.undo.captured_piece()
    }

    #[inline(always)]
    pub(crate) const fn last_move(&self) -> Move32 {
        self.undo.last_move()
    }

    #[inline(always)]
    pub(crate) const fn last_moved_piece_type(&self) -> PieceType {
        self.undo.last_moved_piece_type()
    }
}

impl Default for StateColdPayload {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[derive(Clone, Copy)]
pub(crate) struct MoveStateWriteBack {
    pub(crate) hot: StateHotComputed,
    pub(crate) cold: StateColdPayload,
}

#[derive(Clone, Copy)]
pub(crate) struct NullStateWriteBack {
    pub(crate) board_key: ZobristKey,
    pub(crate) hand_key: ZobristKey,
    pub(crate) hand: Hand,
    pub(crate) continuous_check: [u16; Color::COUNT],
    pub(crate) check_squares: CheckSquares,
}

/// segmented storage における 1 chunk あたりの `StateInfo` 数。
///
/// chunk 単位で `Box` 確保するため、grow しても既存 chunk の物理アドレスは動かない。
/// この値は内部実装の詳細であり、benchmark に応じて将来調整しうる。
const STATE_CHUNK_CAPACITY: usize = 64;

type StateChunk = Box<[StateInfo; STATE_CHUNK_CAPACITY]>;

// ANCHOR: state_stack_struct
/// 可変長の `StateInfo` スタック
///
/// 局面履歴を動的に保持する。
///
/// # Address stability 契約
///
/// `StateStack` は内部を chunk 単位の segmented storage で持つ。
/// このため、以下が保証される。
///
/// - 同一 `StateStack` インスタンス内で live な entry
///   (`0..=current_index()`) の物理アドレスは、
///   `push_empty` / `push_for_move` / `push_clone_from_prev` で
///   stack を伸長しても変わらない。
///
/// 一方で次は契約外であり、view / pointer は再取得を要する。
///
/// - `pop` / `pop_unchecked` で外れた entry の参照
/// - `reset_sfen` / `reset_with_position` 後の任意 entry の参照
/// - `clone` / `clone_for_search` 後の別インスタンス間の pointer identity
/// - `drop` 後の任意 entry の参照
///
/// なお「物理アドレスが安定している」ことは、その entry が
/// 「current state を表し続ける」ことを意味しない。
/// `apply_move32` / `apply_null_move` 等の mutation 後は、
/// 旧 entry は live ではあっても current ではなくなる。
#[derive(Debug)]
pub struct StateStack {
    /// chunk のリスト。各 chunk は `Box` 配下の固定長配列で、grow しても再配置されない。
    chunks: Vec<StateChunk>,

    /// 現在のスタックトップのインデックス
    head: StateIndex,
}
// ANCHOR_END: state_stack_struct

impl StateStack {
    #[inline(always)]
    const fn locate(idx: StateIndex) -> (usize, usize) {
        (idx / STATE_CHUNK_CAPACITY, idx % STATE_CHUNK_CAPACITY)
    }

    fn new_chunk() -> StateChunk {
        Box::new(std::array::from_fn(|_| StateInfo::default()))
    }

    #[inline]
    fn ensure_chunk_for(&mut self, idx: StateIndex) {
        let (chunk_idx, _) = Self::locate(idx);
        while self.chunks.len() <= chunk_idx {
            self.chunks.push(Self::new_chunk());
        }
    }

    /// 現在の head より先に `additional` 個の state slot を事前確保する。
    ///
    /// 成功後は、pop を挟まない `additional` 回以内の push で chunk を増設しない。
    /// 予約 token は管理せず、pop 済みの物理 slot は再利用できる。
    pub(crate) fn prepare_additional(&mut self, additional: usize) -> bool {
        let Some(last) = self.head.checked_add(additional) else {
            return false;
        };
        self.ensure_chunk_for(last);
        true
    }

    /// 次の state slot を allocation なしで利用できるかを返す。
    #[must_use]
    #[inline(always)]
    pub(crate) fn has_prepared_next(&self) -> bool {
        let Some(next) = self.head.checked_add(1) else {
            return false;
        };
        let (chunk_idx, _) = Self::locate(next);
        chunk_idx < self.chunks.len()
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn prepared_additional_capacity(&self) -> usize {
        self.chunks
            .len()
            .saturating_mul(STATE_CHUNK_CAPACITY)
            .saturating_sub(self.head.saturating_add(1))
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) const fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// 新しい `StateStack` を作成
    #[must_use]
    pub fn new() -> Self {
        let chunks = vec![Self::new_chunk()];
        Self { chunks, head: 0 }
    }

    /// 探索開始時のリセット（root state のみ初期化し、追加 chunk は破棄）
    pub(crate) fn reset_sfen(&mut self) {
        self.head = 0;
        self.chunks.truncate(1);
        self.chunks[0][0].reset_sfen();
    }

    /// 局面に同期した初期StateInfoを設定する
    #[cfg(test)]
    pub(crate) fn reset_with_position(&mut self, pos: &mut Position) {
        self.reset_sfen();
        let mut state = StateInfo::default();
        state.write_root_hot(pos.board_key, pos.hand_key, pos.hand(pos.turn()));
        pos.compute_caches_for_state(&mut state);
        self.write_current_state(state);
        pos.st_index = self.current_index();
        pos.debug_assert_partial_keys_consistent();
    }

    /// 現在の（最新の）`StateInfo` への参照を取得
    #[cfg(test)]
    #[must_use]
    #[inline(always)]
    pub(crate) fn current(&self) -> &StateInfo {
        let (chunk_idx, offset) = Self::locate(self.head);
        &self.chunks[chunk_idx][offset]
    }

    /// 現在の`StateInfo`のインデックスを取得
    #[must_use]
    #[inline(always)]
    pub const fn current_index(&self) -> StateIndex {
        self.head
    }

    /// 現在の（最新の）`StateInfo` への可変参照を取得
    #[inline(always)]
    #[cfg(test)]
    pub(crate) fn current_mut(&mut self) -> &mut StateInfo {
        let (chunk_idx, offset) = Self::locate(self.head);
        &mut self.chunks[chunk_idx][offset]
    }

    /// 指定 index の hot state への参照を取得する。
    #[must_use]
    #[inline(always)]
    pub(crate) fn hot(&self, idx: StateIndex) -> &StateHot {
        let (chunk_idx, offset) = Self::locate(idx);
        self.chunks[chunk_idx][offset].hot()
    }

    /// 指定 index の cold payload への参照を取得する。
    #[must_use]
    #[inline(always)]
    pub(crate) fn cold(&self, idx: StateIndex) -> &StateColdPayload {
        let (chunk_idx, offset) = Self::locate(idx);
        &self.chunks[chunk_idx][offset].cold
    }

    /// 指定 index の `StateInfo` への可変参照を取得する。
    ///
    /// 局面履歴の整合性を保つため、外部 crate には公開しない。
    #[inline(always)]
    #[cfg(test)]
    pub(crate) fn get_mut(&mut self, idx: StateIndex) -> &mut StateInfo {
        let (chunk_idx, offset) = Self::locate(idx);
        &mut self.chunks[chunk_idx][offset]
    }

    #[inline(always)]
    pub(crate) fn write_current_state(&mut self, state: StateInfo) {
        let (chunk_idx, offset) = Self::locate(self.head);
        self.chunks[chunk_idx][offset] = state;
    }

    #[inline(always)]
    pub(crate) fn write_move_state(&mut self, idx: StateIndex, state: MoveStateWriteBack) {
        let (chunk_idx, offset) = Self::locate(idx);
        self.chunks[chunk_idx][offset].write_move_state(state);
    }

    #[inline(always)]
    pub(crate) fn write_null_state_after_turn_flip(
        &mut self,
        idx: StateIndex,
        state: NullStateWriteBack,
    ) {
        let (chunk_idx, offset) = Self::locate(idx);
        let entry = &mut self.chunks[chunk_idx][offset];
        entry.hot.board_key = state.board_key;
        entry.hot.hand_key = state.hand_key;
        entry.hot.hand = state.hand;
        entry.hot.continuous_check = state.continuous_check;
        entry.write_null_tactical_after_turn_flip(state.check_squares);
    }

    #[inline(always)]
    pub(crate) fn sync_caches_from_state(&mut self, idx: StateIndex, state: &StateInfo) {
        let (chunk_idx, offset) = Self::locate(idx);
        let current = &mut self.chunks[chunk_idx][offset];
        current.copy_tactical_cache_from(state.tactical_cache());
        current.set_partial_keys(state.partial_keys());
    }

    /// 新しい空の `StateInfo` をプッシュし、そのインデックスを返す
    /// 呼び出し側で内容を埋める責任がある
    #[cfg(test)]
    #[inline(always)]
    pub(crate) fn push_empty(&mut self) -> StateIndex {
        self.head += 1;
        let head = self.head;
        self.ensure_chunk_for(head);
        let (chunk_idx, offset) = Self::locate(head);
        self.chunks[chunk_idx][offset].reset_sfen();
        self.head
    }

    /// 現在の`StateInfo`を複製して新しいエントリをプッシュする
    ///
    /// 現在のエントリ全体を複製する。
    #[cfg(test)]
    #[inline(always)]
    pub(crate) fn push_clone_from_prev(&mut self) -> StateIndex {
        let prev_index = self.head;
        self.head += 1;
        let head = self.head;

        let (prev_chunk_idx, prev_offset) = Self::locate(prev_index);
        let prev_entry = self.chunks[prev_chunk_idx][prev_offset].clone();

        self.ensure_chunk_for(head);
        let (head_chunk_idx, head_offset) = Self::locate(head);
        self.chunks[head_chunk_idx][head_offset] = prev_entry;

        self.head
    }

    /// apply_move32 用に、必要なフィールドだけをコピーして新しいエントリをプッシュする
    ///
    /// 指し手適用に必要なフィールドだけを引き継ぐ。残りのフィールドは呼び出し側で全て上書きされるため、
    /// ゼロクリア（`..StateInfo::default()`）を行わない。
    #[inline(always)]
    pub(crate) fn push_for_move(&mut self) -> StateIndex {
        let prev_index = self.head;
        self.head += 1;
        let head = self.head;

        let (prev_chunk_idx, prev_offset) = Self::locate(prev_index);
        let prefix = self.chunks[prev_chunk_idx][prev_offset].move_prefix();
        self.ensure_chunk_for(head);
        let (head_chunk_idx, head_offset) = Self::locate(head);
        self.chunks[head_chunk_idx][head_offset].copy_move_prefix_from(prefix);

        self.head
    }

    /// null move 用に、盤面が変わらないため引き継げるフィールドだけをコピーして
    /// 新しいエントリをプッシュする。
    #[inline(always)]
    pub(crate) fn push_for_null_move(&mut self) -> StateIndex {
        let prev_index = self.head;
        self.head += 1;
        let head = self.head;

        let (prev_chunk_idx, prev_offset) = Self::locate(prev_index);
        let prefix = self.chunks[prev_chunk_idx][prev_offset].null_prefix();
        self.ensure_chunk_for(head);
        let (head_chunk_idx, head_offset) = Self::locate(head);
        self.chunks[head_chunk_idx][head_offset].copy_null_prefix_from(prefix);

        self.head
    }

    /// `apply_move32` 用に next state slot を準備する。
    ///
    /// `push_for_move` を内部で呼び、追加で next slot 専用の初期化（`captured = Piece::NONE`）も
    /// 行う。`apply_move32` 側からは「stack 側で carry-over の責務が完結する」ことを期待する
    /// 統一 helper として使う。
    ///
    /// 戻り値は新しい current となる `StateIndex`。
    /// `apply_move32` で必要となる prev フィールド（`continuous_check` / `plies_from_null` 等）は、
    /// この helper 呼び出しの直後に `state_stack[state_idx]` 経由で carry-over 済みの値として読み出せる。
    #[inline(always)]
    pub(crate) fn prepare_next_for_move(&mut self) -> StateIndex {
        let state_idx = self.push_for_move();
        let (chunk_idx, offset) = Self::locate(state_idx);
        self.chunks[chunk_idx][offset].reset_move_cold_fields();
        state_idx
    }

    /// `apply_null_move` 用に next state slot を準備する。
    ///
    /// `push_for_null_move` で prev から必要なフィールドだけをコピーした上で、
    /// null move の不変条件で強制的に確定するフィールドを stack 側で初期化する。
    /// 具体的には次をリセットする。
    ///
    /// - `plies_from_null` ← 0
    /// - `captured` ← `Piece::NONE`
    /// - `checkers` ← `Bitboard::EMPTY` (null move では局面の駒位置が変わらないため、
    ///   呼び出し前提として「現状で王手がかかっていない」ことを `apply_null_move` 側で
    ///   debug assert する)
    /// - `repetition_*` ← 初期値 (null move 直後は同一局面の判定対象外)
    ///
    /// 残りの `continuous_check` 反転や `check_squares` 再計算など、turn / zobrist に
    /// 依存するフィールドは `apply_null_move` 側の責務として残す。
    #[inline(always)]
    pub(crate) fn prepare_next_for_null_move(&mut self) -> StateIndex {
        let state_idx = self.push_for_null_move();
        let (chunk_idx, offset) = Self::locate(state_idx);
        self.chunks[chunk_idx][offset].reset_null_cold_fields();
        state_idx
    }

    /// スタックをポップし、ポップしたエントリのインデックスを返す。
    /// スタックが空（ルート局面のみ）の場合は `None` を返す。
    #[inline(always)]
    pub(crate) fn pop(&mut self) -> Option<StateIndex> {
        if self.head == 0 {
            return None;
        }

        let popped = self.head;
        self.head = popped - 1;
        Some(popped)
    }

    /// スタックをポップし、ポップしたエントリのインデックスを返す（境界チェックなし）。
    ///
    /// # Safety
    /// - `head > 0` が成り立つこと。
    #[inline(always)]
    pub(crate) unsafe fn pop_unchecked(&mut self) -> StateIndex {
        let popped = self.head;
        self.head = popped - 1;
        popped
    }

    /// 現在の深さ（スタックに積まれている `StateInfo` の数）を取得
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn depth(&self) -> usize {
        self.head
    }
}

impl Index<StateIndex> for StateStack {
    type Output = StateInfo;

    fn index(&self, idx: StateIndex) -> &Self::Output {
        let (chunk_idx, offset) = Self::locate(idx);
        &self.chunks[chunk_idx][offset]
    }
}

impl Clone for StateStack {
    fn clone(&self) -> Self {
        let chunks = self.chunks.iter().map(|chunk| Box::new((**chunk).clone())).collect();
        Self { chunks, head: self.head }
    }
}

impl Default for StateStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
